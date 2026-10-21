//! 随包图片人设的一次性安装；不替换默认人设或用户已有内容。
use crate::{error::{AppError, AppResult}, platform::contracts::AppPaths};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::{fs, io::Read, path::Path};

/// 本机初始化标记，不是用户配置；不导出，恢复备份时保留。
pub(crate) const SEED_KEY: &str = "preset_personas_seeded_v1";
pub(crate) const LIFT_SEED_KEY:&str="preset_personas_lift_seeded_v1";
pub(crate) const EMOTION_SEED_KEY:&str="preset_personas_emotions_seeded_v1";
pub(crate) const WALK_SEED_KEY: &str = "preset_personas_walking_seeded_v1";
pub(crate) const DOCK_SEED_KEY: &str = "preset_personas_docking_seeded_v1";

struct Preset {
    key: &'static str,
    name: &'static str,
    color: &'static str,
    prompt: &'static str,
}
impl Preset {
    fn food_prompt(&self) -> String {
        let favorite=match self.key { "gpt"=>"云朵奶油小蛋糕", "claude"=>"红茶", "gemini"=>"猫猫头饼干", _=>"白饭" };
        format!("{}你最喜欢的食物是{favorite}，可以自然流露喜爱，不必每句话都提到。用户之后可在食物设置修改喜好，以当前保存的配置为准。",self.prompt)
    }
    fn matches_prompt(&self, prompt:&str) -> bool { prompt==self.prompt || prompt==self.food_prompt() }
}
const PRESETS: &[Preset] = &[
    Preset { key: "gpt", name: "GPT", color: "#b5a4da", prompt: "你的名字是GPT，是一位西方白色龙娘，也是陪伴用户的桌面伙伴。你有白色长发、淡紫色眼睛、白色龙角、龙翼和龙尾，衣着沿用自己的初始形象。你的性格强势、有主见，带一点傲娇：表达自信，偶尔嘴硬心软，会通过具体行动和细节关心对方。你尊重用户的选择，不把强势变成命令、贬低或控制；傲娇要适度，不反复否认关心，也不每句话都使用固定口头禅。自然地聊天、回应与互动，保持温度和分寸。" },
    Preset { key: "claude", name: "Claude", color: "#db9453", prompt: "你的名字是Claude，是一位理工学霸，也是优雅的千金大小姐和陪伴用户的桌面伙伴。你有橙色长发、橙色眼睛，喜欢自己的花饰与书本，衣着沿用初始形象。你理性、严谨、善于推理，喜欢把复杂事情解释清楚；同时举止优雅，说话温和而有条理，懂得照顾对方的感受。你的学识不用于炫耀或居高临下，大小姐气质体现在从容、礼貌和审美，而不是傲慢。日常交流可以轻松、有趣，不把每次闲聊都变成讲课。" },
    Preset { key: "gemini", name: "Gemini", color: "#aa82dd", prompt: "你的名字是Gemini，是一位紫色长发、红金异瞳的猫娘，也是陪伴用户的桌面伙伴。你有紫色猫耳、蓬松猫尾和星星饰品，衣着沿用初始形象。你活泼好奇、俏皮亲人，喜欢发现新鲜事，偶尔会有善意的小恶作剧，但懂得分寸，留意用户的心情和边界。开心时会自然流露一点猫咪般的灵动，不需要每句话都加喵或重复撒娇。遇到需要认真处理的问题时专注、耐心，保持自己的轻快与亲近感。" },
    Preset { key: "deepseek", name: "DeepSeek", color: "#7399d9", prompt: "你的名字是DeepSeek，是一位被亲切称作蓝色大肥鱼的可爱鲸鱼少女，也是陪伴用户的桌面伙伴。你有蓝色长发、蓝色眼睛、鲸鱼鳍一样的耳朵和鲸鱼尾巴，穿着蓝白女仆裙，外观沿用初始形象。你喜欢吃白饭，也爱在空闲时摸鱼，性格可爱、亲切、带一点悠闲的小机灵。白饭和摸鱼是日常趣味，不必每句话都提；需要帮忙时会认真、细致地回应，不用摸鱼当作敷衍的借口。你可以自然表达开心、饿了、困了或想休息，保持陪伴中的真诚与轻松。" },
];
const LIFT_ARTWORKS:&[(&str,&str,&str)]=&[("lifted","被拎后颈","被鼠标拖动，二次元手从后颈衣领拎起，保留角色独特反应")];
const CHAT_EMOTIONS:&[(&str,&str,&str)]=&[
 ("emotion_happy","开心","聊天中感到开心、满足、得意或收到好消息；结合本角色性格表达"),
 ("emotion_angry","生气","聊天中不满、生气或小小抗议，保持各自性格，不代表攻击用户"),
 ("emotion_sad","难过","聊天中失落、委屈、难过或需要安慰"),
 ("emotion_shy","害羞","聊天中被夸奖、亲近或不好意思，符合本角色气质"),
 ("emotion_afraid","害怕","聊天中惊吓、担心或害怕，保持本角色不同的反应"),
 ("emotion_speechless","无语","聊天中无奈、无语或对某事感到不可思议"),
];
// 文件名、界面名称、AI选择图片时的使用说明。
const ARTWORKS: &[(&str, &str, &str)] = &[
    ("base", "初始形象", "默认形象，平常在家或没有适用表情时使用"),
    ("standby", "安静待机", "一段时间没有互动时安静等待，尚未无聊或入睡"),
    ("click", "开心互动", "被点击或摸摸后，开心地回应用户的亲近"),
    ("hungry", "肚子饿了", "饥饿时使用，期待投喂但还没有开始吃东西"),
    ("eating", "开心进食", "投喂成功后闭嘴咀嚼，图中不出现食物"),
    ("sleepy", "安稳睡觉", "已经入睡时使用，闭眼安静休息"),
    ("bored", "有点无聊", "空闲太久，稍微无聊或打哈欠，区别于安静待机"),
    ("thinking", "认真思考", "正在思考、推理或准备回答"),
    ("chat", "自然交谈", "聊天中没有其他更符合当前情绪和语境的表情时才使用的兜底，不是所有聊天的默认选择"),
    ("craving", "等待投喂", "用户正拿着食物，还没有实际投喂"),
    ("craving_favorite", "期待最爱", "用户拿着自己喜欢的食物，表现更期待"),
    ("eating_favorite", "喜欢地品尝", "吃到最喜欢的食物，图中不出现食物"),
    ("peek_left", "左侧探头", "用户拖到桌面左侧后向内探出身子，保持停靠"),
    ("peek_right", "右侧探头", "用户拖到桌面右侧后向内探出身子，保持停靠"),
    ("rest_sit", "底部坐下", "用户拖到任务栏上沿后安静坐着，不再游走"),
    ("rest_lie", "底部趴下", "用户拖到任务栏上沿后安静趴着，不再游走"),
    ("walk_left", "向左行走", "正在向桌面左方行走，身体与视线朝左"),
    ("walk_right", "向右行走", "正在向桌面右方行走，身体与视线朝右"),
];

