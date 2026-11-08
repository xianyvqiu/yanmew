//! 聊天中的表情工具、饰品只读查询及通用生图；沿用活动请求与现有消息图片。
use super::*;
use crate::ai::provider::ToolCall;
use base64::Engine;
use rusqlite::params;
use serde_json::{json, Value};
mod reference;
pub const RULES:&str="生图使用用户配置的服务，可能产生API费用。仅在用户明确要求时生成，每个要求默认一张，不自行付费重试。参考图请求失败时说明原因，不擅自去掉参考图或换模型重试，也不保证生成结果完全一致；配置问题引导设置→提供商→生图。生成图片允许背景。只有实际查看过图片才能描述其内容，不能凭绘图提示词声称看过。生成图像的查看、透明检测和抠图工具只处理当前会话AI生成的图片，不处理用户上传图。";
pub fn definitions(image_mode: bool) -> Vec<Value> {
    let mut values=vec![definition("list_accessories","查询当前会话拥有的饰品及佩戴情况，不修改饰品或喵币。",json!({"query":{"type":"string"},"beforeId":{"type":["string","null"]}}),vec![]),definition("generate_chat_image","生成普通图片并展示在本轮聊天消息中；不制作饰品、不扣喵币。",json!({"prompt":{"type":"string","description":"画面主题、动作、构图等要求"}}),vec!["prompt"])];
    if image_mode {
        values[1]["function"]["parameters"]["properties"]["useReference"] = json!({"type":"boolean","description":"是否实际使用当前人设已接受的默认形象作参考；画自己或要求外观一致时优先true，其他画面默认false。无需先调用看图工具。"});
        values.push(definition("view_pet_base_image","查看当前人设已接受的默认形象，了解真实外观。",json!({}),vec![]));
        values.push(definition("list_pet_expressions","分页查询当前人设全部已启用且已有正式图片的表情，包含自定义和所有动作类别，返回ID、名称和说明。",json!({"query":{"type":"string","description":"名称或说明中的连续关键词，例如开心；不是多个词组成的布尔查询"},"beforeId":{"type":["string","null"]}}),vec![]));
        values.push(definition("set_pet_expression","切换本轮桌宠图片表情。成功后自然回答，不把图片ID、内部动作标识或工具结果写进聊天正文。",json!({"artworkId":{"type":"string"}}),vec!["artworkId"]));
    } else {
        values.push(definition("set_pet_expression","结合人设和当前语境创作并显示临时颜文字，不抽取或改写固定表情库。",json!({"expression":{"type":"string","description":"单行临时颜文字，最多14半角显示宽度，非ASCII按2计"}}),vec!["expression"]));
    }
    values.extend(super::image_tools::definitions());
    values
}
fn definition(name: &str, description: &str, properties: Value, required: Vec<&str>) -> Value {
    json!({"type":"function","function":{"name":name,"description":description,"parameters":{"type":"object","properties":properties,"required":required,"additionalProperties":false}}})
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct List {
    #[serde(default)]
    query: String,
    #[serde(default)]
    before_id: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Set {
    #[serde(default)]
    artwork_id: Option<String>,
    #[serde(default)]
    expression: Option<String>,
}
pub fn local(
    runtime: &Runtime,
    persona: i64,
    session: i64,
    revision: u64,
    call: &ToolCall,
) -> AppResult<Value> {
    runtime.read(|s,_|{
  coordinator::require_context(s,persona,session,revision)?;
  match call.name.as_str(){
   "list_accessories"=>{let a:List=serde_json::from_value(call.arguments.clone()).map_err(|_|AppError::validation("arguments","饰品查询参数无效"))?;let before=a.before_id.as_deref().map(crate::error::parse_id).transpose()?;
    let mut q=s.connection.prepare("SELECT id,name,description,display_text,cost,is_worn,created_at FROM accessories WHERE session_id=?1 AND (?2 IS NULL OR id<?2) AND instr(lower(name||' '||description),lower(?3))>0 ORDER BY id DESC LIMIT 13")?;
    let mut values=q.query_map(params![session,before,a.query],|r|Ok(json!({"id":r.get::<_,i64>(0)?.to_string(),"name":r.get::<_,String>(1)?,"description":r.get::<_,String>(2)?,"fallbackIcon":r.get::<_,String>(3)?,"cost":r.get::<_,i64>(4)?.to_string(),"isWorn":r.get::<_,bool>(5)?,"createdAt":r.get::<_,String>(6)?})))?.collect::<Result<Vec<_>,_>>()?;
    let more=values.len()>12;values.truncate(12);Ok(json!({"items":values,"nextBeforeId":if more{values.last().map(|v|v["id"].clone())}else{None}}))
   },
   "list_pet_expressions"=>{let a:List=serde_json::from_value(call.arguments.clone()).map_err(|_|AppError::validation("arguments","图片表情查询参数无效"))?;let before=a.before_id.as_deref().map(crate::error::parse_id).transpose()?.unwrap_or(i64::MAX);let word=a.query.to_lowercase();let mut items=crate::pet_creation::chat_choices(&s.connection,persona)?.ok_or_else(||AppError::validation("mode","当前为颜文字模式，应临时生成表情，不查询图片库"))?.into_iter().filter(|a|a.id.parse::<i64>().is_ok_and(|id|id<before)&&format!("{} {}",a.name,a.description).to_lowercase().contains(&word)).collect::<Vec<_>>();items.sort_by_key(|a|std::cmp::Reverse(a.id.parse::<i64>().unwrap_or(0)));let more=items.len()>12;items.truncate(12);Ok(json!({"items":items.iter().map(|a|json!({"id":a.id,"name":a.name,"usage":a.description,"fallback":a.name=="自然交谈"||a.kind=="base"})).collect::<Vec<_>>(),"nextBeforeId":if more{items.last().map(|a|a.id.clone())}else{None}}))},
   "set_pet_expression"=>{let a:Set=serde_json::from_value(call.arguments.clone()).map_err(|_|AppError::validation("arguments","表情参数无效"))?;let expression=if let Some(items)=crate::pet_creation::chat_choices(&s.connection,persona)?{if a.expression.is_some(){return Err(AppError::validation("expression","图片模式请选择已有图片ID"));}let id=a.artwork_id.ok_or_else(||AppError::validation("artworkId","请选择图片ID"))?;let item=items.iter().find(|i|i.id==id).ok_or_else(||AppError::validation("artworkId","图片不属于当前人设、尚无正式图片或已停用"))?;crate::pet_creation::marker(&item.id)}else{if a.artwork_id.is_some(){return Err(AppError::validation("artworkId","颜文字模式不使用图片ID"));}let text=a.expression.ok_or_else(||AppError::validation("expression","请生成临时颜文字"))?;validate_expression(&text)?;text};s.pet.chat_expression=Some(expression.clone());Ok(json!({"expression":expression,"temporary":true}))},
   _=>Err(AppError::new("UNKNOWN_TOOL","未知聊天视觉工具"))
  }
 })
}
pub fn fallback(
    runtime: &Runtime,
    persona: i64,
    session: i64,
    revision: u64,
) -> AppResult<Option<String>> {
    runtime.read(|s, _| {
        coordinator::require_context(s, persona, session, revision)?;
        let items = crate::pet_creation::chat_choices(&s.connection, persona)?;
        let expression = items
            .as_ref()
            .and_then(|items| {
                items
                    .iter()
                    .find(|a| a.name == "自然交谈")
                    .or_else(|| items.iter().find(|a| a.kind == "base"))
            })
            .map(|a| crate::pet_creation::marker(&a.id));
        if let Some(value) = &expression {
            s.pet.chat_expression = Some(value.clone());
        }
        Ok(expression)
    })
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Generate {
    prompt: String,
    #[serde(default)]
    use_reference: bool,
}
pub async fn generate(
    client: &AiClient,
    runtime: &Runtime,
    lease: &RequestLease,
    persona: i64,
    session: i64,
    revision: u64,
    call: &ToolCall,
) -> AppResult<Value> {
    let a: Generate = serde_json::from_value(call.arguments.clone())
        .map_err(|_| AppError::validation("prompt", "请输入绘图要求"))?;
    if a.prompt.trim().is_empty() {
        return Err(AppError::validation("prompt", "绘图要求不能为空"));
    }
    let snapshot = runtime.read(|s, p| {
        lease.require_conversation_scope(&runtime.requests, session, persona)?;
        coordinator::require_context(s, persona, session, revision)?;
        crate::image_generation::snapshot(&s.connection, p)?.ok_or_else(|| {
            AppError::new(
                "IMAGE_GENERATION_DISABLED",
                "尚未开启生图，请在设置→提供商→生图选择模型并开启",
            )
        })
    })?;
    let reference = if a.use_reference { Some(reference::Reference::capture(runtime, lease, persona, session, revision)?) } else { None };
    let bytes = if let Some(reference) = &reference {
        let image = reference.read(runtime, persona).await?;
        lease.cancel.check()?;
        reference.verify(runtime, lease, persona, session, revision)?;
        crate::image_generation::openai_compatible::generate_reference_picture(client, &snapshot, &image, &reference.prompt(&a.prompt), &lease.cancel).await?
    } else {
        crate::image_generation::openai_compatible::generate_picture(client, &snapshot, &a.prompt, &lease.cancel).await?
    };
    lease.cancel.check()?;
    let format = image::guess_format(&bytes)
        .map_err(|_| AppError::new("IMAGE_GENERATION_FAILED", "服务没有返回可识别的图片"))?;
    let ext = match format {
        image::ImageFormat::Png => "png",
        image::ImageFormat::Jpeg => "jpg",
        image::ImageFormat::WebP => "webp",
        _ => {
            return Err(AppError::new(
                "IMAGE_GENERATION_FAILED",
                "图片格式不受支持，请选择PNG、JPEG或WebP输出",
            ))
        }
    };
    let mut staged = crate::chat_images::stage(
        runtime.paths.as_ref().map_err(Clone::clone)?,
        vec![crate::chat_images::Input {
            filename: format!("AI生成图片.{ext}"),
            data_url: format!(
                "data:image/{ext};base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            ),
        }],
    )?;
    if let Some(reference) = &reference { reference.verify(runtime, lease, persona, session, revision)?; }
    let result=runtime.read(|s,p|{lease.require_conversation_scope(&runtime.requests,session,persona)?;coordinator::require_context(s,persona,session,revision)?;let current=crate::image_generation::snapshot(&s.connection,p)?.ok_or_else(||AppError::new("CONTEXT_CHANGED","生图已关闭，旧结果未加入聊天"))?;if current.provider!=snapshot.provider||current.revision!=snapshot.revision||current.key!=snapshot.key{return Err(AppError::new("CONTEXT_CHANGED","生图配置已改变，旧结果未加入聊天"));}
  if let Some(reference) = &reference { reference.verify_store(&s.connection, persona)?; }
  let active=s.chat_requests.get(&session).filter(|a|a.request_id==lease.id.to_string()).ok_or_else(||AppError::new("CONTEXT_CHANGED","本次聊天已结束，旧图片未加入记录"))?;let round=crate::error::parse_id(&active.round_id)?;let content=if active.partial_content.trim().is_empty(){"已生成图片。".to_string()}else{active.partial_content.clone()};
  lease.cancel.check()?;let tx=s.connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;crate::chat_images::publish_staged(&staged.images)?;
  tx.execute("INSERT INTO messages(session_id,round_id,role,content,created_at) VALUES(?1,?2,'assistant',?3,?4) ON CONFLICT(round_id,role) DO NOTHING",params![session,round,content,chrono::Utc::now().to_rfc3339()])?;
  let message=tx.query_row("SELECT id FROM messages WHERE round_id=?1 AND role='assistant'",[round],|r|r.get::<_,i64>(0))?;
  crate::chat_images::insert(&tx,message,&staged.images,&chrono::Utc::now().to_rfc3339())?;
  tx.execute("UPDATE message_images SET caption=?1 WHERE message_id=?2 AND image_file=?3",params![format!("AI生成图片；用户绘图要求：{}",a.prompt),message,staged.images[0].image_file])?;let image_id=tx.query_row("SELECT id FROM message_images WHERE message_id=?1 AND image_file=?2",params![message,staged.images[0].image_file],|r|r.get::<_,i64>(0))?;lease.cancel.check()?;tx.commit()?;Ok(json!({"status":"saved","usedDefaultAppearance":a.use_reference,"imageId":image_id.to_string(),"messageId":message.to_string(),"display":"图片已保存到本轮AI消息，聊天页面会展示，不要编造本机路径或下载链接"}))
 });
    if result.is_ok() {
        staged.committed = true;
    }
    result
}
