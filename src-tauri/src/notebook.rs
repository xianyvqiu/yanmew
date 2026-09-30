use crate::{
    error::{AppError, AppResult},
    models::Notebook,
};
use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookView {
    pub notebook: Option<Notebook>,
    pub limit_chars: usize,
}

fn require_current(store: &crate::runtime::Store, persona_id: i64, session_id: i64, revision: u64) -> AppResult<()> {
    let context = crate::persona::context(&store.connection, store.context_revision)?;
    if context.persona_id != persona_id.to_string() || context.session_id != Some(session_id.to_string()) || context.context_revision != revision {
        return Err(AppError::new("CONTEXT_CHANGED", "当前人设或会话已经改变，请重新打开记事本"));
    }
    crate::sessions::get(&store.connection, persona_id, session_id)?;
    Ok(())
}

/// 管理窗口和 AI 共用同一仓储；手动保存同样比较读取时的完整快照。
pub fn current(runtime: &crate::runtime::Runtime, persona_id: i64, session_id: i64, revision: u64) -> AppResult<NotebookView> {
    runtime.read(|store, _| {
        require_current(store, persona_id, session_id, revision)?;
        let limit_chars = crate::config::get(&store.connection)?["notebook_max_chars"].parse().map_err(|_| AppError::storage())?;
        Ok(NotebookView { notebook: read(&store.connection, session_id)?, limit_chars })
    })
}
pub fn save_current(runtime: &crate::runtime::Runtime, persona_id: i64, session_id: i64, revision: u64,
    expected: Option<Notebook>, content: String) -> AppResult<NotebookView> {
    runtime.read(|store, _| {
        require_current(store, persona_id, session_id, revision)?;
        let limit_chars = crate::config::get(&store.connection)?["notebook_max_chars"].parse().map_err(|_| AppError::storage())?;
        let notebook = save(&mut store.connection, session_id, &expected, &content, runtime.clock.sample()?.utc)?;
        Ok(NotebookView { notebook: Some(notebook), limit_chars })
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    Append(String),
    Replace { section_index: usize, content: String },
    Delete { section_index: usize },
}

#[derive(Debug, Clone, Copy)]
struct Paragraph {
    start: usize,
    end: usize,
}

/// 将正文按空行分段，同时保留每个段落在原文中的精确字节范围。
/// 空行可包含空格或其他空白字符，LF 与 CRLF 均按原样保留。
fn paragraphs(content: &str) -> Vec<Paragraph> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, byte) in content.bytes().enumerate() {
        if byte == b'\n' {
            let mut body_end = index;
            if body_end > start && content.as_bytes()[body_end - 1] == b'\r' {
                body_end -= 1;
            }
            lines.push((start, body_end, index + 1));
            start = index + 1;
        }
    }
    if start < content.len() {
        let mut body_end = content.len();
        if body_end > start && content.as_bytes()[body_end - 1] == b'\r' {
            body_end -= 1;
        }
        lines.push((start, body_end, content.len()));
    }

    let mut result = Vec::new();
    let mut paragraph_start = None;
    let mut paragraph_end = 0;
    for (line_start, body_end, _line_end) in lines {
        let blank = content[line_start..body_end].trim().is_empty();
        if blank {
            if let Some(start) = paragraph_start.take() {
                result.push(Paragraph {
                    start,
                    end: paragraph_end,
                });
            }
        } else {
            paragraph_start.get_or_insert(line_start);
            paragraph_end = body_end;
        }
    }
    if let Some(start) = paragraph_start {
        result.push(Paragraph {
            start,
            end: paragraph_end,
        });
    }
    result
}

/// 返回每个非空段落的原文，不包含段落间的空行分隔符。
pub fn sections(content: &str) -> Vec<String> {
    paragraphs(content)
        .into_iter()
        .map(|paragraph| content[paragraph.start..paragraph.end].to_owned())
        .collect()
}

fn invalid_section() -> AppError {
    AppError::validation("sectionIndex", "段落编号不存在，编号从 1 开始")
}

fn require_text(content: &str) -> AppResult<()> {
    if content.trim().is_empty() {
        Err(AppError::validation("content", "段落内容不能为空"))
    } else {
        Ok(())
    }
}

fn line_ending(content: &str) -> &'static str {
    if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn ends_with_blank_line(content: &str) -> bool {
    if content.is_empty() || !content.ends_with('\n') {
        return false;
    }
    let line_start = content[..content.len() - 1]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let mut body_end = content.len() - 1;
    if body_end > line_start && content.as_bytes()[body_end - 1] == b'\r' {
        body_end -= 1;
    }
    content[line_start..body_end].trim().is_empty()
}