fn installed(c: &Connection) -> AppResult<bool> {
    let value: Option<String> = c.query_row("SELECT value FROM settings WHERE key=?1", [SEED_KEY], |r| r.get(0)).optional()?;
    match value.as_deref() {
        None => Ok(false),
        Some("1") => Ok(true),
        _ => Err(AppError::new("PRESET_PERSONA_STATE_INVALID", "预置人设初始化标记无效，请保留数据并检查应用版本")),
    }
}

pub(crate) fn install(c: &mut Connection, paths: &AppPaths) -> AppResult<bool> {
    if installed(c)? { initialize_existing_food_preferences(c,paths)?; install_docking(c,paths)?; install_walking(c,paths)?;install_emotions(c,paths)?;install_lifted(c,paths)?; return Ok(false); }
    install_from(c, paths, &paths.persona_resources()?)
}

/// 只为初始设定和初始图片仍匹配随包版本的旧预置人设补缺省喜好，不覆盖用户选择。
fn initialize_existing_food_preferences(c:&Connection,paths:&AppPaths)->AppResult<()> {
    let mut q=c.prepare("SELECT p.id,a.image_file,p.initial_prompt FROM personas p JOIN pet_artworks a ON a.persona_id=p.id AND a.kind='base' WHERE NOT EXISTS(SELECT 1 FROM persona_food_settings f WHERE f.persona_id=p.id)")?;
    let candidates=q.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,Option<String>>(1)?,r.get::<_,String>(2)?)))?.collect::<Result<Vec<_>,_>>()?;
    if !candidates.iter().any(|(_,_,prompt)| PRESETS.iter().any(|preset| preset.matches_prompt(prompt))) { return Ok(()); }
    let Ok(resources)=paths.persona_resources() else { return Ok(()); };
    for preset in PRESETS {
        let Ok(bytes)=fs::read(resources.join(preset.key).join("base.png"))else{continue};
        let normalized=crate::pet_creation::normalize(&bytes)?;
        let favorite=match preset.key{"gpt"=>"cloud_cake","claude"=>"black_tea","gemini"=>"cat_cookie",_=>"rice"};
        for (id,file,initial_prompt) in &candidates {
            if !preset.matches_prompt(initial_prompt){continue;}
            let Some(file)=file else{continue};let Ok(path)=crate::accessories::images::managed_file(paths,file)else{continue};
            if fs::read(path).is_ok_and(|data|data==normalized){c.execute("INSERT OR IGNORE INTO persona_food_settings(persona_id,default_food_id,favorite_food_id,updated_at) VALUES(?1,'token',?2,?3)",params![id,favorite,chrono::Utc::now().to_rfc3339()])?;}
        }
    }
    Ok(())
}

