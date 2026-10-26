//! 普通旅行伴手礼管理；展示项删除不修改不可变出行事实和发放标记。
use crate::{
    error::{AppError, AppResult},
    models::Souvenir,
};
use rusqlite::{params, Connection, OptionalExtension};
pub(crate) mod generation;
#[cfg(test)]
mod tests;

const COLUMNS: &str = "id,session_id,travel_id,content,note,is_favorite,created_at,image_file,title";
pub(crate) fn images_enabled(c:&Connection)->AppResult<bool>{
    let value:Option<String>=c.query_row("SELECT value FROM settings WHERE key='souvenir_images_enabled'",[],|r|r.get(0)).optional()?;
    match value.as_deref(){None|Some("1")=>Ok(true),Some("0")=>Ok(false),_=>Err(AppError::validation("souvenirImages","伴手礼图片设置无效"))}
}
pub(crate) fn set_images_enabled(c:&Connection,enabled:bool)->AppResult<bool>{
    c.execute("INSERT INTO settings(key,value) VALUES('souvenir_images_enabled',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[if enabled{"1"}else{"0"}])?;Ok(enabled)
}
pub(crate) fn image(runtime:&crate::runtime::Runtime,persona:i64,session:i64,revision:u64,id:i64)->AppResult<crate::accessories::images::ImageView>{
    let record=runtime.read(|store,_|{crate::accessories::require_current(store,persona,session,revision)?;get(&store.connection,session,id)})?;
    let result=record.image_file.as_ref().map(|file|runtime.paths.as_ref().map_err(Clone::clone).and_then(|paths|crate::accessories::images::read(paths,file)));
    runtime.read(|store,_|{crate::accessories::require_current(store,persona,session,revision)?;if get(&store.connection,session,id)?.image_file!=record.image_file{return Err(AppError::new("CONFLICT","伴手礼图片已变化，请刷新"));}Ok(())})?;
    Ok(match result {Some(Ok(data_url))=>crate::accessories::images::ImageView{data_url:Some(data_url),notice:None},Some(Err(_))=>crate::accessories::images::ImageView{data_url:None,notice:Some("伴手礼图片暂不可用，文字内容仍保留。".into())},None=>crate::accessories::images::ImageView{data_url:None,notice:None}})
}
fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Souvenir> {
    Ok(Souvenir {
        id: r.get::<_, i64>(0)?.to_string(),
        session_id: r.get::<_, i64>(1)?.to_string(),
        travel_id: r.get::<_, Option<i64>>(2)?.map(|id| id.to_string()),
        content: r.get(3)?,
        note: r.get(4)?,
        is_favorite: r.get(5)?,
        created_at: r.get(6)?,
        image_file:r.get(7)?,
        title:r.get(8)?,
    })
}
pub(crate) fn list(
    c: &Connection,
    session: i64,
    before: Option<i64>,
    limit: i64,
) -> AppResult<Vec<Souvenir>> {
    if !(1..=200).contains(&limit) || before.is_some_and(|id| id <= 0) {
        return Err(AppError::validation(
            "pagination",
            "伴手礼分页参数无效，每页须为1–200条",
        ));
    }
    let mut query=c.prepare(&format!("SELECT {COLUMNS} FROM souvenirs WHERE session_id=?1 AND (?2 IS NULL OR id<?2) ORDER BY id DESC LIMIT ?3"))?;
    let rows = query
        .query_map(params![session, before, limit], row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
pub(crate) fn get(c: &Connection, session: i64, id: i64) -> AppResult<Souvenir> {
    c.query_row(
        &format!("SELECT {COLUMNS} FROM souvenirs WHERE session_id=?1 AND id=?2"),
        params![session, id],
        row,
    )
    .optional()?
    .ok_or_else(AppError::not_found)
}
pub(crate) fn favorite(c: &Connection, session: i64, id: i64, value: bool) -> AppResult<Souvenir> {
    if c.execute(
        "UPDATE souvenirs SET is_favorite=?1 WHERE session_id=?2 AND id=?3",
        params![value, session, id],
    )? != 1
    {
        return Err(AppError::not_found());
    }
    get(c, session, id)
}
pub(crate) fn delete(c: &Connection, session: i64, id: i64) -> AppResult<()> {
    if c.execute(
        "DELETE FROM souvenirs WHERE session_id=?1 AND id=?2",
        params![session, id],
    )? != 1
    {
        return Err(AppError::not_found());
    }
    Ok(())
}
