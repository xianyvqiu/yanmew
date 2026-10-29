use crate::{
    error::{AppError, AppResult},
    models::{
        ChatRound, ChatRoundStatus, Message, MessagePage, MessageRole, RoundSnapshot, Session,
    },
};
use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};

/// 第一版上手指南唯一内置正文，与前端设计 5.7.3 保持一致。
pub(crate) const INITIAL_GUIDE: &str = r#"# 欢迎来到颜喵，我是电子小猫 `(・ω・)`

我会住在你的桌面上，陪你聊天，也会饿、犯困、出门旅行，偶尔带点小礼物回来。先一起完成第一次聊天，其他功能用到时再慢慢认识。

> **先做这三件事：** 配置聊天模型 → 选择伙伴 → 发一句“你好”。随时可以到“设置 → 帮助与使用手册”查详细用法，或重新看新手教程。

## 1. 先接好聊天模型

打开 **设置 → 全局设置 → 基础服务 → 提供商**，添加服务地址和 API Key，再在聊天用途中选择连接与模型，保存并测试连接。

- 先配置聊天就能开聊，识图、生图等能力以后再按需配置。
- 不同用途可以共用连接，也可以各自选择模型；能聊天不代表已经配置好画图。
- 模型服务可能收费，额度和费用由你使用的服务商决定。请妥善保管 Key。

## 2. 选一个伙伴，也告诉它你是谁

**人设**决定伙伴的性格与外观。除了我，还有四位预置图片伙伴，也可以创建自己的角色。名字不限定模型服务，喜欢谁就选谁。

**用户设定**介绍的是你，可以写称呼、兴趣和作息。保存后，到聊天详情左侧的“绑定用户设定”选择；也能设置新会话的默认绑定。开启人设自调后，还可以看看伙伴的设定逐渐有哪些变化。

## 3. 从快捷聊天，到完整聊天

点击宠窗底部的聊天按钮可以直接发消息，从“…”或回复气泡的“聊天详情”可打开完整记录。这两个入口接着同一段会话聊。

- **Enter 发送，Shift+Enter 换行。** 回复期间可以继续准备下一条草稿；想停下时点击“暂停回复”。
- 左侧可新建、切换会话，右键会话可重命名、置顶或删除。
- 消息可以复制、编辑；最后一轮完整回复可以重说。删除从记录底部开始，达到输出上限时可使用“继续生成”。
- 添加图片可以请我看图，配置生图后也可以让我画图。想画角色自己，可以说：“参考你的默认形象画一张。”参考图需要对应服务支持。
- 在“会话设置 → 聊天外观”调整背景、气泡和文字颜色。向上翻历史时可以安心阅读，新回复不会把你强行拉回底部。

## 4. 让我记住重要的事

完整对话会逐渐形成**短期记忆与长期记忆**。聊天详情左侧“查看记事本”可以打开记忆窗口，查看、展开、翻页和编辑；生成失败时，原有来源仍保留。

记事本适合保存约定和持续关注的事，也可以直接对我说：“把这件事记进记事本。”改过聊天原文后，已有记忆不会自动重写，记错的地方可以一起修正。

## 5. 用 Skill 教我习惯与方法

在 **Skill 仓库**写交流习惯、背景资料或做事步骤，再绑定给对应人设；也能上传 `.md` 文件，检查简介和正文后保存。

- 标题是 `SKILL_BASE`、`SKILL_BASE.md`、`AGENTS` 或 `AGENTS.md`，并已绑定时，我会每次阅读全文。名称区分大小写，不必一定带文件后缀。
- 其他已绑定 Skill 会按需要查找、阅读，适合放较长或偶尔使用的资料。
- 普通聊天也可以请我创建或修改自己的 Skill；需要你确认时，会先给你看预览。

## 6. 和桌面上的我玩一会儿

左键点击可以互动，右键打开摸摸、喂食等菜单，拖动可以换位置。靠近边缘会探头或坐趴，往上放也可以；快速向上、左上或右上甩动再松手，就能抛接，飞行中按住可以接住我。

在宠窗 **“…” → 全屏游走** 开启走动。我有时会追逐鼠标，出现“等摸摸”时点击就好。待机、睡觉、出行、停靠或正在交互等情况会暂停游走。

**桌宠状态**查看当前会话的亲密度与饥饿；**时间与桌宠**调整生活节奏、追逐冷却和气泡收起。无聊时我也可能主动找你聊天。

## 7. 签到、喵币和小点心

到 **全局设置 → 扩展服务 → 喵币与签到** 每日领取喵币。所有伙伴共用一个钱包，可在“喵币账单”查看收支、趋势和小游戏排行。

在“食物”购买点心，弹窗里选择数量再确认。**Token 小点心免费且不限量**，其他食物使用喵币；每个人设可以有自己的喜好与默认食物。选中投喂后，食物跟随鼠标，点击伙伴喂食，右键或 Esc 取消。

## 8. 换成图片伙伴，添一些新表情

在 **人设与会话设置 → 桌宠自定义创作** 上传并接受默认形象，再选择“一次性生成所有基础表情”或“自己上传表情”。手动上传不调用生图服务；自动生成先给你看预览，接受后才使用。

可以为自己的表情填写动作和神态要求。图片库每张卡片的“查看与操作”可以上传、生成、编辑和检查预览。开启图片模式需要默认形象和可用待机图。

默认形象、表情、饰品和伴手礼使用**真正的透明底**；生成不透明时会尝试本地抠图，记得检查边缘。普通聊天画图可以有背景，也可以要求我查看结果、检测透明度或抠图。生图可能收费，预览满意后再接受。

## 9. 给伙伴准备小装扮

在 **饰品**制作、佩戴、卸下或出售装扮，多件可以一起戴，并调整大小、位置和前后层级。可以上传透明图片，配置生图后也能生成；没有生图服务时还有图标、Emoji 和颜文字等装扮。

亲密度达到相应阶段时，我可能提出制作请求。先看预览，再确认花费喵币；装扮保存在当前会话，生图服务费用另计。

## 10. 等我旅行，给我写封信

当前会话在家且符合空闲条件时，会累计出门前的等待，**默认4～8小时**；旅行默认持续 **1～3小时**。可以在“时间与桌宠”调整节奏，已经抽到的当前等待不会重新计算。

出门后快捷聊天会变成邮箱，你可以给我写信，等待旅途回信，也可能收到我的主动来信。左键点击出行中的我，可以看已出门多久；回来会短暂显示“我回来了”。

在 **出行**查看历史、邮箱、伴手礼和游记，可搜索、按一趟或多趟旅行筛选，展开卡片读全文。开启伴手礼生图并配好服务后，新礼物才会尝试配图；没有图片仍会保留文字。

> **旅途中切换人设或会话，会取消这次旅行和待回信。** 已有信件和经历保留，回来后不能再接续旅途回信；删除游记不会连带删除信件与礼物。

## 11. 提醒、天气和特别的日子