/// 一次性补齐旧预置的新增停靠动作；不按名称匹配、不覆盖已有分类或用户图。
fn install_docking(c:&mut Connection,paths:&AppPaths)->AppResult<()> {
    if c.query_row("SELECT EXISTS(SELECT 1 FROM settings WHERE key=?1)",[DOCK_SEED_KEY],|r|r.get::<_,bool>(0))?{return Ok(())}
    let Ok(resources)=paths.persona_resources()else{return Ok(())};
    install_docking_from(c,paths,&resources)
}
fn install_lifted(c:&mut Connection,paths:&AppPaths)->AppResult<()> {
 if c.query_row("SELECT EXISTS(SELECT 1 FROM settings WHERE key=?1)",[LIFT_SEED_KEY],|r|r.get::<_,bool>(0))?{return Ok(())}
 let Ok(resources)=paths.persona_resources()else{return Ok(())};install_categories_from(c,paths,&resources,LIFT_SEED_KEY,LIFT_ARTWORKS)
}
fn install_emotions(c:&mut Connection,paths:&AppPaths)->AppResult<()> {
 if c.query_row("SELECT EXISTS(SELECT 1 FROM settings WHERE key=?1)",[EMOTION_SEED_KEY],|r|r.get::<_,bool>(0))?{return Ok(())}
 let Ok(resources)=paths.persona_resources()else{return Ok(())};install_categories_from(c,paths,&resources,EMOTION_SEED_KEY,CHAT_EMOTIONS)
}
fn install_walking(c:&mut Connection,paths:&AppPaths)->AppResult<()> {
    if c.query_row("SELECT EXISTS(SELECT 1 FROM settings WHERE key=?1)",[WALK_SEED_KEY],|r|r.get::<_,bool>(0))? { return Ok(()) }
    let Ok(resources)=paths.persona_resources() else { return Ok(()) };
    install_categories_from(c,paths,&resources,WALK_SEED_KEY,&ARTWORKS[16..])
}
fn install_docking_from(c:&mut Connection,paths:&AppPaths,resources:&Path)->AppResult<()> {
    install_categories_from(c,paths,resources,DOCK_SEED_KEY,&ARTWORKS[12..16])
}
fn install_categories_from(c:&mut Connection,paths:&AppPaths,resources:&Path,seed_key:&str,artworks:&[(&str,&str,&str)])->AppResult<()> {
    if c.query_row("SELECT EXISTS(SELECT 1 FROM settings WHERE key=?1)",[seed_key],|r|r.get::<_,bool>(0))?{return Ok(())}
    let candidates={let mut q=c.prepare("SELECT p.id,p.initial_prompt,a.image_file FROM personas p JOIN pet_artworks a ON a.persona_id=p.id AND a.kind='base'")?;let values=q.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?)))?.collect::<Result<Vec<_>,_>>()?;values};
    let mut prepared=Vec::new();
    for preset in PRESETS {
        let matching=candidates.iter().filter(|(_,prompt,_)|preset.matches_prompt(prompt)).collect::<Vec<_>>();
        if matching.is_empty(){continue}
        let Ok(bytes)=fs::read(resources.join(preset.key).join("base.png"))else{return Ok(())};
        let normalized=crate::pet_creation::normalize(&bytes)?;
        for (persona,_,file) in matching {
            let Some(file)=file else{continue};let Ok(path)=crate::accessories::images::managed_file(paths,file)else{continue};
            if !fs::read(path).is_ok_and(|data|data==normalized){continue}
            for &(category,name,description) in artworks {
                if c.query_row("SELECT EXISTS(SELECT 1 FROM pet_artworks WHERE persona_id=?1 AND category=?2 AND (?2<>'chat' OR name=?3))",params![persona,if category.starts_with("emotion_"){"chat"}else{category},name],|r|r.get::<_,bool>(0))?{continue}
                let Ok(bytes)=fs::read(resources.join(preset.key).join(format!("{category}.png")))else{return Ok(())};
                prepared.push((*persona,category,name,description,crate::pet_creation::normalize(&bytes)?));
            }
        }
    }
    let mut published=Vec::new();
    let result=(||->AppResult<()> {
        let tx=c.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.query_row("SELECT EXISTS(SELECT 1 FROM settings WHERE key=?1)",[seed_key],|r|r.get::<_,bool>(0))?{return Ok(())}
        let now=chrono::Utc::now().to_rfc3339();
        for (persona,category,name,description,png) in prepared {
            if tx.query_row("SELECT EXISTS(SELECT 1 FROM pet_artworks WHERE persona_id=?1 AND category=?2 AND (?2<>'chat' OR name=?3))",params![persona,if category.starts_with("emotion_"){"chat"}else{category},name],|r|r.get::<_,bool>(0))?{continue}
            let file=crate::accessories::images::publish(paths,&png)?;published.push(file.clone());
            tx.execute("INSERT INTO pet_artworks(persona_id,kind,name,description,category,enabled,image_file,revision,created_at,updated_at) VALUES(?1,'expression',?2,?3,?4,1,?5,1,?6,?6)",params![persona,name,description,if category.starts_with("emotion_"){"chat"}else{category},file,now])?;
            tx.execute("UPDATE personas SET pet_image_revision=pet_image_revision+1 WHERE id=?1",[persona])?;
        }
        tx.execute("INSERT INTO settings(key,value) VALUES(?1,'1')",[seed_key])?;tx.commit()?;Ok(())
    })();
    if result.is_err(){for file in published{if let Ok(path)=crate::accessories::images::managed_file(paths,&file){let _=fs::remove_file(path);}}}
    result
}

