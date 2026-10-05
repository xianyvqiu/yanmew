//! 保守的本地输出守卫；只报告错误，消息保存由原协调器完成。
use crate::error::{AppError, AppResult};
use serde_json::Value;
use std::collections::{HashSet, VecDeque};

// 单位均为 Unicode 字符或采样次数，不依赖供应商分片大小。
const TEXT_WINDOW: usize = 2048;
const REPEAT_MIN_CHARS: usize = 1024;
const REPEAT_SAMPLE_EVERY: usize = 128;
const REPEAT_PERIOD_MIN: usize = 8;
const REPEAT_PERIOD_MAX: usize = 256;
const REPEAT_MIN_CYCLES: usize = 8;
const REPEAT_MATCH_PERCENT: usize = 100;
const REPEAT_NOVEL_PERCENT_MAX: usize = 35;
const REPEAT_HITS: usize = 4;
const ENCODING_WINDOW: usize = 256;
const ENCODING_SAMPLE_EVERY: usize = 64;
const ENCODING_BAD_PERCENT: usize = 25;
const ENCODING_HITS: usize = 3;
const IDENTICAL_TOOL_ROUNDS: usize = 3;

/// 映射至已有数据库枚举；检测类错误不归入提供商连接故障。
pub(super) fn stop_reason(code: &str) -> Option<&'static str> {
    match code {
        "CHAT_REPETITION" => Some("repetition"),
        "CHAT_INVALID_ENCODING" => Some("invalid_encoding"),
        "CHAT_NO_PROGRESS" | "CHAT_FIRST_RESPONSE_TIMEOUT" | "CHAT_TOTAL_TIMEOUT" => {
            Some("timeout")
        }
        "CHAT_TOOL_NO_PROGRESS" => Some("tool_limit"),
        _ => None,
    }
}

#[derive(Default)]
pub(super) struct TextGuard {
    text: VecDeque<char>,
    encoding: VecDeque<bool>,
    encoding_bad: usize,
    encoding_since_sample: usize,
    encoding_hits: usize,
    repeat_since_sample: usize,
    repeat_hits: usize,
    fence: Option<char>,
    delimiter: Option<char>,
    delimiter_run: usize,
}
impl TextGuard {
    pub(super) fn observe(&mut self, text: &str) -> AppResult<()> {
        for character in text.chars() {
            self.observe_encoding(character)?;
            // 代码块可以合法包含成片相同模板；其重复交给输出上限和总时长保护。
            if matches!(character, '`' | '~') {
                self.delimiter_run = if self.delimiter == Some(character) {
                    self.delimiter_run + 1
                } else {
                    1
                };
                self.delimiter = Some(character);
                if self.delimiter_run == 3
                    && (self.fence.is_none() || self.fence == Some(character))
                {
                    self.fence = if self.fence.is_some() {
                        None
                    } else {
                        Some(character)
                    };
                    self.text.clear();
                    self.repeat_hits = 0;
                    self.repeat_since_sample = 0;
                }
                continue;
            }
            self.delimiter = None;
            self.delimiter_run = 0;
            if self.fence.is_some() {
                continue;
            }
            self.text.push_back(character);
            if self.text.len() > TEXT_WINDOW {
                self.text.pop_front();
            }
            self.repeat_since_sample += 1;
            if self.repeat_since_sample >= REPEAT_SAMPLE_EVERY {
                self.repeat_since_sample = 0;
                self.repeat_hits = if self.repeats() {
                    self.repeat_hits + 1
                } else {
                    0
                };
                if self.repeat_hits >= REPEAT_HITS {
                    return Err(AppError::new(
                        "CHAT_REPETITION",
                        "检测到持续重复输出，已停止生成；已收到的内容已保存。",
                    ));
                }
            }
        }
        Ok(())
    }