可以说：“明天上午九点提醒我开会。”也可以到 **提醒**手动创建、搜索和筛选。到点后本次提醒自动完成，宠窗点击“知道了”即可收起；提醒话语也会进入当时的聊天。到“通用”配置并验证邮件通知后，还能收到邮件。颜喵关闭时不能实时提醒。

在 **时间与天气**设置地区，查看实况、预报、穿衣建议、预警、台风强度与路径和来源链接。气象情况重大变化时，会更新角色关怀；获取失败不代表没有预警，记得看更新时间。聊天中也可以请我查看已有资讯。

在 **日历**查看节日、公历与农历，管理特别日期。节日可能有签到奖励和祝福；需要到点通知的事情，另外创建提醒。

## 12. 记下现实生活的账

**现实记账**记录生活中的钱，与喵币分开。例如：“帮我记一笔现实支出：今天午餐25元。”也可以手动记账。

首页查看收支和近期账单，直接点击日/月/年预算卡片编辑；底部栏可切换账单、统计、资产、周期、分类和日历。资产页可管理默认、储蓄和旅游资金池。这里记录金额，不会连接银行扣款或转账。

## 13. 玩一局小游戏

**小游戏插件**里有剪刀石头布、掷骰子、接星星、我的家、扫雷和2048，点击卡片打开，帮助与进度在游戏页面查看。

扫雷和2048都有免费与赚钱模式，赚钱模式各自每天最多5局，开始前看看入场费和奖励规则。也能安装本地 `.yanmew-game` 插件，是否允许使用喵币或联网，由你决定。“清除数据并重载”会重置该插件进度，操作前先确认。

## 14. 把颜喵调成你喜欢的样子，照顾好资料

**通用**可以切换亮暗主题、设置开机自启、邮件和聊天工具次数，也能导出、导入备份。需要退出时，从系统托盘选择“退出”；只关闭聊天或设置窗口，伙伴还在运行。

- **全局共用：** 服务、提醒、天气、日历、Skill 与用户设定仓库、喵币、食物库存、现实账本和小游戏。
- **跟随人设：** 性格、形象、表情库、聊天外观与食物喜好。
- **跟随会话：** 聊天、记忆、记事本、亲密度、日常状态、旅行与来信、伴手礼、游记和饰品。

换电脑、更新或恢复资料前先备份，导入时仔细看范围。**全量恢复会替换全局设置和账目**，服务 Key 与邮箱授权码需要重新配置，不随导出备份带走。资料保存在本机，使用模型时仍会把需要的内容发给你选择的服务。

遇到“！”可以点开看错误，再点击收起；聊天或生图失败，先检查对应服务、模型、额度和连接，保留资料，不要急着删数据。

---

不用一次记住所有功能。先配好聊天模型，回来和我打个招呼吧，我在这里等你。`ฅ^•ﻌ•^ฅ`"#;

const DEFAULT_TITLE: &str = "新会话";
const MAX_TITLE_CHARS: usize = 100;
const MAX_MESSAGE_PAGE: i64 = 200;

#[path = "sessions/operations.rs"]
mod operations;
pub(crate) use operations::delete_in_context;
#[path = "sessions/titles.rs"]
mod titles;
pub(crate) use titles::generate_first_title;
#[path = "sessions/messages.rs"]
pub(crate) mod messages;

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn session_from_row(row: &Row<'_>) -> rusqlite::Result<Session> {
    Ok(Session {
        id: row.get::<_, i64>(0)?.to_string(),
        persona_id: row.get::<_, i64>(1)?.to_string(),
        user_profile_id: row.get::<_, Option<i64>>(2)?.map(|id| id.to_string()),
        title: row.get(3)?,
        memory_revision: row.get(4)?,
        intimacy: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        is_current: row.get(8)?,
        is_pinned: row.get(9)?,
    })
}

fn message_from_row(row: &Row<'_>) -> rusqlite::Result<Message> {
    let role: String = row.get(3)?;
    let role = MessageRole::parse(&role).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            3,
            rusqlite::types::Type::Text,
            "invalid message role".into(),
        )
    })?;
    Ok(Message {
        id: row.get::<_, i64>(0)?.to_string(),
        session_id: row.get::<_, i64>(1)?.to_string(),
        round_id: row.get::<_, Option<i64>>(2)?.map(|id| id.to_string()),
        role,
        content: row.get(4)?,
        expression: row.get(5)?,
        images:vec![],
        created_at: row.get(6)?,
    })
}

fn round_from_row(row: &Row<'_>) -> rusqlite::Result<ChatRound> {
    let status: String = row.get(2)?;
    let status = ChatRoundStatus::parse(&status).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Text,
            "invalid chat round status".into(),
        )
    })?;
    Ok(ChatRound {
        id: row.get::<_, i64>(0)?.to_string(),
        session_id: row.get::<_, i64>(1)?.to_string(),
        status,
        created_at: row.get(3)?,
        completed_at: row.get(4)?,
        stop_reason: row.get(5)?,
    })
}

fn require_session(connection: &Connection, persona_id: i64, session_id: i64) -> AppResult<()> {
    if connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sessions WHERE id=?1 AND persona_id=?2)",
        params![session_id, persona_id],
        |row| row.get::<_, bool>(0),
    )? {
        Ok(())
    } else {
        Err(AppError::new(
            "CONTEXT_CHANGED",
            "当前人设或会话已经改变，请刷新后重试",
        ))
    }
}

pub fn get(connection: &Connection, persona_id: i64, session_id: i64) -> AppResult<Session> {
    connection
        .query_row(
            "SELECT s.id,s.persona_id,s.user_profile_id,s.title,s.memory_revision,s.intimacy,s.created_at,s.updated_at,
                    EXISTS(SELECT 1 FROM settings WHERE key='current_session_id' AND value=CAST(s.id AS TEXT)),s.is_pinned
             FROM sessions s WHERE s.id=?1 AND s.persona_id=?2",
            params![session_id, persona_id],
            session_from_row,
        )
        .optional()?
        .ok_or_else(AppError::not_found)
}

pub fn list(connection: &Connection, persona_id: i64) -> AppResult<Vec<Session>> {
    let exists = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM personas WHERE id=?1)",
        [persona_id],
        |row| row.get::<_, bool>(0),
    )?;
    if !exists {
        return Err(AppError::not_found());
    }
    let mut statement = connection.prepare(
        "SELECT s.id,s.persona_id,s.user_profile_id,s.title,s.memory_revision,s.intimacy,s.created_at,s.updated_at,
                EXISTS(SELECT 1 FROM settings WHERE key='current_session_id' AND value=CAST(s.id AS TEXT)),s.is_pinned
         FROM sessions s WHERE s.persona_id=?1 ORDER BY s.is_pinned DESC,s.updated_at DESC,s.id DESC",
    )?;
    let sessions = statement
        .query_map([persona_id], session_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(sessions)
}