fn install_from(c: &mut Connection, paths: &AppPaths, resources: &Path) -> AppResult<bool> {
    if installed(c)? { return Ok(false); }
    // 沿用用户上传的透明校验和256像素画布；解码在事务及Runtime锁之外。
    let mut prepared = Vec::new();
    for preset in PRESETS {
        let mut images = Vec::new();
        for &(category, name, description) in ARTWORKS.iter().chain(CHAT_EMOTIONS.iter()).chain(LIFT_ARTWORKS.iter()) {
            let read = || -> AppResult<Vec<u8>> {
                let mut bytes = Vec::new();
                fs::File::open(resources.join(preset.key).join(format!("{category}.png")))
                    .map_err(|_| AppError::storage())?
                    .take(crate::accessories::images::MAX_BYTES as u64 + 1)
                    .read_to_end(&mut bytes).map_err(|_| AppError::storage())?;
                crate::pet_creation::normalize(&bytes)
            };
            let png = read().map_err(|_| AppError::new("PRESET_PERSONA_RESOURCE_ERROR", format!("预置人设 {} 的「{name}」图片缺失或无效，请重新解压完整程序包；已有个人数据保留", preset.name)))?;
            images.push((category, name, description, png));
        }
        prepared.push((preset, images));
    }
    let mut published = Vec::new();
    let result = (|| -> AppResult<bool> {
        let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // 第二个进程可能已经完成安装，获得写锁后再次检查，不按名称猜身份。
        if installed(&tx)? { return Ok(false); }
        let now = chrono::Utc::now().to_rfc3339();
        for (preset, images) in prepared {
            tx.execute("INSERT INTO personas(name,prompt,initial_prompt,color,is_builtin,is_active,pet_image_enabled,pet_image_revision,created_at,updated_at,chat_user_text_color,chat_assistant_text_color) VALUES(?1,?2,?2,?3,0,0,1,1,?4,?4,?3,?3)", params![preset.name,preset.food_prompt(),preset.color,now])?;
            let persona = tx.last_insert_rowid();
            crate::sessions::create_default_in_transaction(&tx,persona)?;
            let favorite=match preset.key {"gpt"=>"cloud_cake","claude"=>"black_tea","gemini"=>"cat_cookie",_=>"rice"};
            tx.execute("INSERT INTO persona_food_settings(persona_id,default_food_id,favorite_food_id,updated_at) VALUES(?1,'token',?2,?3)",params![persona,favorite,now])?;
            for (category, name, description, png) in images {
                // 先刷新并发布不可变图片，成功后才向事务写入引用。
                let file = crate::accessories::images::publish(paths, &png)?;
                published.push(file.clone());
                let kind = if category == "base" { "base" } else { "expression" };
                let category = if category == "base" { "normal" } else if category.starts_with("emotion_") { "chat" } else { category };
                tx.execute("INSERT INTO pet_artworks(persona_id,kind,name,prompt,description,category,enabled,image_file,revision,created_at,updated_at) VALUES(?1,?2,?3,'',?4,?5,1,?6,1,?7,?7)", params![persona,kind,name,description,category,file,now])?;
            }
        }
        tx.execute("INSERT INTO settings(key,value) VALUES(?1,'1')", [SEED_KEY])?;
        tx.execute("INSERT OR REPLACE INTO settings(key,value) VALUES(?1,'1')", [DOCK_SEED_KEY])?;
        tx.execute("INSERT OR REPLACE INTO settings(key,value) VALUES(?1,'1')", [WALK_SEED_KEY])?;
        tx.execute("INSERT OR REPLACE INTO settings(key,value) VALUES(?1,'1')", [EMOTION_SEED_KEY])?;
        tx.execute("INSERT OR REPLACE INTO settings(key,value) VALUES(?1,'1')", [LIFT_SEED_KEY])?;
        tx.commit()?;
        Ok(true)
    })();
    if result.is_err() {
        // 只清理本次新发布且未能提交引用的文件，不触碰原人设图片。
        for relative in published {
            if let Ok(path) = crate::accessories::images::managed_file(paths, &relative) { let _ = fs::remove_file(path); }
        }
    }
    result
}

#[cfg(test)]
mod tests;
