//! 鼠标追逐复用Desktop的唯一移动令牌；阶段反馈不改写业务活动或模型上下文。
use super::*;
use std::time::Instant;

const WAITING: Duration = Duration::from_secs(5);
const FEEDBACK: Duration = Duration::from_secs(3);
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Phase { Running, Waiting, Sad }
#[derive(Clone, Serialize)]
#[serde(rename_all="camelCase")]
pub struct Chase {
    pub phase: Phase,
    pub persona_id: String,
    pub session_id: String,
    pub context_revision: u64,
    #[serde(skip)] since: Instant,
}
impl Chase {
    pub(super) fn new(walking:&Walking)->Self {
        Self {phase:Phase::Running,persona_id:walking.persona_id.clone(),session_id:walking.session_id.clone(),context_revision:walking.context_revision,since:Instant::now()}
    }
    fn advance(&mut self, now:Instant, reached:bool)->bool {
        if self.phase==Phase::Running && reached { self.phase=Phase::Waiting;self.since=now; }
        else if self.phase==Phase::Waiting && now.saturating_duration_since(self.since)>=WAITING {self.phase=Phase::Sad;self.since=now;}
        else if self.phase==Phase::Sad && now.saturating_duration_since(self.since)>=FEEDBACK {return true;}
        false
    }
    fn pettable(&self,input:&crate::pet::controller::InteractionInput,now:Instant)->bool {
        self.phase==Phase::Waiting && now.saturating_duration_since(self.since)<WAITING
            && matches!(input.action,crate::pet::controller::Action::Click)
            && self.persona_id==input.persona_id && self.session_id==input.session_id && self.context_revision==input.context_revision
    }
}
pub(super) fn draw(random:&mut dyn RandomSource)->AppResult<bool> {Ok(random.below(100)?<5)}

impl Desktop {
    pub(crate) fn claim_petting(&self,input:&crate::pet::controller::InteractionInput)->AppResult<Option<Snapshot>> {
        let mut s=self.inner.lock().map_err(|_|AppError::storage())?;
        if !s.chase.as_ref().is_some_and(|c|c.pettable(input,Instant::now())) {return Ok(None)}
        self.epoch.fetch_add(1,Ordering::SeqCst);
        s.stop_motion();s.target=None;
        Ok(Some(s.snapshot()))
    }
}

fn target(point:(i32,i32),cursor:(i32,i32),body:Rect,area:Area,size:PhysicalSize<u32>,scale:f64)->((i32,i32),bool) {
    let x=f64::from(point.0)+body.x*scale;
    let y=f64::from(point.1)+body.y*scale;
    let dx=(f64::from(cursor.0)-x).min(0.0).abs().max((f64::from(cursor.0)-x-body.width*scale).max(0.0));
    let dy=(f64::from(cursor.1)-y).min(0.0).abs().max((f64::from(cursor.1)-y-body.height*scale).max(0.0));
    let reached=dx==0.0 && dy==0.0;
    let desired=((f64::from(cursor.0)-(body.x+body.width/2.0)*scale).round() as i32,(f64::from(cursor.1)-(body.y+body.height/2.0)*scale).round() as i32);
    (fit_body(desired,size,area,body,scale),reached)
}

