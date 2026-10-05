//! 本地单实例 CPU 推理与联网向量调用。模型身份和内容哈希均不包含 API Key。
use crate::{
    embedding_providers::Stamp,
    error::{AppError, AppResult},
    runtime::Runtime,
    state::Cancellation,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf, sync::Mutex};

pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn content_hash(title: &str, body: &str) -> String {
    hash(format!("{}:{title}{body}", title.len()).as_bytes())
}
pub fn unavailable(message: &str) -> AppError {
    AppError::new("EMBEDDING_UNAVAILABLE", message)
}
pub fn normalize(mut vector: Vec<f32>) -> AppResult<Vec<f32>> {
    if vector.is_empty() || vector.len() > 16384 || vector.iter().any(|n| !n.is_finite()) {
        return Err(unavailable("Embedding 返回的向量维度或数值无效"));
    }
    let norm = vector
        .iter()
        .map(|n| (*n as f64).powi(2))
        .sum::<f64>()
        .sqrt();
    if !norm.is_finite() || norm < 1e-12 {
        return Err(unavailable("Embedding 返回了零向量"));
    }
    for value in &mut vector {
        *value = (*value as f64 / norm) as f32;
    }
    Ok(vector)
}
pub fn encode(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|v| v.to_le_bytes()).collect()
}
pub fn decode(bytes: &[u8], dimensions: usize) -> AppResult<Vec<f32>> {
    if bytes.len() != dimensions.saturating_mul(4) {
        return Err(unavailable("向量缓存维度不匹配"));
    }
    normalize(
        bytes
            .chunks_exact(4)
            .map(|v| f32::from_le_bytes(v.try_into().unwrap()))
            .collect(),
    )
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    model: String,
    revision: String,
    dimensions: usize,
    max_sequence: usize,
    pooling: String,
    normalize: String,
    query_prefix: String,
    runtime: String,
    files: BTreeMap<String, String>,
}
struct LocalModel {
    session: ort::session::Session,
    tokenizer: tokenizers::Tokenizer,
    manifest: Manifest,
    identity: String,
}
#[derive(Default)]
struct LocalState {
    model: Option<LocalModel>,
    failure: Option<AppError>,
}
pub struct Engine {
    root: AppResult<PathBuf>,
    state: Mutex<LocalState>,
}
impl Engine {
    pub fn new(root: AppResult<PathBuf>) -> Self {
        Self {
            root,
            state: Mutex::new(LocalState::default()),
        }
    }
    pub fn reset_failure(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.failure = None;
            state.model = None;
        }
    }
    pub fn local_identity(&self) -> AppResult<String> {
        let root = self.root.as_ref().map_err(Clone::clone)?;
        let manifest = std::fs::read(root.join("manifest.json"))
            .map_err(|_| unavailable("本地 Embedding 资源缺失，请补齐便携版模型资源后测试连接"))?;
        Ok(format!("local:{}", hash(&manifest)))
    }
    fn load(&self) -> AppResult<LocalModel> {
        let root = self.root.as_ref().map_err(Clone::clone)?;
        let bytes = std::fs::read(root.join("manifest.json"))
            .map_err(|_| unavailable("本地 Embedding 模型资源缺失，请补齐资源后测试连接"))?;
        let manifest: Manifest =
            serde_json::from_slice(&bytes).map_err(|_| unavailable("本地模型资源清单无效"))?;
        if manifest.model != "BAAI/bge-small-zh-v1.5"
            || manifest.dimensions != 512
            || manifest.max_sequence != 512
            || manifest.pooling != "CLS"
            || manifest.normalize != "L2"
            || manifest.runtime != "1.22.0"
            || manifest.revision.is_empty()
        {
            return Err(unavailable("本地模型资源版本不兼容"));
        }
        for name in [
            "model.onnx",
            "tokenizer.json",
            "config.json",
            "onnxruntime.dll",
        ] {
            let path = root.join(name);
            // 大文件流式校验，避免为校验复制整份权重。
            let mut file =
                std::fs::File::open(path).map_err(|_| unavailable("本地模型文件缺失"))?;
            let mut digest = Sha256::new();
            use std::io::Read;
            let mut buffer = [0u8; 65536];
            loop {
                let count = file
                    .read(&mut buffer)
                    .map_err(|_| unavailable("本地模型文件读取失败"))?;
                if count == 0 {
                    break;
                }
                digest.update(&buffer[..count]);
            }
            if manifest.files.get(name) != Some(&format!("{:x}", digest.finalize())) {
                return Err(unavailable("本地模型文件校验失败，请恢复原始资源"));
            }
        }
        let dll = root.join("onnxruntime.dll");
        // 绝对受管路径加载；不搜索用户 PATH。ort 环境为进程全局，模型 session 只创建一次。
        ort::init_from(dll.to_string_lossy())
            .with_name("yanmew-embedding")
            .commit()
            .map_err(|_| unavailable("本地推理运行库加载失败"))?;
        let threads = std::thread::available_parallelism()
            .map(|n| n.get().min(4))
            .unwrap_or(1);
        let session = ort::session::Session::builder()
            .and_then(|s| s.with_intra_threads(threads))
            .and_then(|s| s.with_parallel_execution(false))
            .and_then(|s| s.with_config_entry("session.intra_op.allow_spinning", "0"))
            .and_then(|s| s.with_config_entry("session.inter_op.allow_spinning", "0"))
            .and_then(|s| s.commit_from_file(root.join("model.onnx")))
            .map_err(|_| unavailable("本地 Embedding 模型加载失败"))?;
        let mut tokenizer = tokenizers::Tokenizer::from_file(root.join("tokenizer.json"))
            .map_err(|_| unavailable("本地分词器加载失败"))?;
        tokenizer
            .with_truncation(None)
            .map_err(|_| unavailable("本地分词配置无效"))?;
        tokenizer.with_padding(None);
        Ok(LocalModel {
            session,
            tokenizer,
            manifest,
            identity: format!("local:{}", hash(&bytes)),
        })
    }
    pub fn local(&self, text: &str, query: bool, cancel: &Cancellation) -> AppResult<Vector> {
        cancel.check()?;
        let mut state = self.state.lock().map_err(|_| AppError::storage())?;
        cancel.check()?;
        if let Some(error) = &state.failure {
            return Err(error.clone());
        }
        if state.model.is_none() {
            let loaded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.load()))
                .unwrap_or_else(|_| {
                    Err(unavailable(
                        "本地推理运行库不兼容或无法加载，请恢复原始资源后重试",
                    ))
                });
            match loaded {
                Ok(model) => state.model = Some(model),
                Err(error) => {
                    state.failure = Some(error.clone());
                    return Err(error);
                }
            }
        }
        let model = state.model.as_mut().unwrap();
        let text = if query {
            format!("{}{text}", model.manifest.query_prefix)
        } else {
            text.to_owned()
        };
        let encoded = model
            .tokenizer
            .encode(text, true)
            .map_err(|_| unavailable("Embedding 分词失败"))?;
        let length = encoded.len();
        if length > model.manifest.max_sequence {
            return Err(AppError::new(
                "EMBEDDING_TOO_LONG",
                "内容超过本地模型 512 token 范围，本条仅使用全文检索",
            ));
        }
        let tensor = |values: &[u32]| {
            ort::value::Tensor::from_array((
                [1usize, length],
                values
                    .iter()
                    .map(|n| *n as i64)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ))
        };
        let result = (|| -> ort::Result<Vec<f32>> {
            let output = model.session.run(ort::inputs!["input_ids" => tensor(encoded.get_ids())?, "attention_mask" => tensor(encoded.get_attention_mask())?, "token_type_ids" => tensor(encoded.get_type_ids())?])?;
            let (shape, values) = output["embedding"].try_extract_tensor::<f32>()?;
            if shape.as_ref() != [1, 512] {
                return Ok(vec![]);
            }
            Ok(values.to_vec())
        })();
        cancel.check()?;
        match result {
            Ok(values) => match normalize(values) {
                Ok(values) => Ok(Vector {
                    model_key: format!("{}:512", model.identity),
                    values,
                }),
                Err(error) => {
                    state.failure = Some(error.clone());
                    Err(error)
                }
            },
            Err(_) => {
                let error =
                    unavailable("本地 Embedding 推理失败，请测试连接后重试；全文检索仍可使用");
                state.failure = Some(error.clone());
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests;
#[derive(Debug)]
pub struct Vector {
    pub model_key: String,
    pub values: Vec<f32>,
}
pub fn identity(engine: &Engine, stamp: &Stamp) -> AppResult<String> {
    if stamp.kind == "local" {
        engine.local_identity()
    } else {
        Ok(format!(
            "online:{}",
            hash(
                format!(
                    "{}\n{}",
                    stamp
                        .base_url
                        .as_deref()
                        .unwrap_or_default()
                        .trim_end_matches('/'),
                    stamp.model
                )
                .as_bytes()
            )
        ))
    }
}
pub async fn generate(
    runtime: &Runtime,
    stamp: &Stamp,
    text: String,
    query: bool,
    cancel: &Cancellation,
) -> AppResult<Vector> {
    cancel.check()?;
    if stamp.kind == "local" {
        let engine = runtime.embedding.clone();
        let signal = cancel.clone();
        tauri::async_runtime::spawn_blocking(move || engine.local(&text, query, &signal))
            .await
            .map_err(|_| unavailable("本地推理任务异常"))?
    } else {
        let config = stamp.connection(runtime.secrets.as_ref())?;
        let values = runtime
            .ai
            .as_ref()
            .map_err(Clone::clone)?
            .embedding(&config, &text, cancel)
            .await?;
        let values = normalize(values)?;
        Ok(Vector {
            model_key: format!("{}:{}", identity(&runtime.embedding, stamp)?, values.len()),
            values,
        })
    }
}