/// 在原文中直接替换或移除目标段落，未操作文本及原分隔符保持原样。
pub fn apply_edit(content: &str, edit: Edit) -> AppResult<String> {
    let ranges = paragraphs(content);
    match edit {
        Edit::Append(added) => {
            require_text(&added)?;
            if content.is_empty() {
                return Ok(added);
            }
            if ranges.is_empty() {
                return Ok(format!("{content}{}{added}", line_ending(content).repeat(2)));
            }
            if ends_with_blank_line(content) {
                Ok(format!("{content}{added}"))
            } else {
                Ok(format!(
                    "{content}{}{added}",
                    line_ending(content).repeat(2)
                ))
            }
        }
        Edit::Replace {
            section_index,
            content: replacement,
        } => {
            require_text(&replacement)?;
            let range = ranges
                .get(section_index.checked_sub(1).ok_or_else(invalid_section)?)
                .ok_or_else(invalid_section)?;
            let mut result = String::with_capacity(
                content.len() - (range.end - range.start) + replacement.len(),
            );
            result.push_str(&content[..range.start]);
            result.push_str(&replacement);
            result.push_str(&content[range.end..]);
            Ok(result)
        }
        Edit::Delete { section_index } => {
            let range = ranges
                .get(section_index.checked_sub(1).ok_or_else(invalid_section)?)
                .ok_or_else(invalid_section)?;
            let mut result = String::with_capacity(content.len() - (range.end - range.start));
            result.push_str(&content[..range.start]);
            result.push_str(&content[range.end..]);
            Ok(result)
        }
    }
}

fn validate_session(connection: &Connection, session_id: i64) -> AppResult<()> {
    if session_id <= 0 {
        return Err(AppError::validation("sessionId", "会话 ID 必须是正整数"));
    }
    let exists = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sessions WHERE id=?1)",
        [session_id],
        |row| row.get::<_, bool>(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(AppError::not_found())
    }
}

fn read_row(connection: &Connection, session_id: i64) -> AppResult<Option<Notebook>> {
    Ok(connection
        .query_row(
            "SELECT session_id,content,updated_at FROM notebooks WHERE session_id=?1",
            [session_id],
            |row| {
                Ok(Notebook {
                    session_id: row.get::<_, i64>(0)?.to_string(),
                    content: row.get(1)?,
                    updated_at: row.get(2)?,
                })
            },
        )
        .optional()?)
}

/// 读取会话记事本；缺少正文行时返回 None，且不创建空行。
pub fn read(connection: &Connection, session_id: i64) -> AppResult<Option<Notebook>> {
    validate_session(connection, session_id)?;
    read_row(connection, session_id)
}