pub(super) fn run(app:&tauri::AppHandle,window:&WebviewWindow,generation:u64,start:(i32,i32),size:PhysicalSize<u32>,area:Area,scale:f64,mut walking:Walking) {
    let managed=app.state::<Desktop>();
    if let Ok(snapshot)=managed.snapshot(){publish(app,&snapshot);}
    let mut point=start;
    let mut view=None;
    let mut checked=Instant::now();
    let mut moved=Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(30));
        if managed.epoch.load(Ordering::SeqCst)!=generation || !window.is_visible().unwrap_or(false) || window.is_minimized().unwrap_or(true) || chat_open(app) {break}
        if view.is_none() || checked.elapsed()>=Duration::from_millis(150) {
            let Ok(next_view)=app.state::<Runtime>().pet_view() else {break};view=Some(next_view);checked=Instant::now();
        }
        let elapsed=moved.elapsed().as_secs_f64().min(0.1);moved=Instant::now();
        let Ok((actual,actual_size,actual_area,actual_scale))=geometry(window) else {break};
        if actual!=point || actual_size!=size || (actual_area.x,actual_area.y,actual_area.width,actual_area.height)!=(area.x,area.y,area.width,area.height) || (actual_scale-scale).abs()>0.01 {break}
        let (body,phase,contact)={
            let Ok(s)=managed.inner.lock() else {break};
            if s.motion!=Some(generation) || !view.as_ref().is_some_and(|view|s.allowed(view)) || s.toss.is_some() {break}
            let (Some(layout),Some(chase))=(&s.layout,&s.chase) else {break};
            (layout.contact.as_ref().and_then(|c|c.visible_rect()).unwrap_or(layout.anchor.unwrap_or(layout.body)),chase.phase,layout.contact.clone())
        };
        let mut reached=false;
        let mut next=point;
        if phase==Phase::Running {
            let Some(cursor)=toss::cursor() else {break};
            // 鼠标离开当前工作区后停止本次追逐，避免隔着屏幕边缘反复走动。
            if cursor.0<area.x || cursor.1<area.y || i64::from(cursor.0)>=i64::from(area.x)+i64::from(area.width) || i64::from(cursor.1)>=i64::from(area.y)+i64::from(area.height) {break}
            let (mut destination,caught)=target(point,cursor,body,area,size,scale);
            if let Some(aim)=contact.as_ref().and_then(|c|c.aim()) {destination=fit_body(((f64::from(cursor.0)-aim.x*scale).round() as i32,(f64::from(cursor.1)-aim.y*scale).round() as i32),size,area,body,scale);}reached=contact.as_ref().map(|c|c.contains(f64::from(cursor.0-point.0)/scale,f64::from(cursor.1-point.1)/scale)).unwrap_or(caught);
            if !reached {next=fit_body(next_step(point,destination,180.0*scale*elapsed),size,area,body,scale);}
        }
        if managed.epoch.load(Ordering::SeqCst)!=generation {break}
        if next!=point {
            if window.set_position(PhysicalPosition::new(next.0,next.1)).is_err(){break}
            walking.direction=if next.0<point.0 {WalkDirection::Left}else if next.0>point.0 {WalkDirection::Right}else{walking.direction};
            point=next;
        }
        let snapshot={
            let Ok(mut s)=managed.inner.lock() else {break};
            if s.motion!=Some(generation) {break}
            let Some(chase)=s.chase.as_mut() else {break};
            let old=chase.phase;
            let finished=chase.advance(Instant::now(),reached);
            let phase=chase.phase;
            if finished {s.stop_motion();s.target=None;let snapshot=s.snapshot();drop(s);publish(app,&snapshot);break}
            let show=phase==Phase::Running && outside_arrival_tolerance(start,point,scale);
            let change=old!=phase || (show && s.walking.as_ref().is_none_or(|w|w.direction!=walking.direction)) || (!show && s.walking.is_some());
            if change {s.walking=show.then(||walking.clone());s.revision+=1;Some(s.snapshot())}else{None}
        };
        if let Some(snapshot)=snapshot {publish(app,&snapshot);}
    }
    if let Ok(mut s)=managed.inner.lock() {
        if s.motion==Some(generation) {s.stop_motion();s.target=None;let snapshot=s.snapshot();drop(s);publish(app,&snapshot);}
    };
}

#[tauri::command]
pub async fn get_pet_chase_expression(window:WebviewWindow,runtime:tauri::State<'_,Runtime>,persona_id:String,phase:Phase)->AppResult<String> {
    authorize(&window)?;
    let rt=runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move||rt.read(|store,_|{
        let id=crate::error::parse_id(&persona_id)?;
        let mut random=rt.random.lock().map_err(|_|AppError::storage())?;
        if phase==Phase::Sad {
            if let Some(items)=crate::pet_creation::choices(&store.connection,id)? {
                let candidates=items.iter().filter(|a|a.kind=="expression" && a.category=="chat" && matches!(a.name.as_str(),"难过"|"伤心")).collect::<Vec<_>>();
                if !candidates.is_empty(){return Ok(crate::pet_creation::marker(&candidates[random.below(candidates.len() as u64)? as usize].id))}
            }
            return Ok("(｡•́︿•̀｡)".into());
        }
        let category=if phase==Phase::Waiting {"wait_pet"}else{"walk_right"};
        match crate::pet_creation::select(&store.connection,id,category,None,random.as_mut())? {
            Some(image)=>Ok(image),None=>crate::expressions::choose_local(&store.connection,id,category,random.as_mut()),
        }
    })).await.map_err(|_|AppError::storage())?
}

