//! 原生拖动的上甩采样与单次抛接；不调用模型、不写飞行过程位置。
use super::*;
use std::{collections::VecDeque,time::Instant};

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Phase { Up, Down, Landed, Caught }
impl Phase { fn category(self)->&'static str {match self{Self::Up=>"toss_up",Self::Down=>"toss_down",Self::Landed=>"landing",Self::Caught=>"lifted"}} }
#[derive(Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Toss {
    pub phase:Phase,
    pub persona_id:String,
    pub session_id:String,
    pub context_revision:u64,
    #[serde(serialize_with="serialize_body")]
    pub body:Rect,
    pub controls_below:bool,
}
fn serialize_body<S:serde::Serializer>(body:&Rect,serializer:S)->Result<S::Ok,S::Error> {
    serde_json::json!({"x":body.x,"y":body.y,"width":body.width,"height":body.height}).serialize(serializer)
}
#[derive(Default)]
pub(super) struct DragTrace { samples:VecDeque<(Instant,(i32,i32))> }
impl DragTrace {
    pub(super) fn record(&mut self, now:Instant, point:(i32,i32)) {
        self.samples.push_back((now,point));
        while self.samples.front().is_some_and(|s|now.saturating_duration_since(s.0)>Duration::from_millis(140)){self.samples.pop_front();}
    }
    pub(super) fn upward_speed(&self,now:Instant,scale:f64)->Option<f64> {
        if !scale.is_finite()||scale<=0.0||self.samples.len()<3{return None}
        let first=self.samples.front()?;let last=self.samples.back()?;
        let dt=last.0.saturating_duration_since(first.0).as_secs_f64();
        if now.saturating_duration_since(last.0)>Duration::from_millis(70)||!(0.04..=0.14).contains(&dt){return None}
        let up=(f64::from(first.1.1)-f64::from(last.1.1))/scale;
        let sideways=(f64::from(first.1.0)-f64::from(last.1.0)).abs()/scale;
        let speed=up/dt;
        (up>=36.0&&speed>=900.0&&up>sideways).then_some(speed.min(1800.0))
    }
}
pub(crate) fn cursor()->Option<(i32,i32)> {
    #[cfg(windows)] {use windows::Win32::{Foundation::POINT,UI::WindowsAndMessaging::GetCursorPos};let mut p=POINT::default();unsafe{GetCursorPos(&mut p)}.ok()?;Some((p.x,p.y))}
    #[cfg(not(windows))] {None}
}
pub(crate) fn track_drag(window:&WebviewWindow) {
    let window=window.clone();
    let serial=window.state::<Desktop>().inner.lock().ok().map(|s|s.drag_serial);
    std::thread::spawn(move||loop{
        let desktop=window.state::<Desktop>();
        let Ok(mut state)=desktop.inner.lock()else{break};
        if Some(state.drag_serial)!=serial||!state.dragging||state.finishing_drag{break}
        if let Some(point)=cursor(){state.drag_trace.record(Instant::now(),point);}
        drop(state);std::thread::sleep(Duration::from_millis(16));
    });
}
struct Trajectory {start:f64,speed:f64,gravity:f64,floor:f64,impact:f64}
const LANDING_SECONDS:f64=3.0;
impl Trajectory {
    fn new(start:f64,ceiling:f64,floor:f64,speed:f64,scale:f64)->Self {
        let start=start.clamp(ceiling,floor.max(ceiling));let gravity=2800.0*scale;
        let speed=(speed*scale).min((2.0*gravity*(start-ceiling).max(0.0)).sqrt());
        let impact=(speed+(speed*speed+2.0*gravity*(floor-start).max(0.0)).sqrt())/gravity;
        Self{start,speed,gravity,floor,impact}
    }
    fn sample(&self,seconds:f64)->(f64,Phase) {
        if seconds>=self.impact{return(self.floor,Phase::Landed)}
        (self.start-self.speed*seconds+0.5*self.gravity*seconds*seconds,if seconds<self.speed/self.gravity{Phase::Up}else{Phase::Down})
    }
    fn settled(&self,seconds:f64)->bool {seconds>=self.impact+LANDING_SECONDS}
}
fn matches_view(t:&Toss,v:&View)->bool {
    t.persona_id==v.persona_id&&v.session_id.as_deref()==Some(t.session_id.as_str())&&t.context_revision==v.context_revision
        &&v.activity!=crate::pet::controller::Activity::Away&&v.can_interact&&v.error.is_none()
}
fn controls_below(body_top:f64,area_top:i32,scale:f64)->bool {body_top<f64::from(area_top)+60.0*scale}