/// 置顶只改变列表排序，不变更聊天更新时间、当前选择或上下文版本。
pub fn set_pinned(connection: &mut Connection, persona_id: i64, session_id: i64, pinned: bool) -> AppResult<Session> {
    let tx = connection.transaction()?;
    require_session(&tx, persona_id, session_id)?;
    tx.execute("UPDATE sessions SET is_pinned=?1 WHERE id=?2 AND persona_id=?3", params![pinned, session_id, persona_id])?;
    let session = get(&tx, persona_id, session_id)?;
    tx.commit()?;
    Ok(session)
}

/// 切换人设或会话时，已出发旅行与待回信都取消；未出发倒计时保留在会话行。
pub(crate) fn cancel_current_travel_and_replies(
    connection: &Connection,
    changed_at: &str,
) -> AppResult<()> {
    crate::travel::cancel_current(connection, changed_at)
}

fn set_current_in_transaction(
    transaction: &Transaction<'_>,
    persona_id: i64,
    session_id: i64,
    changed_at: &str,
) -> AppResult<bool> {
    require_session(transaction, persona_id, session_id)?;
    let current: Option<String> = transaction
        .query_row(
            "SELECT value FROM settings WHERE key='current_session_id'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if current.as_deref() == Some(&session_id.to_string()) {
        return Ok(false);
    }
    cancel_current_travel_and_replies(transaction, changed_at)?;
    transaction.execute(
        "INSERT INTO settings(key,value) VALUES('current_session_id',?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [session_id.to_string()],
    )?;
    Ok(true)
}

pub fn create(connection: &mut Connection, persona_id: i64) -> AppResult<Session> {
    create_with_profile(connection, persona_id, None)
}

/// 创建和绑定同事务提交；无效设定不得切换当前会话或取消旅行。
pub fn create_with_profile(connection: &mut Connection, persona_id: i64, user_profile_id: Option<i64>) -> AppResult<Session> {
    let changed_at = now();
    let transaction = connection.transaction()?;
    let result = create_in_transaction(&transaction, persona_id, user_profile_id, DEFAULT_TITLE, &changed_at)?;
    transaction.commit()?;
    Ok(result)
}

/// 初始化与普通创建共用同一写入路径；调用者负责提交，不能在这里嵌套事务。
fn create_in_transaction(
    transaction: &Transaction<'_>,
    persona_id: i64,
    user_profile_id: Option<i64>,
    title: &str,
    changed_at: &str,
) -> AppResult<Session> {
    let active = transaction
        .query_row(
            "SELECT is_active FROM personas WHERE id=?1",
            [persona_id],
            |row| row.get::<_, bool>(0),
        )
        .optional()?
        .ok_or_else(AppError::not_found)?;
    if !active {
        return Err(AppError::new(
            "CONTEXT_CHANGED",
            "当前人设已经改变，请刷新后重试",
        ));
    }
    if let Some(id) = user_profile_id { crate::user_profiles::get(&transaction, id)?; }
    let session=insert_in_transaction(transaction,persona_id,user_profile_id,title,changed_at)?;
    let session_id=crate::error::parse_id(&session.id)?;
    set_current_in_transaction(transaction, persona_id, session_id, changed_at)?;
    get(transaction, persona_id, session_id)
}

/// 资料创建复用写入与旅行初始化，但不切换当前会话或创建教程消息。
pub(crate) fn create_default_in_transaction(transaction:&Transaction<'_>,persona_id:i64)->AppResult<()> {
    if !transaction.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE persona_id=?1)",[persona_id],|r|r.get::<_,bool>(0))? {
        insert_in_transaction(transaction,persona_id,None,DEFAULT_TITLE,&now())?;
    }
    Ok(())
}
fn insert_in_transaction(transaction:&Transaction<'_>,persona_id:i64,user_profile_id:Option<i64>,title:&str,changed_at:&str)->AppResult<Session> {
    transaction.execute(
        "INSERT INTO sessions(persona_id,user_profile_id,title,created_at,updated_at) VALUES(?1,?2,?3,?4,?4)",
        params![persona_id, user_profile_id, title, changed_at],
    )?;
    let session_id = transaction.last_insert_rowid();
    crate::travel::reset_wait(transaction, session_id)?;
    get(transaction, persona_id, session_id)
}

/// 只由全新数据库初始化调用；不经过回复完成或主动消息结算，不生成回合与奖励。
pub(crate) fn create_initial_in_transaction(transaction: &Transaction<'_>) -> AppResult<()> {
    let persona_id: i64 = transaction.query_row(
        "SELECT id FROM personas WHERE is_builtin=1", [], |row| row.get(0),
    )?;
    let created_at = now();
    let session = create_in_transaction(transaction, persona_id, None, "初次见面", &created_at)?;
    transaction.execute(
        "INSERT INTO messages(session_id,round_id,role,content,created_at) VALUES(?1,NULL,'assistant',?2,?3)",
        params![crate::error::parse_id(&session.id)?, INITIAL_GUIDE, created_at],
    )?;
    Ok(())
}

pub fn select(connection: &mut Connection, persona_id: i64, session_id: i64) -> AppResult<bool> {
    let changed_at = now();
    let transaction = connection.transaction()?;
    let changed = set_current_in_transaction(&transaction, persona_id, session_id, &changed_at)?;
    transaction.commit()?;
    Ok(changed)
}

/// 绑定只修改资料引用，不重写历史消息、记忆或桌宠状态。
pub fn set_user_profile(connection: &mut Connection, persona_id: i64, session_id: i64, user_profile_id: Option<i64>) -> AppResult<Session> {
    let tx = connection.transaction()?;
    let before = get(&tx, persona_id, session_id)?;
    if let Some(id) = user_profile_id { crate::user_profiles::get(&tx, id)?; }
    if before.user_profile_id != user_profile_id.map(|id| id.to_string()) {
        tx.execute("UPDATE sessions SET user_profile_id=?1,updated_at=?2 WHERE id=?3",params![user_profile_id,now(),session_id])?;
    }
    let saved = get(&tx, persona_id, session_id)?;
    tx.commit()?;
    Ok(saved)
}

pub fn rename(
    connection: &mut Connection,
    persona_id: i64,
    session_id: i64,
    title: &str,
) -> AppResult<Session> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > MAX_TITLE_CHARS {
        return Err(AppError::validation("title", "会话标题须为 1–100 个字符"));
    }
    let transaction = connection.transaction()?;
    require_session(&transaction, persona_id, session_id)?;
    transaction.execute(
        "UPDATE sessions SET title=?1,updated_at=?2 WHERE id=?3 AND persona_id=?4",
        params![title, now(), session_id, persona_id],
    )?;
    let result = get(&transaction, persona_id, session_id)?;
    transaction.commit()?;
    Ok(result)
}

