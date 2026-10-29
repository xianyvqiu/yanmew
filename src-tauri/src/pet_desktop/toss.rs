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
    pub velocity:Velocity,
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
#[derive(Clone,Copy,Debug,Default,Serialize)]
pub struct Velocity {pub x:f64,pub y:f64}
#[derive(Default)]
pub(super) struct DragTrace { samples:VecDeque<(Instant,(i32,i32))> }
impl DragTrace {
    pub(super) fn record(&mut self, now:Instant, point:(i32,i32)) {
        self.samples.push_back((now,point));
        while self.samples.front().is_some_and(|s|now.saturating_duration_since(s.0)>Duration::from_millis(120)){self.samples.pop_front();}
    }
    fn velocity(&self,now:Instant,scale:f64)->Option<Velocity> {
        if !scale.is_finite()||scale<=0.0||self.samples.len()<3{return None}
        let first=self.samples.front()?;let last=self.samples.back()?;
        let dt=last.0.saturating_duration_since(first.0).as_secs_f64();
        if now.saturating_duration_since(last.0)>Duration::from_millis(70)||!(0.04..=0.125).contains(&dt){return None}
        Some(Velocity{x:(f64::from(last.1.0)-f64::from(first.1.0))/scale/dt,y:(f64::from(last.1.1)-f64::from(first.1.1))/scale/dt})
    }
    pub(super) fn upward_velocity(&self,now:Instant,scale:f64)->Option<Velocity> {
        let velocity=self.velocity(now,scale)?;let speed=velocity.x.hypot(velocity.y);
        // 任意上向角度均可；过滤水平抖动，不要求竖直速度大于横向速度。
        if velocity.y>=-60.0||speed<900.0{return None}
        let factor=(1800.0/speed).min(1.0);Some(Velocity{x:velocity.x*factor,y:velocity.y*factor})
    }
}
pub(crate) fn cursor()->Option<(i32,i32)> {
    #[cfg(windows)] {use windows::Win32::{Foundation::POINT,UI::WindowsAndMessaging::GetCursorPos};let mut p=POINT::default();unsafe{GetCursorPos(&mut p)}.ok()?;Some((p.x,p.y))}
    #[cfg(not(windows))] {None}
}
#[derive(Default)]
struct ReleaseWatch {since:Option<Instant>}
impl ReleaseWatch {
    fn overdue(&mut self,now:Instant,down:Option<bool>)->bool {
        if down!=Some(false){self.since=None;return false}
        now.saturating_duration_since(*self.since.get_or_insert(now))>=Duration::from_millis(96)
    }
}
fn primary_button_down()->Option<bool> {
    #[cfg(windows)] {use windows::Win32::UI::{Input::KeyboardAndMouse::{GetAsyncKeyState,VK_LBUTTON,VK_RBUTTON},WindowsAndMessaging::{GetSystemMetrics,SM_SWAPBUTTON}};let key=if unsafe{GetSystemMetrics(SM_SWAPBUTTON)}!=0{VK_RBUTTON}else{VK_LBUTTON};Some(unsafe{GetAsyncKeyState(i32::from(key.0))}<0)}
    #[cfg(not(windows))] {None}
}
pub(crate) fn track_drag(window:&WebviewWindow) {
    let window=window.clone();
    let scale=window.scale_factor().unwrap_or(1.0);
    #[cfg(windows)] let hwnd=window.hwnd().ok().map(|handle|handle.0 as usize);
    let serial=window.state::<Desktop>().inner.lock().ok().map(|s|s.drag_serial);
    std::thread::spawn(move||{
      let mut release=ReleaseWatch::default();
      loop{
        let down=primary_button_down();
        let desktop=window.state::<Desktop>();
        let Ok(mut state)=desktop.inner.lock()else{break};
        if Some(state.drag_serial)!=serial||!state.dragging||state.finishing_drag{break}
        let now=Instant::now();
        if release.overdue(now,down) {
            drop(state);
            // 鼠标已持续松开却没有结束通知，取消遗漏的系统循环并按原流程收尾。
            #[cfg(windows)] if let Some(hwnd)=hwnd{use windows::Win32::{Foundation::{HWND,LPARAM,WPARAM},UI::WindowsAndMessaging::{PostMessageW,WM_CANCELMODE}};let _=unsafe{PostMessageW(Some(HWND(hwnd as *mut std::ffi::c_void)),WM_CANCELMODE,WPARAM(0),LPARAM(0))};}
            let window=window.clone();
            tauri::async_runtime::spawn(async move{
                if let Err(error)=crate::commands::finish_pet_drag_for(&window,serial).await{crate::pet_position::report(window.app_handle(),&error);}
                let current=window.state::<Desktop>().drag_finished_for(serial);
                if current{let _=window.emit("pet-drag-finished",());}
            });break;
        }
        if let Some(point)=cursor(){state.drag_trace.record(now,point);}
        let velocity=state.drag_trace.velocity(now,scale).unwrap_or_default();
        let changed=state.drag_velocity.is_none_or(|old|(old.x-velocity.x).abs()>8.0||(old.y-velocity.y).abs()>8.0);
        let snapshot=if changed{state.drag_velocity=Some(velocity);state.revision+=1;Some(state.snapshot())}else{None};
        drop(state);if let Some(snapshot)=snapshot{publish(window.app_handle(),&snapshot);}std::thread::sleep(Duration::from_millis(32));
      }
    });
}
struct Trajectory {
    x:f64,y:f64,velocity:Velocity,gravity:f64,left:f64,right:f64,ceiling:f64,floor:f64,
    previous:f64,landed_at:Option<f64>,
}
const LANDING_SECONDS:f64=3.0;
const RESTITUTION:f64=0.62;
impl Trajectory {
    fn new(origin:(f64,f64),bounds:(f64,f64,f64,f64),velocity:Velocity,scale:f64)->Self {
        let(left,right,ceiling,floor)=bounds;
        Self{x:origin.0.clamp(left,right),y:origin.1.clamp(ceiling,floor),velocity:Velocity{x:velocity.x*scale,y:velocity.y*scale},gravity:2800.0*scale,left,right,ceiling,floor,previous:0.0,landed_at:None}
    }
    fn phase(&self)->Phase {if self.landed_at.is_some(){Phase::Landed}else if self.velocity.y<0.0{Phase::Up}else{Phase::Down}}
    fn advance(&mut self,seconds:f64) {
        let mut remaining=(seconds-self.previous).clamp(0.0,0.25);self.previous=seconds;
        // 小步积分保持抛物线与反弹连续；长时间停顿不让角色瞬移穿屏。
        while remaining>0.000001 && self.landed_at.is_none() {
            let dt=remaining.min(1.0/240.0);remaining-=dt;
            self.x+=self.velocity.x*dt;self.y+=self.velocity.y*dt+0.5*self.gravity*dt*dt;self.velocity.y+=self.gravity*dt;
            if self.x<self.left {self.x=(self.left+(self.left-self.x)*RESTITUTION).min(self.right);self.velocity.x=self.velocity.x.abs()*RESTITUTION;}
            else if self.x>self.right {self.x=(self.right-(self.x-self.right)*RESTITUTION).max(self.left);self.velocity.x=-self.velocity.x.abs()*RESTITUTION;}
            if self.y<self.ceiling {self.y=(self.ceiling+(self.ceiling-self.y)*RESTITUTION).min(self.floor);self.velocity.y=self.velocity.y.abs()*RESTITUTION;}
            if self.y>=self.floor&&self.velocity.y>=0.0 {self.y=self.floor;self.velocity=Velocity::default();self.landed_at=Some(seconds-remaining);}
        }
    }
    fn settled(&self,seconds:f64)->bool {self.landed_at.is_some_and(|at|seconds>=at+LANDING_SECONDS)}
}
fn matches_view(t:&Toss,v:&View)->bool {
    t.persona_id==v.persona_id&&v.session_id.as_deref()==Some(t.session_id.as_str())&&t.context_revision==v.context_revision
        &&v.activity!=crate::pet::controller::Activity::Away&&v.can_interact&&v.error.is_none()
}
fn controls_below(body_top:f64,area_top:i32,scale:f64)->bool {body_top<f64::from(area_top)+60.0*scale}