pub(crate) async fn try_launch(window:&WebviewWindow,runtime:Runtime,velocity:Option<f64>)->AppResult<bool> {
    let Some(velocity)=velocity else{return Ok(false)};
    let(point,size,area,scale)=geometry(window)?;
    let window=window.clone();let worker=runtime.clone();
    let prepared=tauri::async_runtime::spawn_blocking(move||worker.read(|store,secrets|{
        let view=worker.settle_pet(store,secrets)?;
        if !view.can_interact||view.error.is_some()||view.activity==crate::pet::controller::Activity::Away{return Ok(None)}
        let managed=window.state::<Desktop>();let mut s=managed.inner.lock().map_err(|_|AppError::storage())?;
        let Some(old_body)=s.layout.as_ref().map(|l|l.anchor.unwrap_or(l.body))else{return Ok(None)};
        // 抛接画布把身体居中留出上下按钮空间，同时补偿窗口位置保持身体的屏幕位置。
        let body=Rect{x:(f64::from(size.width)/scale-old_body.width)/2.0,y:(f64::from(size.height)/scale-old_body.height)/2.0,..old_body};
        let origin=(area.fit(((f64::from(point.0)+(old_body.x-body.x)*scale).round() as i32,point.1),size).0,(f64::from(point.1)+(old_body.y-body.y)*scale).round() as i32);
        let ceiling=f64::from(area.y)-body.y*scale+4.0*scale;
        let floor=f64::from(area.y)+f64::from(area.height)-(body.y+body.height)*scale;
        if floor<ceiling{return Ok(None)}
        let trajectory=Trajectory::new(f64::from(origin.1),ceiling,floor,velocity,scale);
        let token=managed.toss_epoch.fetch_add(1,Ordering::SeqCst)+1;
        managed.epoch.fetch_add(1,Ordering::SeqCst);s.stop_motion();s.target=None;
        s.preferences.dock=Dock::None;s.preferences.floor_inset=0.0;save(&store.connection,&s.preferences)?;
        s.toss=Some(Toss{phase:trajectory.sample(0.0).1,persona_id:view.persona_id,session_id:view.session_id.unwrap_or_default(),context_revision:view.context_revision,body,controls_below:controls_below(f64::from(origin.1)+body.y*scale,area.y,scale)});
        s.revision+=1;
        Ok(Some((window.clone(),token,s.snapshot(),trajectory,origin)))
    })).await.map_err(|_|AppError::storage())??;
    let Some((window,token,snapshot,trajectory,point))=prepared else{return Ok(false)};
    window.set_position(PhysicalPosition::new(point.0,trajectory.sample(0.0).0.round() as i32)).map_err(|_|AppError::storage())?;
    publish(window.app_handle(),&snapshot);
    std::thread::spawn(move||run(window,runtime,token,trajectory,point,size,area,scale));
    Ok(true)
}
fn clear(window:&WebviewWindow,token:u64) {
    let desktop=window.state::<Desktop>();
    if desktop.toss_epoch.load(Ordering::SeqCst)!=token{return}
    if let Ok(mut s)=desktop.inner.lock(){if s.toss.take().is_some(){s.revision+=1;}let snapshot=s.snapshot();drop(s);publish(window.app_handle(),&snapshot);}
    if let Ok((p,size,area,_))=geometry(window){let target=area.fit(p,size);let _=window.set_position(PhysicalPosition::new(target.0,target.1));}
}
fn run(window:WebviewWindow,runtime:Runtime,token:u64,trajectory:Trajectory,point:(i32,i32),size:PhysicalSize<u32>,area:Area,scale:f64) {
    let started=Instant::now();let mut last_check=Instant::now()-Duration::from_secs(1);let mut last_position=None;
    loop {
        let desktop=window.state::<Desktop>();
        if desktop.toss_epoch.load(Ordering::SeqCst)!=token{
            if !desktop.has_toss()&&!desktop.is_dragging().unwrap_or(true){let _=window.state::<crate::pet_position::WindowPosition>().ensure_visible(&window,true);if let Ok(snapshot)=desktop.snapshot(){publish(window.app_handle(),&snapshot);}}
            return
        }
        if !window.is_visible().unwrap_or(false)||window.is_minimized().unwrap_or(true)||window.outer_size().ok()!=Some(size)||window.scale_factor().is_ok_and(|v|(v-scale).abs()>0.001){clear(&window,token);return}
        if last_check.elapsed()>=Duration::from_millis(150){
            let same_area=geometry(&window).is_ok_and(|(_,_,current,_)|current.x==area.x&&current.y==area.y&&current.width==area.width&&current.height==area.height);
            if !same_area{clear(&window,token);return}
            let current=runtime.pet_view();
            let valid=desktop.inner.lock().is_ok_and(|s|s.toss.as_ref().is_some_and(|t|current.as_ref().is_ok_and(|v|matches_view(t,v))));
            if !valid{clear(&window,token);return}last_check=Instant::now();
        }
        let seconds=started.elapsed().as_secs_f64();let(y,phase)=trajectory.sample(seconds);
        let position=(point.0,y.round() as i32);
        if last_position!=Some(position){if window.set_position(PhysicalPosition::new(position.0,position.1)).is_err(){clear(&window,token);return}last_position=Some(position);}
        if let Ok(mut s)=desktop.inner.lock(){
            let changed=if let Some(t)=s.toss.as_mut(){let below=controls_below(y+t.body.y*scale,area.y,scale);let changed=t.phase!=phase||t.controls_below!=below;t.phase=phase;t.controls_below=below;changed}else{false};
            if changed{s.revision+=1;let snapshot=s.snapshot();drop(s);publish(window.app_handle(),&snapshot);}
        }
        if trajectory.settled(seconds){break}
        std::thread::sleep(Duration::from_millis(16));
    }
    let result=runtime.read(|store,secrets|{
        let view=runtime.settle_pet(store,secrets)?;let desktop=window.state::<Desktop>();
        let dock={let mut random=runtime.random.lock().map_err(|_|AppError::storage())?;if random.below(2)?==0{Dock::RestSit}else{Dock::RestLie}};
        let mut s=desktop.inner.lock().map_err(|_|AppError::storage())?;
        if desktop.toss_epoch.load(Ordering::SeqCst)!=token||!s.toss.as_ref().is_some_and(|t|matches_view(t,&view)){return Ok(None)}
        s.preferences.floor_inset=landing_floor_inset(size,scale,s.toss.as_ref().unwrap().body);
        s.preferences.upper_layout=false;s.upper_layout=false;
        s.preferences.dock=dock;save(&store.connection,&s.preferences)?;
        s.toss=None;s.revision+=1;Ok(Some(s.snapshot()))
    });
    match result {
        Ok(Some(snapshot))=>{
            let position=window.state::<crate::pet_position::WindowPosition>().inner().clone();
            // 保持落地原点；坐趴布局沿用同一地面，不在旧图淡出时搬动整个窗口。
            publish(window.app_handle(),&snapshot);
            if let Err(error)=tauri::async_runtime::block_on(position.remember_placement(&window,runtime)){crate::pet_position::report(window.app_handle(),&error);}
        }
        _=>clear(&window,token),
    }
}

