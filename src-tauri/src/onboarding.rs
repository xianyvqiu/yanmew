//! 新手教程的独立透明层：只接受固定主窗口，插件没有教程或业务权限。
use crate::error::{AppError,AppResult};
use serde::{Serialize,Deserialize};
use tauri::{AppHandle,Manager,Emitter,WebviewWindow,WebviewWindowBuilder,WebviewUrl};
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct GuideState {pub token:u64,pub step:usize,pub viewing:bool,pub target:String,pub page:String,pub title:String,pub text:String,pub total:usize,pub busy:bool,pub error:String}
#[derive(Clone,Copy,PartialEq,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rect {pub left:f64,pub top:f64,pub width:f64,pub height:f64}
impl Rect {fn valid(self)->bool{[self.left,self.top,self.width,self.height].iter().all(|v|v.is_finite()&&v.abs()<100000.0)&&self.width>=0.0&&self.height>=0.0}}
#[derive(Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct View {#[serde(skip)]scale:f64,pub theme:String,pub state:GuideState,pub area:Option<Rect>,pub bounds:Rect,pub panel:Rect}
#[derive(Default)]
pub struct Guide {state:std::sync::Mutex<Option<GuideState>>,view:std::sync::Mutex<Option<View>>}
fn permission()->AppError{AppError::new("PERMISSION_DENIED","此窗口不能操作新手教程")}
fn failure(_:tauri::Error)->AppError{AppError::new("ONBOARDING_WINDOW_ERROR","教程层显示失败，请重试或跳过教程")}
fn trusted(window:&WebviewWindow)->AppResult<()>{if crate::desktop_windows::is_application_window(window.label())||window.label()=="onboarding"{Ok(())}else{Err(permission())}}
#[tauri::command]
pub fn get_onboarding_state(window:WebviewWindow,app:AppHandle)->AppResult<Option<GuideState>>{trusted(&window)?;Ok(app.state::<Guide>().state.lock().map_err(|_|AppError::storage())?.clone())}
#[tauri::command]
pub fn get_onboarding_view(window:WebviewWindow,app:AppHandle)->AppResult<Option<View>>{if window.label()!="onboarding"{return Err(permission())}Ok(app.state::<Guide>().view.lock().map_err(|_|AppError::storage())?.clone())}
#[tauri::command]
pub async fn set_onboarding_state(window:WebviewWindow,app:AppHandle,state:Option<GuideState>)->AppResult<()> {
 if window.label()!="settings"{return Err(permission())}
 if let Some(s)=&state {if !matches!(s.target.as_str(),"settings"|"main"|"chat")||s.step>=s.total||s.total>80||s.title.len()>500||s.text.len()>4000||s.error.len()>4000{return Err(AppError::validation("guide","教程步骤无效"))}}
 let previous=app.state::<Guide>().state.lock().map_err(|_|AppError::storage())?.clone();
 if let Some(s)=&state {if previous.as_ref().map(|p|p.target.as_str())!=Some(s.target.as_str()) {
   match s.target.as_str(){"chat"=>crate::desktop_windows::open(app.clone(),crate::desktop_windows::Page::Chat).await?,"settings"=>{window.show().map_err(failure)?;window.unminimize().map_err(failure)?;window.set_focus().map_err(failure)?;},"main"=>{if let Some(pet)=app.get_webview_window("main"){pet.show().map_err(failure)?;pet.unminimize().map_err(failure)?;pet.set_focus().map_err(failure)?;}},_=>()}
 }}
 *app.state::<Guide>().state.lock().map_err(|_|AppError::storage())?=state.clone();
 // 同一窗口内保留提示层，待新位置就绪后原位更新，避免每一步先消失再出现。
 let reset=state.is_none()||previous.as_ref().zip(state.as_ref()).is_some_and(|(a,b)|a.target!=b.target||a.token!=b.token);
 if reset {
  if let Some(layer)=app.get_webview_window("onboarding"){layer.hide().map_err(failure)?;}
  *app.state::<Guide>().view.lock().map_err(|_|AppError::storage())?=None;
 }
 app.emit("onboarding-state-changed",()).map_err(failure)
}
#[tauri::command]
pub fn onboarding_action(window:WebviewWindow,app:AppHandle,token:u64,step:usize,action:String)->AppResult<()> {
 trusted(&window)?;
 if !["view","next","previous","skip"].contains(&action.as_str()){return Err(permission())}
 let current=app.state::<Guide>().state.lock().map_err(|_|AppError::storage())?.clone();
 if current.as_ref().is_some_and(|s|s.token==token&&s.step==step&&(window.label()=="onboarding"||window.label()==s.target)) {
  if let Some(settings)=app.get_webview_window("settings"){settings.emit("onboarding-action",serde_json::json!({"token":token,"step":step,"action":action})).map_err(failure)?;}
 }
 Ok(())
}
/// 提示框与高亮区域分开排布；小窗优先在窗口外侧显示，屏幕不足时允许提示框自身滚动。
fn placement(bounds:Rect,area:Option<Rect>,width:f64,height:f64)->Rect {
 let w=320.0_f64.min((width-24.0).max(1.0)); let h=300.0_f64.min((height-24.0).max(1.0));
 let a=area.unwrap_or(bounds);
 let x=if a.left-w-16.0>=12.0{a.left-w-16.0}else if a.left+a.width+w+16.0<=width-12.0{a.left+a.width+16.0}else if bounds.left>=w+16.0{bounds.left-w-12.0}else{(width-w-12.0).max(12.0)};
 let y=if x<a.left+a.width&&x+w>a.left {if a.top+a.height+h+16.0<=height {a.top+a.height+12.0}else{12.0}}else{a.top.clamp(12.0,(height-h-12.0).max(12.0))};
 Rect{left:x,top:y,width:w,height:h}
}
/// 同一步骤保持说明框相对窗口的位置，滚动只更新高亮，避免阅读中反复换边。
fn stable_placement(previous:Option<&View>,state:&GuideState,bounds:Rect,area:Option<Rect>,width:f64,height:f64)->Rect {
 let Some(previous)=previous.filter(|p|p.state.token==state.token&&p.state.step==state.step&&p.state.viewing==state.viewing&&p.state.target==state.target&&p.bounds.width==bounds.width&&p.bounds.height==bounds.height)else{return placement(bounds,area,width,height)};
 let w=320.0_f64.min((width-24.0).max(1.0));let h=300.0_f64.min((height-24.0).max(1.0));
 Rect{left:(previous.panel.left+bounds.left-previous.bounds.left).clamp(12.0,(width-w-12.0).max(12.0)),top:(previous.panel.top+bounds.top-previous.bounds.top).clamp(12.0,(height-h-12.0).max(12.0)),width:w,height:h}
}
#[tauri::command]
pub async fn show_onboarding_overlay(window:WebviewWindow,app:AppHandle,token:u64,step:usize,viewing:bool,area:Option<Rect>,theme:Option<String>)->AppResult<()> {
 trusted(&window)?;
 if area.is_some_and(|a|!a.valid()){return Err(AppError::validation("area","教程区域无效"))}
 let current=app.state::<Guide>().state.lock().map_err(|_|AppError::storage())?.clone();
 let Some(state)=current.filter(|s|s.token==token&&s.step==step&&s.viewing==viewing&&s.target==window.label()) else{return Ok(())};
 let owner=app.clone();let (tx,rx)=tokio::sync::oneshot::channel();
 app.run_on_main_thread(move||{let result=(||{
  if !owner.state::<Guide>().state.lock().map_err(|_|AppError::storage())?.as_ref().is_some_and(|s|s.token==state.token&&s.step==state.step&&s.viewing==state.viewing){return Ok(())}
  let monitor=window.current_monitor().map_err(failure)?.or(owner.primary_monitor().map_err(failure)?).ok_or_else(AppError::storage)?;
  let scale=monitor.scale_factor();let work=monitor.work_area();let pos=window.inner_position().map_err(failure)?;let size=window.inner_size().map_err(failure)?;let source_scale=window.scale_factor().map_err(failure)?;
  let bounds=Rect{left:(pos.x-work.position.x)as f64/scale,top:(pos.y-work.position.y)as f64/scale,width:size.width as f64/scale,height:size.height as f64/scale};
  let area=area.map(|a|Rect{left:bounds.left+a.left*source_scale/scale,top:bounds.top+a.top*source_scale/scale,width:a.width*source_scale/scale,height:a.height*source_scale/scale});
  let previous=owner.state::<Guide>().view.lock().map_err(|_|AppError::storage())?.clone();
  let panel=stable_placement(previous.as_ref(),&state,bounds,area,work.size.width as f64/scale,work.size.height as f64/scale);
  let view=View{scale,theme:if theme.as_deref()==Some("dark"){"dark".into()}else{"light".into()},panel,bounds,area,state};
  let existed=owner.get_webview_window("onboarding").is_some();
  let layer=match owner.get_webview_window("onboarding"){Some(w)=>w,None=>WebviewWindowBuilder::new(&owner,"onboarding",WebviewUrl::App("index.html".into())).title("颜喵 · 新手引导").transparent(true).decorations(false).shadow(false).always_on_top(false).focused(false).skip_taskbar(true).resizable(false).visible(false).background_color(tauri::window::Color(0,0,0,0)).build().map_err(failure)?};
  attach_owner(&layer,&window)?;
  let reposition=layer.outer_position().map_err(failure)?!=work.position;
  let resize=layer.outer_size().map_err(failure)?!=work.size;
  if reposition{layer.set_position(tauri::Position::Physical(work.position)).map_err(failure)?;}
  if resize{layer.set_size(tauri::Size::Physical(work.size)).map_err(failure)?;}
  if !existed||reposition||resize||previous.as_ref().is_none_or(|p|p.scale!=scale||p.bounds!=view.bounds||p.area!=view.area||p.panel!=view.panel){apply_region(&layer,&view,scale)?;}
  *owner.state::<Guide>().view.lock().map_err(|_|AppError::storage())?=Some(view);
  layer.emit("onboarding-view-changed",()).map_err(failure)?;
  sync_visibility(&owner)?;Ok(())
 })();let _=tx.send(result);}).map_err(failure)?;
 rx.await.map_err(|_|AppError::storage())?
}
/// 窗口切换不推进教程；只在目标窗口或教程本身位于前台时显示。
fn should_show(target_visible:bool,target_minimized:bool,target_foreground:bool,layer_foreground:bool)->bool {
 target_visible&&!target_minimized&&(target_foreground||layer_foreground)
}
#[cfg(windows)]
fn attach_owner(layer:&WebviewWindow,target:&WebviewWindow)->AppResult<()> {
 native::attach_owner(layer.hwnd().map_err(failure)?,target.hwnd().map_err(failure)?)
}
#[cfg(not(windows))]
fn attach_owner(_layer:&WebviewWindow,_target:&WebviewWindow)->AppResult<()>{Ok(())}
fn sync_visibility(app:&AppHandle)->AppResult<()> {
 let state=app.state::<Guide>().state.lock().map_err(|_|AppError::storage())?.clone();
 let view=app.state::<Guide>().view.lock().map_err(|_|AppError::storage())?.clone();
 let Some(layer)=app.get_webview_window("onboarding")else{return Ok(())};
 let Some(state)=state.filter(|s|view.as_ref().is_some_and(|v|v.state.token==s.token&&v.state.step==s.step&&v.state.viewing==s.viewing&&v.state.target==s.target))else{layer.hide().map_err(failure)?;return Ok(())};
 let Some(target)=app.get_webview_window(&state.target)else{layer.hide().map_err(failure)?;return Ok(())};
 #[cfg(windows)]
 {
  let hwnd=layer.hwnd().map_err(failure)?;let target_hwnd=target.hwnd().map_err(failure)?;
  native::sync_visibility(hwnd,target_hwnd)?;
 }
 #[cfg(not(windows))]
 {
  if should_show(target.is_visible().map_err(failure)?,target.is_minimized().map_err(failure)?,target.is_focused().map_err(failure)?,layer.is_focused().map_err(failure)?){layer.show().map_err(failure)?;}else{layer.hide().map_err(failure)?;}
 }
 Ok(())
}
pub fn window_event(app:&AppHandle,label:&str,event:&tauri::WindowEvent) {
 if !matches!(event,tauri::WindowEvent::Focused(_)|tauri::WindowEvent::Resized(_)|tauri::WindowEvent::Destroyed){return;}
 let target=app.state::<Guide>().state.lock().ok().and_then(|s|s.as_ref().map(|s|s.target.clone()));
 if target.as_deref()!=Some(label)&&label!="onboarding"{return;}
 // 排到本轮焦点消息之后，避免 owner 失焦到教程自身时短暂隐藏；不主动抢焦点。
 let owner=app.clone();let _=app.run_on_main_thread(move||{
  if let Err(error)=sync_visibility(&owner){
   if let Some(layer)=owner.get_webview_window("onboarding"){let _=layer.hide();}
   eprintln!("更新教程可见性失败：{error}");
  }
 });
}
#[cfg(windows)]
mod native {
 use super::*;
 use windows::Win32::{Foundation::{HWND,GetLastError,SetLastError,WIN32_ERROR},UI::WindowsAndMessaging::*};
 fn failed()->AppError{AppError::new("ONBOARDING_WINDOW_ERROR","教程窗口层级设置失败，请重试或跳过教程")}
 pub(super) fn attach_owner(layer:HWND,target:HWND)->AppResult<()> {
  unsafe {
   if GetWindowLongPtrW(layer,GWLP_HWNDPARENT)!=target.0 as isize {
    SetLastError(WIN32_ERROR(0));
    if SetWindowLongPtrW(layer,GWLP_HWNDPARENT,target.0 as isize)==0&&GetLastError().0!=0{return Err(failed());}
   }
   // 桌宠允许置顶，教程只继承其层级；普通设置/聊天窗口不进入全局置顶组。
   let order=if GetWindowLongPtrW(target,GWL_EXSTYLE)&WS_EX_TOPMOST.0 as isize!=0{HWND_TOPMOST}else{HWND_NOTOPMOST};
   SetWindowPos(layer,Some(order),0,0,0,0,SWP_NOMOVE|SWP_NOSIZE|SWP_NOACTIVATE|SWP_NOOWNERZORDER).map_err(|_|failed())?;
  }
  Ok(())
 }
 pub(super) fn sync_visibility(layer:HWND,target:HWND)->AppResult<()> {
  unsafe {
   let foreground=GetForegroundWindow();
   let visible=should_show(IsWindowVisible(target).as_bool(),IsIconic(target).as_bool(),foreground==target,foreground==layer);
   if visible {
    if !IsWindowVisible(layer).as_bool(){let _=ShowWindow(layer,SW_SHOWNOACTIVATE);}
   }else if IsWindowVisible(layer).as_bool(){let _=ShowWindow(layer,SW_HIDE);}
  }
  Ok(())
 }
 #[cfg(test)]
 mod tests {
  use super::*;use windows::core::w;
  #[test]fn owner_changes_do_not_activate_or_demote_the_pet() {
   unsafe {
    let settings=CreateWindowExW(WINDOW_EX_STYLE(0),w!("STATIC"),w!("guide-test-settings"),WS_OVERLAPPED,0,0,10,10,None,None,None,None).unwrap();
    let pet=CreateWindowExW(WS_EX_TOPMOST,w!("STATIC"),w!("guide-test-pet"),WS_OVERLAPPED,0,0,10,10,None,None,None,None).unwrap();
    let layer=CreateWindowExW(WINDOW_EX_STYLE(0),w!("STATIC"),w!("guide-test-layer"),WS_POPUP,0,0,10,10,None,None,None,None).unwrap();
    let foreground=GetForegroundWindow();
    attach_owner(layer,pet).unwrap();assert_eq!(GetWindow(layer,GW_OWNER).unwrap(),pet);assert_ne!(GetWindowLongPtrW(pet,GWL_EXSTYLE)&WS_EX_TOPMOST.0 as isize,0);
    attach_owner(layer,settings).unwrap();assert_eq!(GetWindow(layer,GW_OWNER).unwrap(),settings);assert_eq!(GetWindowLongPtrW(layer,GWL_EXSTYLE)&WS_EX_TOPMOST.0 as isize,0);assert_ne!(GetWindowLongPtrW(pet,GWL_EXSTYLE)&WS_EX_TOPMOST.0 as isize,0);
    sync_visibility(layer,settings).unwrap();assert!(!IsWindowVisible(layer).as_bool());assert_eq!(GetForegroundWindow(),foreground);
    let _=DestroyWindow(layer);let _=DestroyWindow(pet);let _=DestroyWindow(settings);
   }
  }
 }
}
#[cfg(test)]mod visibility_tests {
 use super::*;
 #[test]fn external_foreground_or_hidden_owner_hides_guide_but_guide_focus_keeps_it() {
  assert!(should_show(true,false,true,false));assert!(should_show(true,false,false,true));
  assert!(!should_show(true,false,false,false));assert!(!should_show(true,true,true,false));assert!(!should_show(false,false,false,true));
 }
}
#[cfg(windows)]
fn apply_region(window:&WebviewWindow,view:&View,scale:f64)->AppResult<()> {
 use windows::Win32::Graphics::Gdi::{CreateRectRgn,CombineRgn,SetWindowRgn,DeleteObject,RGN_DIFF,RGN_OR};
 let region=|r:Rect|unsafe{CreateRectRgn((r.left*scale).floor()as i32,(r.top*scale).floor()as i32,((r.left+r.width)*scale).ceil()as i32,((r.top+r.height)*scale).ceil()as i32)};
 unsafe {
  let shape=region(view.bounds);
  if let Some(a)=view.area {let hole=region(a);CombineRgn(Some(shape),Some(shape),Some(hole),RGN_DIFF);let _=DeleteObject(hole.into());}
  let panel=region(view.panel);CombineRgn(Some(shape),Some(shape),Some(panel),RGN_OR);let _=DeleteObject(panel.into());
  if SetWindowRgn(window.hwnd().map_err(failure)?,Some(shape),false)==0{let _=DeleteObject(shape.into());return Err(AppError::new("ONBOARDING_WINDOW_ERROR","教程透明区域设置失败"))}
 }
 Ok(())
}
#[cfg(not(windows))]
fn apply_region(_window:&WebviewWindow,_view:&View,_scale:f64)->AppResult<()>{Ok(())}
pub fn stop(app:&AppHandle){if let Ok(mut state)=app.state::<Guide>().state.lock(){*state=None;}if let Some(layer)=app.get_webview_window("onboarding"){let _=layer.hide();}let _=app.emit("onboarding-state-changed",());}
#[cfg(test)]mod tests{use super::*;#[test]fn positions_are_bounded_and_prefer_beside_pet(){let b=Rect{left:900.,top:600.,width:320.,height:280.};let a=Rect{left:980.,top:640.,width:100.,height:150.};let p=placement(b,Some(a),1280.,900.);assert!(p.left+p.width<=a.left);assert!(p.top+p.height<=900.);assert!(!Rect{left:f64::NAN,..a}.valid());}}

