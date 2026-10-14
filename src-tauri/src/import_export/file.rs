//! 临时文件完成、刷新并校验后才提交正式备份；失败保留已有备份。
use super::*;
use crate::platform::contracts::FileTransferPort;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufWriter, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub(super) fn save(
    runtime: &Runtime,
    request: ExportRequest,
    destination: &Path,
    transfer: &dyn FileTransferPort,
) -> AppResult<ExportArchive> {
    request.validate()?;
    if !destination
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
    {
        return Err(AppError::validation("file", "请选择ZIP备份文件名"));
    }
    let parent = destination
        .parent()
        .ok_or_else(AppError::storage)?
        .canonicalize()
        .map_err(|_| AppError::storage())?;
    let name = destination.file_name().ok_or_else(AppError::storage)?;
    let destination = parent.join(name);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| AppError::storage())?
        .as_nanos();
    let temporary_path = parent.join(format!(
        ".yanmew-backup-{}-{stamp}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary_path)
        .map_err(|_| AppError::storage())?;
    let temporary = Temporary(temporary_path);
    // 同一保存目标目录中临时写入，发布不需要跨磁盘复制。
    let prepared = runtime.read(|store, _| {
        snapshot::independent(
            runtime,
            &store.connection,
            &request,
            temporary.0.file_name().ok_or_else(AppError::storage)?,
        )
    })?;
    let writer = BufWriter::new(output);
    let (manifest, writer) = match prepared {
        Some(mut source) => {
            let pinned = std::mem::take(&mut source.pinned);
            archive::write(runtime, &source.connection, &source.scope, pinned, writer)?
        }
        None => {
            // 仅单元测试使用内存数据库；真实运行必须使用独立只读连接。
            #[cfg(test)]
            {
                runtime.read(|store, _| {
                    let scope = snapshot::initialize(&store.connection, &request)?;
                    let pinned = snapshot::pin(runtime, &store.connection, &scope)?;
                    archive::write(runtime, &store.connection, &scope, pinned, writer)
                })?
            }
            #[cfg(not(test))]
            {
                return Err(AppError::storage());
            }
        }
    };
    let file = writer.into_inner().map_err(|_| AppError::storage())?;
    file.sync_all().map_err(|_| AppError::storage())?;
    drop(file);
    let (size, sha256) = {
        let mut file = File::open(&temporary.0).map_err(|_| AppError::storage())?;
        let mut hash = Sha256::new();
        let mut size = 0u64;
        let mut buffer = [0u8; 65536];
        loop {
            let n = file.read(&mut buffer).map_err(|_| AppError::storage())?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
            size += n as u64;
        }
        (size, format!("{:x}", hash.finalize()))
    };
    transfer.publish(&temporary.0, &destination)?;
    Ok(ExportArchive {
        filename: name.to_string_lossy().into_owned(),
        size,
        sha256,
        manifest,
    })
}