/// 会话删除先解除长期来源的 RESTRICT；全局钱包、提醒和插件不属于清理范围。
/// 返回提交后的当前会话，删除非当前项不改变当前选择。
pub fn delete(
    connection: &mut Connection,
    persona_id: i64,
    session_id: i64,
) -> AppResult<Option<String>> {
    let transaction = connection.transaction()?;
    require_session(&transaction, persona_id, session_id)?;
    let current: Option<String> = transaction
        .query_row(
            "SELECT value FROM settings WHERE key='current_session_id'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    transaction.execute("DELETE FROM long_memories WHERE session_id=?1", [session_id])?;
    transaction.execute(
        "DELETE FROM sessions WHERE id=?1 AND persona_id=?2",
        params![session_id, persona_id],
    )?;
    let next = if current.as_deref() == Some(session_id.to_string().as_str()) {
        let fallback: Option<i64> = transaction
            .query_row(
                "SELECT id FROM sessions WHERE persona_id=?1 ORDER BY updated_at DESC,id DESC LIMIT 1",
                [persona_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(id) = fallback {
            transaction.execute(
                "UPDATE settings SET value=?1 WHERE key='current_session_id'",
                [id.to_string()],
            )?;
        } else {
            transaction.execute("DELETE FROM settings WHERE key='current_session_id'", [])?;
        }
        fallback.map(|id| id.to_string())
    } else {
        current
    };
    transaction.commit()?;
    Ok(next)
}

#[cfg(test)]
#[path = "sessions/delete_tests.rs"]
mod delete_tests;
#[cfg(test)]
#[path = "sessions/recent_tests.rs"]
mod recent_tests;

pub fn messages(
    connection: &Connection,
    persona_id: i64,
    session_id: i64,
    after_id: Option<i64>,
    limit: i64,
) -> AppResult<MessagePage> {
    message_page(connection,persona_id,session_id,after_id,limit,false)
}

/// 最新页先读；返回内容仍按时间正序，next_after_id为更早页的不透明游标。
pub fn recent_messages(connection:&Connection,persona_id:i64,session_id:i64,before_id:Option<i64>,limit:i64)->AppResult<MessagePage>{
    message_page(connection,persona_id,session_id,before_id,limit,true)
}
fn message_page(connection:&Connection,persona_id:i64,session_id:i64,after_id:Option<i64>,limit:i64,backward:bool)->AppResult<MessagePage>{
    require_session(connection, persona_id, session_id)?;
    if !(1..=MAX_MESSAGE_PAGE).contains(&limit) {
        return Err(AppError::validation("limit", "每页消息数须为 1–200"));
    }
    let after_id = after_id.unwrap_or(if backward{i64::MAX}else{0});
    if after_id < 0 {
        return Err(AppError::validation("afterId", "消息游标不能为负数"));
    }
    let mut statement = connection.prepare(if backward {
        "SELECT id,session_id,round_id,role,content,expression,created_at
         FROM messages WHERE session_id=?1 AND id<?2 ORDER BY id DESC LIMIT ?3"
    }else{
        "SELECT id,session_id,round_id,role,content,expression,created_at
         FROM messages WHERE session_id=?1 AND id>?2 ORDER BY id LIMIT ?3"
    })?;
    let mut items = statement
        .query_map(params![session_id, after_id, limit + 1], message_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    let has_more = items.len() > limit as usize;
    if has_more {
        items.pop();
    }
    crate::chat_images::hydrate(connection,&mut items)?;
    let next_after_id = has_more.then(|| items.last().expect("non-empty page").id.clone());
    if backward {items.reverse();}
    Ok(MessagePage {
        items,
        next_after_id,
    })
}

pub fn start_round(connection:&mut Connection,persona_id:i64,session_id:i64,content:&str)->AppResult<RoundSnapshot> {
    start_round_with_images(connection,persona_id,session_id,content,&[])
}
pub(crate) fn start_round_with_images(
    connection: &mut Connection,
    persona_id: i64,
    session_id: i64,
    content: &str,
    images:&[crate::chat_images::NewImage],
) -> AppResult<RoundSnapshot> {
    let content = content.trim();
    if content.is_empty() && images.is_empty() {
        return Err(AppError::validation("content", "消息不能为空"));
    }
    let created_at = now();
    let transaction = connection.transaction()?;
    require_session(&transaction, persona_id, session_id)?;
    let pending = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM chat_rounds WHERE session_id=?1 AND status='pending')",
        [session_id],
        |row| row.get::<_, bool>(0),
    )?;
    if pending {
        return Err(AppError::new(
            "REQUEST_BUSY",
            "当前会话已有回复正在生成，请稍后重试",
        ));
    }
    transaction.execute(
        "INSERT INTO chat_rounds(session_id,status,created_at) VALUES(?1,'pending',?2)",
        params![session_id, created_at],
    )?;
    let round_id = transaction.last_insert_rowid();
    transaction.execute(
        "INSERT INTO messages(session_id,round_id,role,content,created_at) VALUES(?1,?2,'user',?3,?4)",
        params![session_id, round_id, content, created_at],
    )?;
    let message_id = transaction.last_insert_rowid();
    crate::chat_images::publish_staged(images)?;
    crate::chat_images::insert(&transaction,message_id,images,&created_at)?;
    transaction.execute(
        "UPDATE sessions SET memory_revision=memory_revision+1,updated_at=?1 WHERE id=?2",
        params![created_at, session_id],
    )?;
    let snapshot = round_snapshot(&transaction, persona_id, session_id, round_id)?;
    debug_assert_eq!(snapshot.user_message.id, message_id.to_string());
    transaction.commit()?;
    Ok(snapshot)
}

pub(crate) fn round_snapshot(
    connection: &Connection,
    persona_id: i64,
    session_id: i64,
    round_id: i64,
) -> AppResult<RoundSnapshot> {
    require_session(connection, persona_id, session_id)?;
    let round = connection
        .query_row(
            "SELECT id,session_id,status,created_at,completed_at,stop_reason FROM chat_rounds WHERE id=?1 AND session_id=?2",
            params![round_id, session_id],
            round_from_row,
        )
        .optional()?
        .ok_or_else(|| {
            AppError::new(
                "CONTEXT_CHANGED",
                "聊天回合不属于当前会话，请刷新后重试",
            )
        })?;
    let mut statement = connection.prepare(
        "SELECT id,session_id,round_id,role,content,expression,created_at
         FROM messages WHERE round_id=?1 AND session_id=?2 ORDER BY id",
    )?;
    let mut messages = statement
        .query_map(params![round_id, session_id], message_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    crate::chat_images::hydrate(connection,&mut messages)?;
    let user_message = messages
        .iter()
        .find(|message| message.role == MessageRole::User)
        .cloned()
        .ok_or_else(AppError::storage)?;
    let assistant_message = messages
        .into_iter()
        .find(|message| message.role == MessageRole::Assistant);
    Ok(RoundSnapshot {
        round,
        user_message,
        assistant_message,
    })
}

fn finish_round(
    connection: &mut Connection,
    persona_id: i64,
    session_id: i64,
    round_id: i64,
    content: Option<&str>,
    expression: Option<&str>,
    completed: bool,
    stop_reason: Option<&str>,
) -> AppResult<RoundSnapshot> {
    let transaction = connection.transaction()?;
    let result = finish_round_in_transaction(&transaction, persona_id, session_id, round_id, content, expression, completed, stop_reason)?;
    transaction.commit()?;
    Ok(result)
}

/// 复用终态保存规则；由调用方事务统一提交，失败时部分回复也一并回滚。
fn finish_round_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    persona_id: i64,
    session_id: i64,
    round_id: i64,
    content: Option<&str>,
    expression: Option<&str>,
    completed: bool,
    stop_reason: Option<&str>,
) -> AppResult<RoundSnapshot> {
    let normalized = content.filter(|value| !value.trim().is_empty());
    let has_sticker:bool=transaction.query_row("SELECT EXISTS(SELECT 1 FROM message_images i JOIN messages m ON m.id=i.message_id WHERE m.session_id=?1 AND m.round_id=?2 AND m.role='assistant' AND i.source_kind='sticker')",params![session_id,round_id],|r|r.get(0))?;
    if completed && normalized.is_none() && !has_sticker {
        return Err(AppError::validation("content", "完整回复正文不能为空"));
    }
    let completed_at = now();
    let current = round_snapshot(&transaction, persona_id, session_id, round_id)?;
    if current.round.status != ChatRoundStatus::Pending {
        return Ok(current);
    }
    if let Some(content) = normalized {
        transaction.execute(
            "INSERT INTO messages(session_id,round_id,role,content,expression,created_at)
             VALUES(?1,?2,'assistant',?3,?4,?5)
             ON CONFLICT(round_id,role) DO UPDATE SET content=excluded.content,expression=COALESCE(excluded.expression,messages.expression)",
            params![session_id, round_id, content, expression, completed_at],
        )?;
    }
    let status = if completed {
        "completed"
    } else {
        "interrupted"
    };
    transaction.execute(
        "UPDATE chat_rounds SET status=?1,completed_at=?2,stop_reason=?3 WHERE id=?4 AND session_id=?5 AND status='pending'",
        params![status, completed_at, if completed { None } else { stop_reason }, round_id, session_id],
    )?;
    if completed {
        let intimacy = transaction.query_row(
            "SELECT intimacy FROM sessions WHERE id=?1",
            [session_id],
            |row| row.get::<_, i64>(0),
        )?;
        let intimacy = crate::pet::rewarded_intimacy(intimacy)?;
        transaction.execute(
            "UPDATE sessions SET intimacy=?1,memory_revision=memory_revision+1,updated_at=?2 WHERE id=?3",
            params![intimacy, completed_at, session_id],
        )?;
    } else {
        transaction.execute(
            "UPDATE sessions SET memory_revision=memory_revision+1,updated_at=?1 WHERE id=?2",
            params![completed_at, session_id],
        )?;
    }
    let result = round_snapshot(&transaction, persona_id, session_id, round_id)?;
    Ok(result)
}

/// 导入改变上下文时，在导入事务中保存中断状态，提交后再取消网络请求。
pub(crate) fn interrupt_for_import(
    transaction: &rusqlite::Transaction<'_>,
    persona_id: i64,
    session_id: i64,
    round_id: i64,
    content: &str,
    expression: Option<&str>,
) -> AppResult<RoundSnapshot> {
    finish_round_in_transaction(transaction, persona_id, session_id, round_id, Some(content), expression, false, Some("context_changed"))
}

pub fn complete_round(
    connection: &mut Connection,
    persona_id: i64,
    session_id: i64,
    round_id: i64,
    content: &str,
    expression: Option<&str>,
) -> AppResult<RoundSnapshot> {
    finish_round(
        connection,
        persona_id,
        session_id,
        round_id,
        Some(content),
        expression,
        true,
        None,
    )
}

pub fn interrupt_round(
    connection: &mut Connection,
    persona_id: i64,
    session_id: i64,
    round_id: i64,
    partial_content: Option<&str>,
    expression: Option<&str>,
) -> AppResult<RoundSnapshot> {
    interrupt_round_with_reason(
        connection,
        persona_id,
        session_id,
        round_id,
        partial_content,
        expression,
        "unknown",
    )
}

pub fn interrupt_round_with_reason(
    connection: &mut Connection,
    persona_id: i64,
    session_id: i64,
    round_id: i64,
    partial_content: Option<&str>,
    expression: Option<&str>,
    reason: &str,
) -> AppResult<RoundSnapshot> {
    if !matches!(
        reason,
        "length_limit"
            | "user_cancelled"
            | "timeout"
            | "repetition"
            | "invalid_encoding"
            | "tool_limit"
            | "provider_error"
            | "context_changed"
            | "app_shutdown"
            | "recovered_after_restart"
            | "unknown"
    ) {
        return Err(AppError::validation("stopReason", "不支持的聊天停止原因"));
    }
    finish_round(
        connection,
        persona_id,
        session_id,
        round_id,
        partial_content,
        expression,
        false,
        Some(reason),
    )
}

/// 按尾部消息读取回合；主动消息不属于回合，因此返回 None。
pub fn last_round(
    connection: &Connection,
    persona_id: i64,
    session_id: i64,
) -> AppResult<Option<RoundSnapshot>> {
    require_session(connection, persona_id, session_id)?;
    let round_id: Option<Option<i64>> = connection
        .query_row(
            "SELECT round_id FROM messages WHERE session_id=?1 ORDER BY id DESC LIMIT 1",
            [session_id],
            |row| row.get(0),
        )
        .optional()?;
    round_id
        .flatten()
        .map(|id| round_snapshot(connection, persona_id, session_id, id))
        .transpose()
}

/// 续写仍使用原回合、原用户和原 AI 消息；事务中再次验证资格。
pub fn continue_round(
    connection: &mut Connection,
    persona_id: i64,
    session_id: i64,
    round_id: i64,
) -> AppResult<RoundSnapshot> {
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let current = round_snapshot(&transaction, persona_id, session_id, round_id)?;
    let tail: Option<(Option<i64>, String)> = transaction
        .query_row(
            "SELECT round_id,role FROM messages WHERE session_id=?1 ORDER BY id DESC LIMIT 1",
            [session_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let has_pending: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM chat_rounds WHERE session_id=?1 AND status='pending')",
        [session_id],
        |row| row.get(0),
    )?;
    if current.round.status != ChatRoundStatus::Interrupted
        || current.round.stop_reason.as_deref() != Some("length_limit")
        || current.assistant_message.is_none()
        || tail != Some((Some(round_id), "assistant".into()))
        || has_pending
    {
        return Err(AppError::new(
            "CONTEXT_CHANGED",
            "当前回合已不满足继续生成条件，请刷新后重试",
        ));
    }
    transaction.execute(
        "UPDATE chat_rounds SET status='pending',completed_at=NULL,stop_reason=NULL WHERE id=?1 AND session_id=?2",
        params![round_id, session_id],
    )?;
    transaction.execute(
        "UPDATE sessions SET memory_revision=memory_revision+1,updated_at=?1 WHERE id=?2",
        params![now(), session_id],
    )?;
    let result = round_snapshot(&transaction, persona_id, session_id, round_id)?;
    transaction.commit()?;
    Ok(result)
}

pub fn append_proactive_message(
    connection: &mut Connection,
    persona_id: i64,
    session_id: i64,
    content: &str,
    expression: Option<&str>,
) -> AppResult<Message> {
    let transaction = connection.transaction()?;
    let result = append_proactive_in_transaction(&transaction, persona_id, session_id, content, expression, &now())?;
    transaction.commit()?;
    Ok(result)
}

pub(crate) fn append_proactive_in_transaction(
    transaction: &rusqlite::Transaction<'_>, persona_id: i64, session_id: i64,
    content: &str, expression: Option<&str>, created_at: &str,
) -> AppResult<Message> {
    let content = content.trim();
    if content.is_empty() {
        return Err(AppError::validation("content", "主动消息不能为空"));
    }
    require_session(&transaction, persona_id, session_id)?;
    transaction.execute(
        "INSERT INTO messages(session_id,round_id,role,content,expression,created_at)
         VALUES(?1,NULL,'assistant',?2,?3,?4)",
        params![session_id, content, expression, created_at],
    )?;
    let id = transaction.last_insert_rowid();
    transaction.execute(
        "UPDATE sessions SET memory_revision=memory_revision+1,updated_at=?1 WHERE id=?2",
        params![created_at, session_id],
    )?;
    let result = transaction.query_row(
        "SELECT id,session_id,round_id,role,content,expression,created_at FROM messages WHERE id=?1",
        [id],
        message_from_row,
    )?;
    Ok(result)
}

/// 启动时没有运行请求，所有遗留 pending 回合都收束为 interrupted。
pub(crate) fn recover_pending_in_transaction(
    connection: &Connection,
    recovered_at: &str,
) -> AppResult<usize> {
    connection.execute(
        "UPDATE sessions SET memory_revision=memory_revision+1,updated_at=?1
         WHERE id IN (SELECT DISTINCT session_id FROM chat_rounds WHERE status='pending')",
        [recovered_at],
    )?;
    Ok(connection.execute(
        "UPDATE chat_rounds SET status='interrupted',completed_at=?1,stop_reason='recovered_after_restart' WHERE status='pending'",
        [recovered_at],
    )?)
}

pub fn recover_pending(connection: &mut Connection, recovered_at: &str) -> AppResult<usize> {
    let transaction = connection.transaction()?;
    let recovered = recover_pending_in_transaction(&transaction, recovered_at)?;
    transaction.commit()?;
    Ok(recovered)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database() -> Connection {
        let connection = crate::db::test_database();
        connection
    }

    #[test]
    fn session_pinning_orders_list_without_touching_chat_or_current_selection() {
        let mut c=database();
        let first=create(&mut c,1).unwrap();
        let second=create(&mut c,1).unwrap();
        let (first_id,second_id)=(first.id.parse::<i64>().unwrap(),second.id.parse::<i64>().unwrap());
        c.execute("UPDATE sessions SET updated_at='2026-01-01' WHERE id=?1",[first_id]).unwrap();
        c.execute("UPDATE sessions SET updated_at='2026-10-01' WHERE id=?1",[second_id]).unwrap();
        let before=get(&c,1,first_id).unwrap();
        set_pinned(&mut c,1,first_id,true).unwrap();
        let after=get(&c,1,first_id).unwrap();
        assert_eq!(list(&c,1).unwrap()[0].id,first.id);
        assert_eq!((after.updated_at,after.memory_revision),(before.updated_at,before.memory_revision));
        assert!(get(&c,1,second_id).unwrap().is_current);
        assert!(set_pinned(&mut c,999,first_id,false).is_err());
        assert!(get(&c,1,first_id).unwrap().is_pinned);
        set_pinned(&mut c,1,first_id,false).unwrap();
        assert_eq!(list(&c,1).unwrap()[0].id,second.id);
    }

    #[test]
    fn create_list_select_and_rename_keep_global_data() {
        let mut connection = database();
        connection
            .execute("UPDATE user_state SET coins=77", [])
            .unwrap();
        let first = create(&mut connection, 1).unwrap();
        let first_id = first.id.parse::<i64>().unwrap();
        connection.execute("INSERT INTO travels(session_id,status,started_at,planned_return_at,midway_letter_at,created_at) VALUES(?1,'away','a','b','c','a')",[first_id]).unwrap();
        connection.execute("INSERT INTO letters(session_id,sender,content,reply_due_at,reply_state,letter_kind,created_at) VALUES(?1,'user','信','b','pending','user','a')",[first_id]).unwrap();
        let second = create(&mut connection, 1).unwrap();
        assert!(second.is_current);
        assert_eq!(list(&connection, 1).unwrap().len(), 2);
        assert_eq!(
            connection
                .query_row("SELECT status FROM travels", [], |row| row
                    .get::<_, String>(0))
                .unwrap(),
            "cancelled"
        );
        assert_eq!(
            connection
                .query_row("SELECT reply_state FROM letters", [], |row| row
                    .get::<_, String>(0))
                .unwrap(),
            "cancelled"
        );
        let renamed = rename(&mut connection, 1, first_id, " 旧朋友 ").unwrap();
        assert_eq!(renamed.title, "旧朋友");
        assert!(select(&mut connection, 1, first_id).unwrap());
        assert!(!select(&mut connection, 1, first_id).unwrap());
        assert!(get(&connection, 1, first_id).unwrap().is_current);
        assert_eq!(
            connection
                .query_row("SELECT coins FROM user_state", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            77
        );
        assert!(rename(&mut connection, 999, first_id, "越权").is_err());
    }

    #[test]
    fn length_limit_continuations_preserve_messages_and_restart_content() {
        let mut connection = database();
        let session_id = create(&mut connection, 1).unwrap().id.parse().unwrap();
        let started = start_round(&mut connection, 1, session_id, "原问题").unwrap();
        let round_id = started.round.id.parse().unwrap();
        let first = interrupt_round_with_reason(
            &mut connection,
            1,
            session_id,
            round_id,
            Some("第一段 "),
            Some("(^_^)"),
            "length_limit",
        )
        .unwrap();
        let assistant = first.assistant_message.as_ref().unwrap();
        let assistant_id = assistant.id.clone();
        let created_at = assistant.created_at.clone();
        assert_eq!(assistant.content, "第一段 ");
        assert_eq!(get(&connection, 1, session_id).unwrap().intimacy, 0);
        assert_eq!(
            last_round(&connection, 1, session_id).unwrap(),
            Some(first.clone())
        );
        let pending = continue_round(&mut connection, 1, session_id, round_id).unwrap();
        assert_eq!(pending.round.status, ChatRoundStatus::Pending);
        assert_eq!(get(&connection, 1, session_id).unwrap().intimacy, 0);
        assert_eq!(pending.round.stop_reason, None);
        assert_eq!(pending.round.completed_at, None);
        assert_eq!(pending.user_message.id, started.user_message.id);
        assert_eq!(pending.assistant_message.unwrap().id, assistant_id);
        assert!(continue_round(&mut connection, 1, session_id, round_id).is_err());
        let second = interrupt_round_with_reason(
            &mut connection,
            1,
            session_id,
            round_id,
            Some("第一段 第二段\n"),
            None,
            "length_limit",
        )
        .unwrap();
        assert_eq!(second.assistant_message.as_ref().unwrap().id, assistant_id);
        assert_eq!(get(&connection, 1, session_id).unwrap().intimacy, 0);
        assert_eq!(
            second.assistant_message.as_ref().unwrap().created_at,
            created_at
        );
        assert_eq!(
            second
                .assistant_message
                .as_ref()
                .unwrap()
                .expression
                .as_deref(),
            Some("(^_^)")
        );
        continue_round(&mut connection, 1, session_id, round_id).unwrap();
        let completed = complete_round(
            &mut connection,
            1,
            session_id,
            round_id,
            "第一段 第二段\n结尾 ",
            None,
        )
        .unwrap();
        assert_eq!(completed.round.status, ChatRoundStatus::Completed);
        assert_eq!(get(&connection, 1, session_id).unwrap().intimacy, 1);
        assert_eq!(completed.round.stop_reason, None);
        assert_eq!(
            completed.assistant_message.as_ref().unwrap().id,
            assistant_id
        );
        assert_eq!(
            completed.assistant_message.as_ref().unwrap().content,
            "第一段 第二段\n结尾 "
        );
        assert_eq!(
            interrupt_round_with_reason(
                &mut connection,
                1,
                session_id,
                round_id,
                Some("迟到片段"),
                None,
                "timeout"
            )
            .unwrap(),
            completed
        );
        assert_eq!(
            complete_round(
                &mut connection,
                1,
                session_id,
                round_id,
                "重复完成不能覆盖",
                None,
            )
            .unwrap(),
            completed
        );
        assert_eq!(get(&connection, 1, session_id).unwrap().intimacy, 1);
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM messages WHERE round_id=?1",
                    [round_id],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            2
        );

        let next = start_round(&mut connection, 1, session_id, "新问题").unwrap();
        let next_id = next.round.id.parse().unwrap();
        interrupt_round_with_reason(
            &mut connection,
            1,
            session_id,
            next_id,
            Some("重启前正文 "),
            None,
            "length_limit",
        )
        .unwrap();
        continue_round(&mut connection, 1, session_id, next_id).unwrap();
        crate::db::initialize(&mut connection).unwrap();
        let recovered = round_snapshot(&connection, 1, session_id, next_id).unwrap();
        assert_eq!(
            recovered.round.stop_reason.as_deref(),
            Some("recovered_after_restart")
        );
        assert_eq!(recovered.assistant_message.unwrap().content, "重启前正文 ");
        assert!(continue_round(&mut connection, 1, session_id, next_id).is_err());
        assert_eq!(get(&connection, 1, session_id).unwrap().intimacy, 1);
    }

    #[test]
    fn continuation_rejects_other_reasons_missing_content_and_changed_tail() {
        let mut connection = database();
        let session_id = create(&mut connection, 1).unwrap().id.parse().unwrap();
        for (reason, content) in [("user_cancelled", Some("部分")), ("length_limit", None)] {
            let started = start_round(&mut connection, 1, session_id, "问题").unwrap();
            let id = started.round.id.parse().unwrap();
            interrupt_round_with_reason(&mut connection, 1, session_id, id, content, None, reason)
                .unwrap();
            assert!(continue_round(&mut connection, 1, session_id, id).is_err());
        }
        let started = start_round(&mut connection, 1, session_id, "问题").unwrap();
        let id = started.round.id.parse().unwrap();
        interrupt_round_with_reason(
            &mut connection,
            1,
            session_id,
            id,
            Some("部分"),
            None,
            "length_limit",
        )
        .unwrap();
        append_proactive_message(&mut connection, 1, session_id, "主动消息", None).unwrap();
        assert_eq!(last_round(&connection, 1, session_id).unwrap(), None);
        assert!(continue_round(&mut connection, 1, session_id, id).is_err());
        start_round(&mut connection, 1, session_id, "后续问题").unwrap();
        assert!(continue_round(&mut connection, 1, session_id, id).is_err());
        assert_eq!(
            round_snapshot(&connection, 1, session_id, id)
                .unwrap()
                .round
                .stop_reason
                .as_deref(),
            Some("length_limit")
        );
        assert!(interrupt_round_with_reason(
            &mut connection,
            1,
            session_id,
            id,
            None,
            None,
            "invalid"
        )
        .is_err());
    }

    #[test]
    fn rounds_are_atomic_idempotent_and_reject_cross_session_writes() {
        let mut connection = database();
        let first = create(&mut connection, 1).unwrap().id.parse().unwrap();
        let started = start_round(&mut connection, 1, first, " 你好 ").unwrap();
        let round_id = started.round.id.parse().unwrap();
        assert_eq!(started.round.status, ChatRoundStatus::Pending);
        assert_eq!(started.user_message.content, "你好");
        assert!(start_round(&mut connection, 1, first, "重复").is_err());
        let interrupted =
            interrupt_round(&mut connection, 1, first, round_id, Some("  "), None).unwrap();
        assert_eq!(interrupted.round.status, ChatRoundStatus::Interrupted);
        assert_eq!(interrupted.round.stop_reason.as_deref(), Some("unknown"));
        assert!(interrupted.assistant_message.is_none());
        assert_eq!(get(&connection, 1, first).unwrap().intimacy, 0);
        assert_eq!(
            complete_round(&mut connection, 1, first, round_id, "迟到回复", None).unwrap(),
            interrupted
        );
        assert_eq!(get(&connection, 1, first).unwrap().intimacy, 0);
        let revision: i64 = connection
            .query_row(
                "SELECT memory_revision FROM sessions WHERE id=?1",
                [first],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(revision, 2);

        let completed = start_round(&mut connection, 1, first, "第二轮").unwrap();
        let completed_id = completed.round.id.parse().unwrap();
        let result = complete_round(
            &mut connection,
            1,
            first,
            completed_id,
            "回答",
            Some("(^_^)"),
        )
        .unwrap();
        assert_eq!(result.round.status, ChatRoundStatus::Completed);
        assert_eq!(result.assistant_message.as_ref().unwrap().content, "回答");
        assert_eq!(get(&connection, 1, first).unwrap().intimacy, 1);
        let revision: i64 = connection
            .query_row(
                "SELECT memory_revision FROM sessions WHERE id=?1",
                [first],
                |row| row.get(0),
            )
            .unwrap();
        let repeated =
            complete_round(&mut connection, 1, first, completed_id, "不能覆盖", None).unwrap();
        assert_eq!(repeated.assistant_message.unwrap().content, "回答");
        assert_eq!(get(&connection, 1, first).unwrap().intimacy, 1);
        assert_eq!(
            connection
                .query_row(
                    "SELECT memory_revision FROM sessions WHERE id=?1",
                    [first],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            revision
        );

        let second = create(&mut connection, 1).unwrap().id.parse().unwrap();
        assert!(complete_round(&mut connection, 1, second, completed_id, "串会话", None).is_err());
        assert_eq!(get(&connection, 1, second).unwrap().intimacy, 0);
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM messages WHERE round_id=?1",
                    [completed_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            2
        );
    }

    #[test]
    fn completed_round_rewards_cap_intimacy_without_blocking_completion() {
        let mut connection = database();
        let session_id = create(&mut connection, 1).unwrap().id.parse().unwrap();
        connection
            .execute("UPDATE sessions SET intimacy=998 WHERE id=?1", [session_id])
            .unwrap();
        for content in ["达到上限", "满值仍能聊天"] {
            let pending = start_round(&mut connection, 1, session_id, content).unwrap();
            let completed = complete_round(
                &mut connection,
                1,
                session_id,
                pending.round.id.parse().unwrap(),
                "正常回复",
                None,
            )
            .unwrap();
            assert_eq!(completed.round.status, ChatRoundStatus::Completed);
            assert_eq!(completed.assistant_message.unwrap().content, "正常回复");
            assert_eq!(get(&connection, 1, session_id).unwrap().intimacy, 999);
        }
    }

    #[test]
    fn failed_intimacy_reward_rolls_back_round_message_and_revision() {
        let mut connection = database();
        let session_id = create(&mut connection, 1).unwrap().id.parse().unwrap();
        let pending = start_round(&mut connection, 1, session_id, "用户消息").unwrap();
        let round_id = pending.round.id.parse().unwrap();
        let session_before = get(&connection, 1, session_id).unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER reject_intimacy AFTER UPDATE OF intimacy ON sessions
                 BEGIN SELECT RAISE(ABORT,'reward failed'); END;",
            )
            .unwrap();
        assert!(complete_round(
            &mut connection,
            1,
            session_id,
            round_id,
            "不能半写的回复",
            None,
        )
        .is_err());
        assert_eq!(
            round_snapshot(&connection, 1, session_id, round_id).unwrap(),
            pending
        );
        assert_eq!(get(&connection, 1, session_id).unwrap(), session_before);
        connection
            .execute_batch("DROP TRIGGER reject_intimacy")
            .unwrap();
        let completed = complete_round(
            &mut connection,
            1,
            session_id,
            round_id,
            "重试成功的回复",
            None,
        )
        .unwrap();
        assert_eq!(completed.round.status, ChatRoundStatus::Completed);
        assert_eq!(
            completed.assistant_message.unwrap().content,
            "重试成功的回复"
        );
        let session_after = get(&connection, 1, session_id).unwrap();
        assert_eq!(session_after.intimacy, session_before.intimacy + 1);
        assert_eq!(
            session_after.memory_revision,
            session_before.memory_revision + 1
        );
    }

    #[test]
    fn proactive_messages_page_in_id_order_and_recovery_is_one_time() {
        let mut connection = database();
        let session_id = create(&mut connection, 1).unwrap().id.parse().unwrap();
        for index in 0..3 {
            let message = append_proactive_message(
                &mut connection,
                1,
                session_id,
                &format!("主动 {index}"),
                None,
            )
            .unwrap();
            assert!(message.round_id.is_none());
            assert_eq!(message.role, MessageRole::Assistant);
        }
        let first = messages(&connection, 1, session_id, None, 2).unwrap();
        assert_eq!(first.items.len(), 2);
        let next = first.next_after_id.as_ref().unwrap().parse().unwrap();
        let second = messages(&connection, 1, session_id, Some(next), 2).unwrap();
        assert_eq!(second.items.len(), 1);
        assert!(second.next_after_id.is_none());
        assert!(messages(&connection, 1, session_id, None, 0).is_err());

        let pending = start_round(&mut connection, 1, session_id, "未完成").unwrap();
        let before: i64 = connection
            .query_row(
                "SELECT memory_revision FROM sessions WHERE id=?1",
                [session_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            recover_pending(&mut connection, "2026-09-20T00:00:00.000Z").unwrap(),
            1
        );
        assert_eq!(
            recover_pending(&mut connection, "2026-09-20T00:01:00.000Z").unwrap(),
            0
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT memory_revision FROM sessions WHERE id=?1",
                    [session_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            before + 1
        );
        let recovered = round_snapshot(
            &connection,
            1,
            session_id,
            pending.round.id.parse().unwrap(),
        )
        .unwrap();
        assert_eq!(recovered.round.status, ChatRoundStatus::Interrupted);
        assert_eq!(
            recovered.round.stop_reason.as_deref(),
            Some("recovered_after_restart")
        );
        assert!(recovered.assistant_message.is_none());
    }

    #[test]
    fn failed_round_commit_rolls_back_messages_and_revision() {
        let mut connection = database();
        let session_id = create(&mut connection, 1).unwrap().id.parse().unwrap();
        connection.execute_batch("CREATE TRIGGER reject_message AFTER INSERT ON messages BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
        assert!(start_round(&mut connection, 1, session_id, "不会半写").is_err());
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM chat_rounds", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT memory_revision FROM sessions WHERE id=?1",
                    [session_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        connection
            .execute_batch("DROP TRIGGER reject_message")
            .unwrap();
        let pending = start_round(&mut connection, 1, session_id, "用户消息").unwrap();
        let round_id = pending.round.id.parse::<i64>().unwrap();
        connection.execute_batch("CREATE TRIGGER reject_round BEFORE UPDATE ON chat_rounds BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
        assert!(complete_round(
            &mut connection,
            1,
            session_id,
            round_id,
            "不能半写的回复",
            None
        )
        .is_err());
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM messages WHERE round_id=?1",
                    [round_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT status FROM chat_rounds WHERE id=?1",
                    [round_id],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            "pending"
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT memory_revision FROM sessions WHERE id=?1",
                    [session_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
    }

    #[test]
    fn database_initialization_recovers_orphan_pending_round_once() {
        let mut connection = database();
        let session_id = create(&mut connection, 1)
            .unwrap()
            .id
            .parse::<i64>()
            .unwrap();
        start_round(&mut connection, 1, session_id, "崩溃前已保存").unwrap();
        let before = connection
            .query_row(
                "SELECT memory_revision FROM sessions WHERE id=?1",
                [session_id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        crate::db::initialize(&mut connection).unwrap();
        assert_eq!(
            connection
                .query_row("SELECT status FROM chat_rounds", [], |row| row
                    .get::<_, String>(0))
                .unwrap(),
            "interrupted"
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT memory_revision FROM sessions WHERE id=?1",
                    [session_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            before + 1
        );
        crate::db::initialize(&mut connection).unwrap();
        assert_eq!(
            connection
                .query_row(
                    "SELECT memory_revision FROM sessions WHERE id=?1",
                    [session_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            before + 1
        );
    }
}