/// 通过整行比较执行记事本的 compare-and-swap 保存。
pub fn save(
    connection: &mut Connection,
    session_id: i64,
    expected: &Option<Notebook>,
    content: &str,
    now: DateTime<Utc>,
) -> AppResult<Notebook> {
    if session_id <= 0 {
        return Err(AppError::validation("sessionId", "会话 ID 必须是正整数"));
    }
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    validate_session(&transaction, session_id)?;
    let current = read_row(&transaction, session_id)?;
    let expected_matches = match expected {
        None => current.is_none(),
        Some(expected) => {
            expected.session_id == session_id.to_string() && current.as_ref() == Some(expected)
        }
    };
    if !expected_matches {
        return Err(AppError::new(
            "NOTEBOOK_CONFLICT",
            "记事本已在其他位置更新，请重新读取后再保存",
        ));
    }

    let updated_at = if current.as_ref().is_some_and(|notebook| notebook.content == content) {
        current.as_ref().expect("已确认记事本存在").updated_at.clone()
    } else {
        now.to_rfc3339_opts(SecondsFormat::Millis, true)
    };
    transaction.execute(
        "INSERT INTO notebooks(session_id,content,updated_at) VALUES(?1,?2,?3)
         ON CONFLICT(session_id) DO UPDATE SET content=excluded.content,updated_at=excluded.updated_at",
        params![session_id, content, updated_at],
    )?;
    let saved = read_row(&transaction, session_id)?.ok_or_else(AppError::storage)?;
    transaction.commit()?;
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        crate::db::initialize(&mut connection).unwrap();
        connection
    }

    fn new_session(connection: &mut Connection) -> i64 {
        crate::sessions::create(connection, 1)
            .unwrap()
            .id
            .parse()
            .unwrap()
    }

    fn at(seconds: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(seconds, 0).unwrap()
    }

    #[test]
    fn read_is_empty_without_writing_and_validates_session() {
        let mut connection = database();
        let session_id = new_session(&mut connection);
        assert_eq!(read(&connection, session_id).unwrap(), None);
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM notebooks", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(read(&connection, 0).unwrap_err().code, "VALIDATION_ERROR");
        assert_eq!(read(&connection, 999).unwrap_err().code, "NOT_FOUND");
    }

    #[test]
    fn save_isolated_by_session_and_read_returns_persisted_row() {
        let mut connection = database();
        let first = new_session(&mut connection);
        let second = new_session(&mut connection);
        let first_saved = save(&mut connection, first, &None, "第一份", at(10)).unwrap();
        let second_saved = save(&mut connection, second, &None, "第二份", at(20)).unwrap();
        assert_eq!(read(&connection, first).unwrap(), Some(first_saved.clone()));
        assert_eq!(read(&connection, second).unwrap(), Some(second_saved));
        assert_eq!(first_saved.session_id, first.to_string());
        assert_eq!(
            save(&mut connection, 999, &None, "不存在", at(30))
                .unwrap_err()
                .code,
            "NOT_FOUND"
        );
    }

    #[test]
    fn compare_and_swap_rejects_stale_and_cross_session_expected_values() {
        let mut connection = database();
        let first = new_session(&mut connection);
        let second = new_session(&mut connection);
        let initial = save(&mut connection, first, &None, "v1", at(10)).unwrap();
        let latest = save(&mut connection, first, &Some(initial.clone()), "v2", at(20)).unwrap();
        assert_eq!(
            save(&mut connection, first, &Some(initial), "stale", at(30))
                .unwrap_err()
                .code,
            "NOTEBOOK_CONFLICT"
        );
        let foreign = save(&mut connection, second, &None, "other", at(40)).unwrap();
        assert_eq!(
            save(&mut connection, first, &Some(foreign), "cross-session", at(50))
                .unwrap_err()
                .code,
            "NOTEBOOK_CONFLICT"
        );
        assert_eq!(read(&connection, first).unwrap(), Some(latest));
    }

    #[test]
    fn save_preserves_large_unicode_content_and_timestamp_when_unchanged() {
        let mut connection = database();
        let session_id = new_session(&mut connection);
        let content = format!("{}🌙你好", "喵".repeat(20_000));
        let saved = save(&mut connection, session_id, &None, &content, at(10)).unwrap();
        assert_eq!(saved.content, content);
        assert_eq!(saved.updated_at, "1970-01-01T00:00:10.000Z");
        let unchanged = save(
            &mut connection,
            session_id,
            &Some(saved.clone()),
            &content,
            at(20),
        )
        .unwrap();
        assert_eq!(unchanged.updated_at, saved.updated_at);
    }

    #[test]
    fn sections_and_edits_preserve_crlf_blank_lines_and_single_newlines() {
        let original = "首段第一行\r\n首段第二行\r\n\r\n第二段\r\n  \t\r\n第三段";
        assert_eq!(
            sections(original),
            vec!["首段第一行\r\n首段第二行", "第二段", "第三段"]
        );
        let replaced = apply_edit(
            original,
            Edit::Replace {
                section_index: 2,
                content: "新段落🙂".into(),
            },
        )
        .unwrap();
        assert_eq!(
            replaced,
            "首段第一行\r\n首段第二行\r\n\r\n新段落🙂\r\n  \t\r\n第三段"
        );
        let single_newline = "一行\n同段第二行";
        assert_eq!(sections(single_newline), vec![single_newline]);
        assert_eq!(sections("第一段\n \t\n第二段"), vec!["第一段", "第二段"]);
    }

    #[test]
    fn append_replace_delete_and_invalid_indexes_work() {
        let appended = apply_edit("第一段\r\n\r\n第二段", Edit::Append("第三段".into())).unwrap();
        assert_eq!(
            appended,
            "第一段\r\n\r\n第二段\r\n\r\n第三段"
        );
        let deleted = apply_edit(
            "第一段\r\n\r\n第二段\r\n  \r\n第三段",
            Edit::Delete { section_index: 2 },
        )
        .unwrap();
        assert_eq!(sections(&deleted), vec!["第一段", "第三段"]);
        assert!(deleted.contains("\r\n\r\n\r\n  \r\n"));
        assert_eq!(apply_edit("", Edit::Append("首段".into())).unwrap(), "首段");
        for edit in [
            Edit::Replace {
                section_index: 0,
                content: "x".into(),
            },
            Edit::Delete { section_index: 0 },
            Edit::Replace {
                section_index: 2,
                content: "x".into(),
            },
            Edit::Delete { section_index: 2 },
        ] {
            assert_eq!(apply_edit("只有一段", edit).unwrap_err().code, "VALIDATION_ERROR");
        }
        assert!(apply_edit("有内容", Edit::Append(" \t ".into())).is_err());
        assert!(apply_edit(
            "有内容",
            Edit::Replace {
                section_index: 1,
                content: "\n".into(),
            }
        )
        .is_err());
    }
}