pub(crate) async fn try_launch(window:&WebviewWindow,runtime:Runtime,velocity:Option<Velocity>)->AppResult<bool> {
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
        let origin=((f64::from(point.0)+(old_body.x-body.x)*scale).round() as i32,(f64::from(point.1)+(old_body.y-body.y)*scale).round() as i32);
        let ceiling=f64::from(area.y)-body.y*scale+4.0*scale;
        let floor=f64::from(area.y)+f64::from(area.height)-(body.y+body.height)*scale;
        if floor<ceiling{return Ok(None)}
        let left=f64::from(area.x)-body.x*scale+4.0*scale;
        let right=f64::from(area.x)+f64::from(area.width)-(body.x+body.width)*scale-4.0*scale;
        if right<left{return Ok(None)}
        let trajectory=Trajectory::new((f64::from(origin.0),f64::from(origin.1)),(left,right,ceiling,floor),velocity,scale);
        let token=managed.toss_epoch.fetch_add(1,Ordering::SeqCst)+1;
        managed.epoch.fetch_add(1,Ordering::SeqCst);s.stop_motion();s.target=None;
        s.drag_velocity=None;s.preferences.dock=Dock::None;s.preferences.floor_inset=0.0;save(&store.connection,&s.preferences)?;
        s.toss=Some(Toss{velocity:Velocity{x:trajectory.velocity.x/scale,y:trajectory.velocity.y/scale},phase:trajectory.phase(),persona_id:view.persona_id,session_id:view.session_id.unwrap_or_default(),context_revision:view.context_revision,body,controls_below:controls_below(f64::from(origin.1)+body.y*scale,area.y,scale)});
        s.revision+=1;
        Ok(Some((window.clone(),token,s.snapshot(),trajectory,origin)))
    })).await.map_err(|_|AppError::storage())??;
    let Some((window,token,snapshot,trajectory,point))=prepared else{return Ok(false)};
    window.set_position(PhysicalPosition::new(trajectory.x.round() as i32,trajectory.y.round() as i32)).map_err(|_|AppError::storage())?;
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
fn run(window:WebviewWindow,runtime:Runtime,token:u64,mut trajectory:Trajectory,_point:(i32,i32),size:PhysicalSize<u32>,area:Area,scale:f64) {
    let started=Instant::now();let mut last_check=Instant::now()-Duration::from_secs(1);let mut last_position=None;let mut last_visual=Instant::now()-Duration::from_secs(1);
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
        let seconds=started.elapsed().as_secs_f64();trajectory.advance(seconds);let y=trajectory.y;let phase=trajectory.phase();
        let position=(trajectory.x.round() as i32,y.round() as i32);
        if last_position!=Some(position){if window.set_position(PhysicalPosition::new(position.0,position.1)).is_err(){clear(&window,token);return}last_position=Some(position);}
        if let Ok(mut s)=desktop.inner.lock(){
            let changed=if let Some(t)=s.toss.as_mut(){let below=controls_below(y+t.body.y*scale,area.y,scale);let changed=t.phase!=phase||t.controls_below!=below||(phase!=Phase::Landed&&last_visual.elapsed()>=Duration::from_millis(32));t.phase=phase;t.controls_below=below;t.velocity=Velocity{x:trajectory.velocity.x/scale,y:trajectory.velocity.y/scale};changed}else{false};
            if changed{last_visual=Instant::now();s.revision+=1;let snapshot=s.snapshot();drop(s);publish(window.app_handle(),&snapshot);}
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
    fn missing_release_waits_for_confirmed_mouse_up_and_resets_when_pressed() {
        let now=Instant::now();let mut release=ReleaseWatch::default();
        assert!(!release.overdue(now,Some(false)));assert!(!release.overdue(now+Duration::from_millis(95),Some(false)));assert!(release.overdue(now+Duration::from_millis(96),Some(false)));
        assert!(!release.overdue(now+Duration::from_millis(100),Some(true)));assert!(!release.overdue(now+Duration::from_millis(130),Some(false)));assert!(!release.overdue(now+Duration::from_millis(250),None));
    }
    #[test]
    fn upward_release_uses_vector_and_rejects_downward_slow_and_stale_motion() {
        let now=Instant::now();
        for(scale,dy,dx,expected)in[(1.0,-120,10,true),(1.5,-180,15,true),(2.0,-240,20,true),(1.0,-25,0,false),(1.0,120,0,false),(1.0,-120,180,true),(1.0,-15,-160,true),(1.0,0,180,false)] {
            let mut trace=DragTrace::default();
            for i in 0..=5 {trace.record(now+Duration::from_millis(i*20),(500+dx*i as i32/5,500+dy*i as i32/5));}
            let velocity=trace.upward_velocity(now+Duration::from_millis(100),scale);
            assert_eq!(velocity.is_some(),expected);if let Some(v)=velocity{assert!(v.y<0.0&&v.x.hypot(v.y)<=1800.001);}
            assert!(trace.upward_velocity(now+Duration::from_millis(200),scale).is_none());
        }
        let mut stopped=DragTrace::default();
        for i in 0..=10 {stopped.record(now+Duration::from_millis(i*20),(500,if i<3{500-i as i32*30}else{410}));}
        assert!(stopped.upward_velocity(now+Duration::from_millis(200),1.0).is_none());
    }
    #[test]
    fn vector_path_has_gravity_wall_and_ceiling_restitution_and_three_second_landing() {
        for scale in [1.0,1.5,2.0] {
            let mut t=Trajectory::new((-80.0*scale,-80.0*scale),(-100.0*scale,300.0*scale,-100.0*scale,600.0*scale),Velocity{x:-1000.0,y:-1200.0},scale);
            let mut bounced_x=false;let mut bounced_top=false;
            for i in 1..=600 {
                let old=t.velocity;t.advance(i as f64/120.0);
                assert!(t.x>=t.left&&t.x<=t.right&&t.y>=t.ceiling&&t.y<=t.floor);
                if old.x<0.0&&t.velocity.x>0.0 {bounced_x=true;assert!((t.velocity.x+old.x*RESTITUTION).abs()<0.001);}
                if old.y<0.0&&t.velocity.y>0.0&&t.y< -50.0*scale {bounced_top=true;assert!(t.velocity.y<1200.0*scale);}
            }
            assert!(bounced_x&&bounced_top);assert_eq!(t.phase(),Phase::Landed);
            let at=t.landed_at.unwrap();assert!(!t.settled(at+2.999));assert!(t.settled(at+3.0));
        }
        let mut t=Trajectory::new((100.0,300.0),(0.0,2000.0,0.0,2000.0),Velocity{x:500.0,y:-1100.0},1.0);
        t.advance(0.1);assert!((t.x-150.0).abs()<0.01);assert!((t.y-204.0).abs()<0.01);
        t.advance(0.2);assert!((t.y-136.0).abs()<0.01);assert_eq!(t.phase(),Phase::Up);
    }
    #[test]
    fn away_error_and_changed_context_cannot_continue_toss() {
        let mut view=View{persona_id:"1".into(),session_id:Some("2".into()),context_revision:3,revision:1,activity:crate::pet::controller::Activity::Normal,expression:"x".into(),intimacy:Some(0),sleep_until:None,error:None,can_interact:true,can_wander:true};
        let toss=Toss{velocity:Velocity::default(),phase:Phase::Down,persona_id:"1".into(),session_id:"2".into(),context_revision:3,body:Rect{x:0.0,y:0.0,width:172.0,height:172.0},controls_below:false};
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
