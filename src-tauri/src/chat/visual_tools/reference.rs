//! 聊天绘图只引用当前人设正式默认图，不开放路径或其他人设选择。
use super::*;
pub(super) struct Reference { id: i64, revision: u64, file: String, name: String, setting: String }
impl Reference {
    fn from_store(c: &rusqlite::Connection, persona: i64) -> AppResult<Self> {
        let library = crate::pet_creation::library(c, persona)?;
        if !library.enabled { return Err(AppError::validation("mode", "当前不是图片模式，无法使用默认形象参考图")); }
        let base = library.items.into_iter().find(|a| a.kind == "base" && a.image_file.is_some()).ok_or_else(|| AppError::new("BASE_IMAGE_MISSING", "当前人设没有已接受的默认形象，未请求参考图生图"))?;
        let (name, setting) = c.query_row("SELECT name,prompt FROM personas WHERE id=?1", [persona], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(Self { id: crate::error::parse_id(&base.id)?, revision: base.revision, file: base.image_file.unwrap(), name, setting })
    }
    pub fn capture(runtime: &Runtime, lease: &RequestLease, persona: i64, session: i64, revision: u64) -> AppResult<Self> {
        runtime.read(|s, _| { lease.require_conversation_scope(&runtime.requests, session, persona)?; coordinator::require_context(s, persona, session, revision)?; Self::from_store(&s.connection, persona) })
    }
    pub fn verify_store(&self, c: &rusqlite::Connection, persona: i64) -> AppResult<()> {
        let current = Self::from_store(c, persona)?;
        if current.id != self.id || current.revision != self.revision || current.file != self.file || current.name != self.name || current.setting != self.setting {
            return Err(AppError::new("CONTEXT_CHANGED", "默认形象或人设已改变，旧生成结果未加入聊天；不会自动重新生图"));
        }
        Ok(())
    }
    pub fn verify(&self, runtime: &Runtime, lease: &RequestLease, persona: i64, session: i64, revision: u64) -> AppResult<()> {
        runtime.read(|s, _| { lease.cancel.check()?; lease.require_conversation_scope(&runtime.requests, session, persona)?; coordinator::require_context(s, persona, session, revision)?; self.verify_store(&s.connection, persona) })
    }
    pub async fn read(&self, runtime: &Runtime, persona: i64) -> AppResult<Vec<u8>> {
        let _=persona;let paths=runtime.paths.as_ref().map_err(Clone::clone)?.clone();let file=self.file.clone();
        tauri::async_runtime::spawn_blocking(move || crate::pet_creation::reference_png(&paths,&file)).await.map_err(|_|AppError::storage())?
    }
    pub fn prompt(&self, request: &str) -> String {
        json!({"userDrawingRequest":request,"currentPersona":{"name":self.name,"setting":self.setting}}).to_string()
    }
}

#[cfg(test)] mod tests {
 use super::*;
 #[test] fn reference_requires_enabled_accepted_current_base_and_rechecks_changes() {
  let c=crate::db::test_database();assert!(Reference::from_store(&c,1).is_err());
  c.execute_batch("UPDATE personas SET pet_image_enabled=1 WHERE id=1;").unwrap();assert!(Reference::from_store(&c,1).is_err());
  c.execute_batch("INSERT INTO pet_artworks(persona_id,kind,name,category,preview_file,created_at,updated_at) VALUES(1,'base','默认','normal','preview.png','a','a');").unwrap();assert!(Reference::from_store(&c,1).is_err());
  c.execute("UPDATE pet_artworks SET image_file='accepted.png' WHERE persona_id=1",[]).unwrap();let reference=Reference::from_store(&c,1).unwrap();reference.verify_store(&c,1).unwrap();
  c.execute("UPDATE pet_artworks SET image_file='changed.png',revision=revision+1 WHERE persona_id=1",[]).unwrap();assert_eq!(reference.verify_store(&c,1).unwrap_err().code,"CONTEXT_CHANGED");
 }
 #[test] fn reference_rechecks_persona_setting_and_excludes_other_personas() {
  let c=crate::db::test_database();c.execute_batch("UPDATE personas SET pet_image_enabled=1 WHERE id=1;INSERT INTO pet_artworks(persona_id,kind,name,category,image_file,created_at,updated_at) VALUES(1,'base','默认','normal','accepted.png','a','a');").unwrap();let reference=Reference::from_store(&c,1).unwrap();
  c.execute("UPDATE personas SET prompt='用户刚改的人设' WHERE id=1",[]).unwrap();assert_eq!(reference.verify_store(&c,1).unwrap_err().code,"CONTEXT_CHANGED");assert!(Reference::from_store(&c,999).is_err());
 }
}
