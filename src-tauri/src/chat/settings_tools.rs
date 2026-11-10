//! 普通聊天的有限设置与签到工具；不开放任意配置、文件路径或奖励写入。
use super::*;
use crate::{ai::provider::ToolCall, events::ChatToolEventSink, persona::background, runtime::Store};
use base64::Engine;
use serde_json::{json, Value};
use std::collections::HashMap;

pub const RULES: &str = "主题、聊天外观和签到工具仅用于普通聊天。查询可以按需执行；只有用户明确要求时才修改主题/外观或签到，写入参数confirmedByUser必须反映真实用户要求。询问今天是否签到不等于要求签到，不自动代签或补签。签到使用工具返回的本地日期和实际grantedCoins，newlySignedIn=false或alreadyApplied=true表示没有再次领奖，不编造奖励。整体主题作用于颜喵主程序；聊天外观按当前人设保存，该人设所有会话共用，修改时说明此范围。修改外观前先get_chat_appearance，只提交用户要求的字段；颜色null表示跟随人设颜色。背景图片只能使用get_chat_appearance返回的本轮用户上传图片序号，不用历史图片、AI生成图、本机路径或网址；不代为生成图片或调用付费服务。冲突时说明情况，重新读取后等待用户决定，不静默覆盖。";

pub fn is_write(name: &str) -> bool {
    matches!(name, "set_ui_theme" | "update_chat_appearance" | "daily_sign_in")
}
pub fn definitions() -> Vec<Value> {
    let confirm = json!({"type":"boolean","description":"仅在用户明确要求执行本操作时为true"});
    let color = json!({"type":["string","null"],"description":"#RRGGBB；null恢复跟随人设标记色，省略则不改"});
    [
        ("get_ui_theme", "查询颜喵整体主题模式，不读取其他配置。", json!({}), vec![]),
        ("set_ui_theme", "按用户要求设置颜喵整体亮暗主题，不改变小游戏自身主题。", json!({"mode":{"type":"string","enum":["light","dark","system"]},"confirmedByUser":confirm.clone()}), vec!["mode","confirmedByUser"]),
        ("get_chat_appearance", "读取当前人设共用的聊天外观和本轮用户上传图片序号；修改前先读取。", json!({}), vec![]),
        ("update_chat_appearance", "按用户要求修改当前人设共用的聊天背景/气泡/文字颜色，保留未指定项。", json!({"opacity":{"type":"integer","minimum":0,"maximum":100},"blur":{"type":"integer","minimum":0,"maximum":40},"bubbleStyle":{"type":"string","enum":["solid","transparent","outline","frosted"]},"userTextColor":color.clone(),"assistantTextColor":color,"backgroundImageIndex":{"type":"integer","minimum":1,"description":"本轮用户上传图的一基序号，先查询，不接受路径或URL"},"clearBackground":{"type":"boolean","description":"清除背景并恢复背景透明度85、模糊度0；不能同时传图片/透明度/模糊度"},"confirmedByUser":confirm.clone()}), vec!["confirmedByUser"]),
        ("get_daily_sign_in_status", "查询今天是否签到、当前喵币余额及签到奖励规则，不领取。", json!({}), vec![]),
        ("daily_sign_in", "用户明确要求时领取今天签到奖励；已签到不会再领奖，返回真实领取结果。", json!({"confirmedByUser":confirm}), vec!["confirmedByUser"]),
    ].into_iter().map(|(name, description, properties, required)| json!({"type":"function","function":{"name":name,"description":description,"parameters":{"type":"object","properties":properties,"required":required,"additionalProperties":false}}})).collect()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Confirm { confirmed_by_user: bool }
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Theme { mode: String, confirmed_by_user: bool }
#[derive(Default)]
enum ColorPatch { #[default] Keep, Set(Option<String>) }
impl<'de> Deserialize<'de> for ColorPatch {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer:D)->Result<Self,D::Error> {
        Option::<String>::deserialize(deserializer).map(Self::Set)
    }
}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct AppearanceInput {
    opacity: Option<i64>, blur: Option<i64>, bubble_style: Option<String>,
    #[serde(default)] user_text_color: ColorPatch,
    #[serde(default)] assistant_text_color: ColorPatch,
    background_image_index: Option<usize>,
    #[serde(default)] clear_background: bool,
    confirmed_by_user: bool,
}
fn parse<T: serde::de::DeserializeOwned>(value:&Value)->AppResult<T> {
    serde_json::from_value(value.clone()).map_err(|_|AppError::validation("arguments","工具参数缺失、含未知字段或格式不正确"))
}
fn confirm(value:bool)->AppResult<()> {
    if value {Ok(())} else {Err(AppError::new("USER_CONFIRMATION_REQUIRED","用户尚未明确要求本次操作，未修改设置或签到"))}
}
fn status(value:&crate::models::SignInStatus)->Value {
    json!({"localDate":value.local_date,"signedIn":value.signed_in,"balance":value.wallet.coins,"rewardMin":value.reward_min,"rewardMax":value.reward_max,"festivals":value.festivals})
}
fn appearance(value:&background::Background)->Value {
    json!({"scope":"persona","personaId":value.persona_id,"hasBackground":value.file.is_some(),"opacity":value.opacity,"blur":value.blur,"bubbleStyle":value.appearance.bubble_style,"userTextColor":value.appearance.user_text_color,"assistantTextColor":value.appearance.assistant_text_color})
}
pub struct Tools {
    runtime: Runtime, lease: Arc<RequestLease>, persona: i64, session: i64, revision: u64,
    image_message: Option<i64>, read_only: bool, events: Arc<dyn ChatToolEventSink>,
    observed: Option<background::Background>,
    calls: HashMap<String,(String,Value,Option<Value>)>,
}
impl Tools {
    pub fn new(runtime:&Runtime,lease:&Arc<RequestLease>,persona:i64,session:i64,revision:u64,image_message:Option<i64>,read_only:bool,events:Arc<dyn ChatToolEventSink>)->Self {
        Self {runtime:runtime.clone(),lease:lease.clone(),persona,session,revision,image_message,read_only,events,observed:None,calls:HashMap::new()}
    }
    fn guard(&self,store:&Store)->AppResult<()> {
        self.lease.require_scope(&self.runtime.requests,&format!("chat:{}",self.session),self.persona)?;
        self.runtime.jobs_cancel.check()?;
        coordinator::require_context(store,self.persona,self.session,self.revision)
    }
    fn uploads(&self,store:&Store)->AppResult<Vec<crate::chat_images::Image>> {
        let Some(message)=self.image_message else{return Ok(vec![])};
        let valid:bool=store.connection.query_row("SELECT EXISTS(SELECT 1 FROM messages WHERE id=?1 AND session_id=?2 AND role='user')",rusqlite::params![message,self.session],|r|r.get(0))?;
        if !valid{return Err(AppError::new("CONTEXT_CHANGED","本轮上传图片不属于当前用户消息"));}
        crate::chat_images::list(&store.connection,message)
    }
    pub fn execute(&mut self,call:&ToolCall)->AppResult<Value> {
        self.runtime.read(|s,_|self.guard(s))?;
        if call.id.trim().is_empty(){return Err(AppError::validation("toolCall","工具调用ID不能为空"));}
        if self.read_only&&is_write(&call.name){return Err(AppError::new("PERMISSION_DENIED","重说时不能修改外观、主题或执行签到"));}
        if let Some((name,args,saved))=self.calls.get(&call.id) {
            if name!=&call.name||args!=&call.arguments{return Err(AppError::new("TOOL_CALL_CONFLICT","同一调用ID被用于其他操作或参数，未执行"));}
            if let Some(saved)=saved {
                let mut value=saved.clone();value["alreadyApplied"]=json!(true);
                if call.name=="daily_sign_in"{value["newlySignedIn"]=json!(false);value["grantedCoins"]=Value::Null;}
                return Ok(value);
            }
        }
        self.calls.insert(call.id.clone(),(call.name.clone(),call.arguments.clone(),None));
        let runtime=self.runtime.clone();
        let value=match call.name.as_str() {
            "get_ui_theme"=>{let _:Empty=parse(&call.arguments)?;runtime.read(|s,_|{self.guard(s)?;Ok(json!({"mode":crate::config::get(&s.connection)?["theme_mode"],"scope":"application"}))})?},
            "set_ui_theme"=>{
                let input:Theme=parse(&call.arguments)?;confirm(input.confirmed_by_user)?;
                if !["light","dark","system"].contains(&input.mode.as_str()){return Err(AppError::validation("mode","主题只能是light、dark或system"));}
                let changed=runtime.read(|s,_|{self.guard(s)?;let before=crate::config::get(&s.connection)?["theme_mode"].clone();if before!=input.mode{crate::config::save(&mut s.connection,[("theme_mode".into(),input.mode.clone())].into_iter().collect())?;}Ok(before!=input.mode)})?;
                if changed{self.events.configuration_changed();}
                json!({"mode":input.mode,"scope":"application","changed":changed,"alreadyApplied":false})
            },
            "get_daily_sign_in_status"=>{let _:Empty=parse(&call.arguments)?;runtime.read(|s,_|{self.guard(s)?;Ok(status(&crate::currency::get_sign_in_status(&s.connection,runtime.clock.sample()?.utc,&chrono::Local)?))})?},
            "daily_sign_in"=>{
                let input:Confirm=parse(&call.arguments)?;confirm(input.confirmed_by_user)?;
                let result=runtime.read(|s,_|{self.guard(s)?;let now=runtime.clock.sample()?.utc;let mut random=runtime.random.lock().map_err(|_|AppError::storage())?;crate::currency::daily_sign_in(&mut s.connection,now,&chrono::Local,random.as_mut())})?;
                let changed=result.granted_coins.is_some();if changed{self.events.wallet_changed();}
                json!({"status":status(&result.status),"grantedCoins":result.granted_coins,"newlySignedIn":changed,"alreadyApplied":!changed})
            },
            "get_chat_appearance"=>{
                let _:Empty=parse(&call.arguments)?;
                let (record,images)=runtime.read(|s,_|{self.guard(s)?;Ok((background::snapshot(s,self.persona,Some(self.revision))?,self.uploads(s)?))})?;
                let mut value=appearance(&record);value["uploadedImages"]=json!(images.iter().enumerate().map(|(i,image)|json!({"index":i+1,"filename":image.filename,"mediaType":image.media_type,"backgroundSupported":(["image/png","image/jpeg","image/webp"].contains(&image.media_type.as_str()))})).collect::<Vec<_>>());
                self.observed=Some(record);value
            },
            "update_chat_appearance"=>self.update_appearance(parse(&call.arguments)?)?,
            _=>return Err(AppError::new("UNKNOWN_TOOL","未注册此设置工具")),
        };
        if is_write(&call.name){self.calls.get_mut(&call.id).unwrap().2=Some(value.clone());}
        Ok(value)
    }
    fn update_appearance(&mut self,input:AppearanceInput)->AppResult<Value> {
        confirm(input.confirmed_by_user)?;
        let expected=self.observed.clone().ok_or_else(||AppError::new("APPEARANCE_NOT_READ","请先查询当前聊天外观，再修改指定字段"))?;
        if input.opacity.is_none()&&input.blur.is_none()&&input.bubble_style.is_none()&&matches!(input.user_text_color,ColorPatch::Keep)&&matches!(input.assistant_text_color,ColorPatch::Keep)&&input.background_image_index.is_none()&&!input.clear_background{return Err(AppError::validation("appearance","请指定要修改的外观字段"));}
        if input.clear_background&&(input.background_image_index.is_some()||input.opacity.is_some()||input.blur.is_some()){return Err(AppError::validation("background","清除背景会恢复默认透明度和模糊度，不能同时上传或指定这些参数"));}
        let image=if let Some(index)=input.background_image_index {
            let images=self.runtime.read(|s,_|{self.guard(s)?;self.uploads(s)})?;
            let image=index.checked_sub(1).and_then(|i|images.get(i)).ok_or_else(||AppError::validation("backgroundImageIndex","请选择本轮用户上传的有效图片序号"))?;
            let extension=match image.media_type.as_str(){"image/png"=>"png","image/jpeg"=>"jpg","image/webp"=>"webp",_=>return Err(AppError::validation("backgroundImageIndex","聊天背景当前仅支持PNG、JPEG和WebP"))};
            let paths=self.runtime.paths.as_ref().map_err(Clone::clone)?;
            let path=crate::chat_images::managed_file(paths,&image.image_file)?;
            use std::io::Read;
            let mut bytes=vec![];std::fs::File::open(path).map_err(|_|AppError::storage())?.take(background::MAX_BYTES as u64+1).read_to_end(&mut bytes).map_err(|_|AppError::storage())?;
            if bytes.len()>background::MAX_BYTES{return Err(AppError::validation("image","背景图片不能超过10MB"));}
            Some(background::Upload{file_name:format!("background.{extension}"),base64:base64::engine::general_purpose::STANDARD.encode(bytes)})
        }else{None};
        let mut style=expected.appearance.clone();
        if let Some(value)=input.bubble_style{style.bubble_style=value;}
        if let ColorPatch::Set(value)=input.user_text_color{style.user_text_color=value;}
        if let ColorPatch::Set(value)=input.assistant_text_color{style.assistant_text_color=value;}
        let saved=background::save_guarded(&self.runtime,self.persona,Some(self.revision),background::Input{opacity:input.opacity.unwrap_or(expected.opacity),blur:input.blur.unwrap_or(expected.blur),expected,image,reset:input.clear_background,appearance:Some(style)},|s|self.guard(s))?;
        self.observed=Some(saved.clone());self.events.configuration_changed();
        let mut value=appearance(&saved);value["alreadyApplied"]=json!(false);Ok(value)
    }
}
#[cfg(test)]mod tests;