fn landing_floor_inset(size:PhysicalSize<u32>,scale:f64,body:Rect)->f64 {
    (f64::from(size.height)/scale-body.y-body.height).max(0.0)
}

#[tauri::command]
pub async fn get_pet_toss_expression(window:WebviewWindow,runtime:tauri::State<'_,Runtime>,persona_id:String,phase:Phase)->AppResult<String> {
    authorize(&window)?;let worker=runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move||worker.read(|store,_|{
        let id=crate::error::parse_id(&persona_id)?;let mut random=worker.random.lock().map_err(|_|AppError::storage())?;
        Ok(crate::pet_creation::select(&store.connection,id,phase.category(),None,random.as_mut())?.unwrap_or(crate::expressions::choose_local(&store.connection,id,phase.category(),random.as_mut())?))
    })).await.map_err(|_|AppError::storage())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upward_release_requires_recent_fast_predominantly_upward_motion() {
        let now=Instant::now();
        for(scale,dy,dx,expected)in[(1.0,-120,10,true),(1.5,-180,15,true),(2.0,-240,20,true),(1.0,-25,0,false),(1.0,120,0,false),(1.0,-120,180,false)] {
            let mut trace=DragTrace::default();
            for i in 0..=5 {trace.record(now+Duration::from_millis(i*20),(500+dx*i as i32/5,500+dy*i as i32/5));}
            assert_eq!(trace.upward_speed(now+Duration::from_millis(100),scale).is_some(),expected);
            assert!(trace.upward_speed(now+Duration::from_millis(200),scale).is_none());
        }
        let mut stopped=DragTrace::default();
        for i in 0..=10 {stopped.record(now+Duration::from_millis(i*20),(500,if i<3{500-i as i32*30}else{410}));}
        assert!(stopped.upward_speed(now+Duration::from_millis(200),1.0).is_none());
    }
    #[test]
    fn ballistic_path_decelerates_rising_accelerates_falling_and_preserves_three_second_landing() {
        let t=Trajectory::new(200.0,-100.0,600.0,1100.0,1.0);
        let apex=t.speed/t.gravity;
        assert_eq!(t.sample(0.0).1,Phase::Up);
        assert_eq!(t.sample(apex+0.01).1,Phase::Down);
        assert!(t.sample(apex).0>=-100.001);
        assert!(t.sample(0.1).0-t.sample(0.2).0<t.sample(0.0).0-t.sample(0.1).0);
        assert!(t.sample(apex+0.3).0-t.sample(apex+0.2).0>t.sample(apex+0.2).0-t.sample(apex+0.1).0);
        assert_eq!(t.sample(t.impact),(600.0,Phase::Landed));
        assert_eq!(t.sample(t.impact+2.999),(600.0,Phase::Landed));
        assert!(!t.settled(t.impact+2.999));assert!(t.settled(t.impact+3.0));
        for scale in [1.0,1.5,2.0]{
            let t=Trajectory::new(-80.0*scale,-100.0*scale,600.0*scale,1800.0,scale);
            for i in 0..=100{let(y,_)=t.sample(t.impact*i as f64/100.0);assert!(y>=-100.0*scale-0.001&&y<=600.0*scale+0.001);}
        }
    }
    #[test]
    fn away_error_and_changed_context_cannot_continue_toss() {
        let mut view=View{persona_id:"1".into(),session_id:Some("2".into()),context_revision:3,revision:1,activity:crate::pet::controller::Activity::Normal,expression:"x".into(),intimacy:Some(0),sleep_until:None,error:None,can_interact:true,can_wander:true};
        let toss=Toss{phase:Phase::Down,persona_id:"1".into(),session_id:"2".into(),context_revision:3,body:Rect{x:0.0,y:0.0,width:172.0,height:172.0},controls_below:false};
        assert!(matches_view(&toss,&view));view.activity=crate::pet::controller::Activity::Away;assert!(!matches_view(&toss,&view));
        view.activity=crate::pet::controller::Activity::Normal;view.context_revision+=1;assert!(!matches_view(&toss,&view));
        view.context_revision=3;view.error=Some("连接错误".into());assert!(!matches_view(&toss,&view));
    }
    #[test]
    fn landed_to_bottom_dock_keeps_the_same_ground_without_moving_the_window() {
        let area=Area{x:-1600,y:-100,width:1600,height:1000};let body=Rect{x:62.0,y:102.0,width:196.0,height:196.0};
        for scale in [1.0,1.25,1.5,2.0] {
            let size=PhysicalSize::new((320.0*scale) as u32,(400.0*scale) as u32);let inset=landing_floor_inset(size,scale,body);
            let origin=(-900,(f64::from(area.y)+f64::from(area.height)-(body.y+body.height)*scale).round() as i32);
            let docked=snap_floor(Dock::RestLie,origin,size,area,inset,scale);
            assert_eq!(docked.1,origin.1);assert!((f64::from(origin.1)+(400.0-inset)*scale-f64::from(area.y+area.height as i32)).abs()<=0.5);
        }
    }

}
