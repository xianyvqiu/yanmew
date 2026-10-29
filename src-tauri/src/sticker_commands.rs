//! 聊天表情包管理只开放设置窗口；聊天模型走独立的已绑定库工具。
use crate::{error::{parse_id,AppError,AppResult},runtime::Runtime,stickers};
use tauri::{AppHandle,Emitter,State,WebviewWindow};
fn allow(window:&WebviewWindow)->AppResult<()> {if window.label()=="settings"{Ok(())}else{Err(AppError::new("PERMISSION_DENIED","请在设置中管理聊天表情包"))}}
#[tauri::command]
pub async fn query_chat_stickers(window:WebviewWindow,runtime:State<'_,Runtime>,input:stickers::Query)->AppResult<serde_json::Value> {
    allow(&window)?;let rt=runtime.inner().clone();tauri::async_runtime::spawn_blocking(move||rt.read(|s,_|stickers::query(&s.connection,input))).await.map_err(|_|AppError::storage())?
}
#[tauri::command]
pub async fn mutate_chat_stickers(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,input:stickers::Mutation)->AppResult<serde_json::Value> {
    allow(&window)?;let rt=runtime.inner().clone();let value=tauri::async_runtime::spawn_blocking(move||rt.read(|s,_|{let deleting=matches!(&input,stickers::Mutation::Delete{..});let result=stickers::mutate(&mut s.connection,input)?;if deleting{if let Ok(paths)=&rt.paths{let _=crate::chat_images::cleanup(&s.connection,paths);}}Ok(result)})).await.map_err(|_|AppError::storage())??;
    let _=app.emit("chat-stickers-changed",());Ok(value)
}
#[tauri::command]
pub async fn import_chat_sticker(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,library_id:String,input:crate::chat_images::Input)->AppResult<()> {
    allow(&window)?;let rt=runtime.inner().clone();tauri::async_runtime::spawn_blocking(move||{
        let id=parse_id(&library_id)?;let paths=rt.paths.as_ref().map_err(Clone::clone)?;
        let staged=crate::chat_images::stage(paths,vec![input])?;
        rt.read(|s,_|stickers::import(&mut s.connection,paths,id,staged))
    }).await.map_err(|_|AppError::storage())??;let _=app.emit("chat-stickers-changed",());Ok(())
}
#[tauri::command]
pub async fn get_chat_sticker_image(window:WebviewWindow,runtime:State<'_,Runtime>,id:String)->AppResult<String> {
    allow(&window)?;let rt=runtime.inner().clone();tauri::async_runtime::spawn_blocking(move||rt.read(|s,_|stickers::image(&s.connection,rt.paths.as_ref().map_err(Clone::clone)?,parse_id(&id)?))).await.map_err(|_|AppError::storage())?
}