#[cfg(test)]
mod tests {
    use super::*;
    fn walking()->Walking {Walking {direction:WalkDirection::Left,persona_id:"1".into(),session_id:"2".into(),context_revision:4}}
    fn input()->crate::pet::controller::InteractionInput {crate::pet::controller::InteractionInput {persona_id:"1".into(),session_id:"2".into(),context_revision:4,action:crate::pet::controller::Action::Click}}
    #[test]
    fn reaches_once_waits_five_seconds_then_sad_three_seconds() {
        let mut c=Chase::new(&walking());let now=c.since;
        assert!(!c.advance(now+Duration::from_secs(10),false));assert_eq!(c.phase,Phase::Running);
        assert!(!c.advance(now+Duration::from_secs(11),true));assert_eq!(c.phase,Phase::Waiting);
        assert!(c.pettable(&input(),now+Duration::from_secs(13)));
        assert!(c.pettable(&input(),now+Duration::from_millis(15999)));assert!(!c.pettable(&input(),now+Duration::from_secs(16)));
        assert!(!c.advance(now+Duration::from_secs(16),true));assert_eq!(c.phase,Phase::Sad);
        assert!(!c.advance(now+Duration::from_secs(18),true));assert!(c.advance(now+Duration::from_secs(19),true));
    }
    #[test]
    fn petting_claim_is_once_context_scoped_and_drag_cancels() {
        let d=Desktop::default();let mut c=Chase::new(&walking());c.phase=Phase::Waiting;
        {let mut s=d.inner.lock().unwrap();s.chase=Some(c);s.motion=Some(1);}
        let mut stale=input();stale.context_revision=5;assert!(d.claim_petting(&stale).unwrap().is_none());
        assert!(d.claim_petting(&input()).unwrap().unwrap().chase.is_none());assert!(d.claim_petting(&input()).unwrap().is_none());
        d.inner.lock().unwrap().chase=Some(Chase::new(&walking()));d.begin_drag().unwrap();assert!(d.snapshot().unwrap().chase.is_none());
    }
    #[test]
    fn paused_layout_cancels_chase_and_invalidates_the_old_mover() {
        let d=Desktop::default();
        let mut layout=Layout {persona_id:"1".into(),session_id:"2".into(),context_revision:4,layout_revision:1,
            area:Rect{x:0.0,y:0.0,width:320.0,height:400.0},body:Rect{x:60.0,y:150.0,width:196.0,height:196.0},anchor:None,free_y:None,contact:None,
            offset:crate::pet::wander::Point::default(),paused:false};
        d.update(layout.clone()).unwrap();
        {let mut s=d.inner.lock().unwrap();s.chase=Some(Chase::new(&walking()));s.motion=Some(7);}
        let before=d.epoch.load(Ordering::SeqCst);layout.paused=true;layout.layout_revision=2;
        d.update(layout).unwrap();assert!(d.snapshot().unwrap().chase.is_none());
        assert!(d.inner.lock().unwrap().motion.is_none());assert_ne!(before,d.epoch.load(Ordering::SeqCst));
    }
    #[test]
    fn five_percent_draw_and_target_follow_cursor_using_body_not_transparent_canvas() {
        struct Draw(u64);impl RandomSource for Draw {fn below(&mut self,max:u64)->AppResult<u64>{assert_eq!(max,100);Ok(self.0)}}
        for n in 0..100 {assert_eq!(draw(&mut Draw(n)).unwrap(),n<5);}
        let body=Rect{x:200.0,y:250.0,width:100.0,height:100.0};let area=Area{x:0,y:0,width:1920,height:1080};let size=PhysicalSize::new(480,600);
        let (destination,caught)=target((100,100),(350,400),body,area,size,1.0);assert!(caught);assert_eq!(destination,(100,100));
        let (destination,caught)=target((100,100),(900,700),body,area,size,1.0);assert!(!caught);assert_eq!(destination,(650,400));
        assert!(!target((100,100),(110,110),body,area,size,1.0).1);
    }
    #[test]
    fn every_chase_exit_respects_configured_cooldown_without_a_running_timeout() {
        let mut s=Inner::default();let now=Instant::now();let cooldown=Duration::from_secs(600);assert!(s.chase_due(now,cooldown));
        let mut c=Chase::new(&walking());assert!(!c.advance(c.since+Duration::from_secs(3600),false));
        s.chase=Some(c);s.stop_motion();let ended=s.chase_ended_at.unwrap();
        assert!(!s.chase_due(ended+cooldown-Duration::from_millis(1),cooldown));assert!(s.chase_due(ended+cooldown,cooldown));
        assert!(!s.chase_due(ended+cooldown,Duration::from_secs(1200)));
        assert!(s.chase_due(ended+Duration::from_secs(60),Duration::from_secs(60)));
        s.stop_motion();assert_eq!(s.chase_ended_at,Some(ended));
    }
    #[test]
    fn transparent_pixels_and_old_ten_pixel_margin_do_not_count_as_contact() {
        let contact=crate::pet::wander::Contact{bounds:Rect{x:100.0,y:100.0,width:160.0,height:160.0},rows:(0..32).map(|y|if (6..26).contains(&y){0x00ffff00}else{0}).collect()};
        assert!(!contact.contains(110.0,150.0));assert!(!contact.contains(180.0,110.0));assert!(contact.contains(180.0,180.0));
        let visible=contact.visible_rect().unwrap();assert_eq!(visible,Rect{x:140.0,y:130.0,width:80.0,height:100.0});
        let aim=contact.aim().unwrap();assert!(contact.contains(aim.x,aim.y));
        let body=Rect{x:100.0,y:100.0,width:160.0,height:160.0};assert!(!target((0,0),(95,150),body,Area{x:0,y:0,width:1920,height:1080},PhysicalSize::new(400,500),1.0).1);
    }

}