#[cfg(test)]mod scroll_tests {
 use super::*;
 fn state()->GuideState{GuideState{token:1,step:0,viewing:true,target:"settings".into(),page:"提供商".into(),title:"入口".into(),text:"说明".into(),total:24,busy:false,error:String::new()}}
 #[test]fn scrolling_and_clipped_target_keep_reading_panel_stable(){
  let bounds=Rect{left:500.,top:100.,width:960.,height:730.};let area=Rect{left:740.,top:220.,width:680.,height:400.};let state=state();
  let panel=placement(bounds,Some(area),1920.,1080.);let view=View{scale:1.,theme:"dark".into(),state:state.clone(),area:Some(area),bounds,panel};
  for next in [Some(Rect{top:120.,height:150.,..area}),None,Some(Rect{top:600.,height:180.,..area})]{assert_eq!(stable_placement(Some(&view),&state,bounds,next,1920.,1080.),panel);}
 }
 #[test]fn new_step_repositions_but_window_movement_and_small_screen_remain_bounded(){
  let bounds=Rect{left:500.,top:100.,width:960.,height:730.};let area=Rect{left:740.,top:220.,width:680.,height:400.};let state=state();let panel=placement(bounds,Some(area),1920.,1080.);let view=View{scale:1.,theme:"dark".into(),state:state.clone(),area:Some(area),bounds,panel};
  let moved=stable_placement(Some(&view),&state,Rect{left:600.,top:180.,..bounds},None,1920.,1080.);assert_eq!(moved.left,panel.left+100.);assert_eq!(moved.top,panel.top+80.);
  let mut next=state.clone();next.step=1;let next_area=Rect{left:1300.,top:600.,..area};assert_eq!(stable_placement(Some(&view),&next,bounds,Some(next_area),1920.,1080.),placement(bounds,Some(next_area),1920.,1080.));
  let resized=Rect{width:600.,height:600.,..bounds};let resized_area=Rect{left:620.,width:450.,..area};assert_eq!(stable_placement(Some(&view),&state,resized,Some(resized_area),1920.,1080.),placement(resized,Some(resized_area),1920.,1080.));
  let small=stable_placement(Some(&view),&state,bounds,None,640.,480.);assert!(small.left>=12.&&small.top>=12.&&small.left+small.width<=628.&&small.top+small.height<=468.);
 }
}