    fn observe_encoding(&mut self, character: char) -> AppResult<()> {
        let bad = character == '\u{fffd}'
            || (character.is_control() && !matches!(character, '\n' | '\r' | '\t'));
        self.encoding.push_back(bad);
        self.encoding_bad += usize::from(bad);
        if self.encoding.len() > ENCODING_WINDOW {
            self.encoding_bad -= usize::from(self.encoding.pop_front().unwrap());
        }
        self.encoding_since_sample += 1;
        if self.encoding_since_sample >= ENCODING_SAMPLE_EVERY {
            self.encoding_since_sample = 0;
            let abnormal = self.encoding.len() == ENCODING_WINDOW
                && self.encoding_bad * 100 >= ENCODING_WINDOW * ENCODING_BAD_PERCENT;
            self.encoding_hits = if abnormal { self.encoding_hits + 1 } else { 0 };
            if self.encoding_hits >= ENCODING_HITS {
                return Err(AppError::new(
                    "CHAT_INVALID_ENCODING",
                    "检测到持续异常编码字符，已停止生成；已收到的内容已保存。",
                ));
            }
        }
        Ok(())
    }

    fn repeats(&self) -> bool {
        if self.text.len() < REPEAT_MIN_CHARS {
            return false;
        }
        let text: Vec<_> = self.text.iter().copied().collect();
        // 保守排除常见代码、表格和结构化数据，避免格式重复误伤。
        if text.iter().any(|c| {
            matches!(
                c,
                '{' | '}' | '[' | ']' | '=' | '<' | '>' | '|' | '\\' | ';'
            )
        }) {
            return false;
        }
        let unique: HashSet<_> = text.windows(3).map(|v| [v[0], v[1], v[2]]).collect();
        if unique.len() * 100 > (text.len() - 2) * REPEAT_NOVEL_PERCENT_MAX {
            return false;
        }
        for period in REPEAT_PERIOD_MIN..=REPEAT_PERIOD_MAX.min(text.len() / REPEAT_MIN_CYCLES) {
            let unit = &text[text.len() - period..];
            let letters: HashSet<_> = unit.iter().filter(|c| c.is_alphabetic()).copied().collect();
            // 单个词、标点或常见颜文字重复，不足以判定为异常。
            if letters.len() < 4 || unit.iter().filter(|c| c.is_alphabetic()).count() < 6 {
                continue;
            }
            let matching = text
                .iter()
                .rev()
                .enumerate()
                .filter(|(offset, c)| **c == unit[period - 1 - offset % period])
                .count();
            if matching * 100 >= text.len() * REPEAT_MATCH_PERCENT {
                // 不能把一个短词重复两遍伪装成符合最短长度的句段。
                let composite = (1..period).filter(|size| period % size == 0).any(|size| {
                    unit.iter()
                        .enumerate()
                        .all(|(index, c)| *c == unit[index % size])
                });
                if composite {
                    continue;
                }
                let boundaries = (0..period)
                    .filter(|index| {
                        !unit[*index].is_alphabetic()
                            && unit[(*index + period - 1) % period].is_alphabetic()
                    })
                    .count();
                let has_cjk = unit.iter().any(|c| matches!(*c, '\u{4e00}'..='\u{9fff}'));
                if boundaries < if has_cjk { 1 } else { 2 } {
                    continue;
                }
                return true;
            }
        }
        false
    }
}

/// 不把工具 ID 当进展；名称、结构化参数和完整结果均相同才累计。
#[derive(Default)]
pub(super) struct ToolGuard {
    previous: Vec<(String, Value, Value)>,
    repeated: usize,
}
impl ToolGuard {
    pub(super) fn observe(&mut self, mut round: Vec<(String, Value, Value)>) -> AppResult<()> {
        for (_, _, value) in &mut round {
            if let Some(result) = value.get_mut("result").and_then(Value::as_object_mut) {
                // 仅忽略宿主协议元数据，业务记录 ID、正文与状态仍参与比较。
                for key in ["request_id", "tool_call_id", "already_applied"] {
                    result.remove(key);
                }
            }
        }
        self.repeated = if !round.is_empty() && round == self.previous {
            self.repeated + 1
        } else {
            1
        };
        self.previous = round;
        if self.repeated >= IDENTICAL_TOOL_ROUNDS {
            Err(AppError::new(
                "CHAT_TOOL_NO_PROGRESS",
                "工具连续返回相同结果且没有进展，已停止本轮；已完成的操作仍然保留。",
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;
