//! 流式ZIP：每次只有一行业务数据或一张图片，校验和随写入累计。
use super::*;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    io::{Read, Seek, Write},
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

pub(super) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn io_error(_: std::io::Error) -> AppError {
    AppError::storage()
}
fn zip_error(_: zip::result::ZipError) -> AppError {
    AppError::new(
        "BACKUP_WRITE_FAILED",
        "备份压缩失败，请检查磁盘空间与目录权限",
    )
}

struct HashedWriter<W> {
    inner: W,
    hash: Sha256,
    size: u64,
}
impl<W: Write> Write for HashedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(bytes)?;
        self.hash.update(&bytes[..n]);
        self.size += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

pub(super) fn write<W: Write + Seek>(
    runtime: &Runtime,
    c: &rusqlite::Connection,
    scope: &snapshot::Scope,
    pinned: snapshot::PinnedAssets,
    writer: W,
) -> AppResult<(Manifest, W)> {
    snapshot::validate(c, scope)?;
    let mut zip = ZipWriter::new(writer);
    // ZIP64支持大文件，不把格式默认值变成用户容量上限。
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .large_file(true);
    zip.add_directory("files/", options).map_err(zip_error)?;
    let mut references = snapshot::AssetReferences::new();
    let mut files = Vec::new();
    let mut written = BTreeSet::new();
    for ((background, name), file) in pinned {
        let Some(mut file) = file else {
            references.insert((background, name), None);
            continue;
        };
        // 只保留既有单张图片校验，不设置聊天/JSON/ZIP的总量上限。
        let chat=name.starts_with("assets/chat_images/");
        let max = if chat {usize::MAX-1} else if background {
            crate::persona::background::MAX_BYTES
        } else {
            crate::accessories::images::MAX_BYTES
        };
        let mut bytes = Vec::new();
        (&mut file)
            .take(max as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(io_error)?;
        let extension = if chat {
            match image::guess_format(&bytes).map_err(|_|AppError::storage())? {image::ImageFormat::Png=>"png",image::ImageFormat::Jpeg=>"jpg",image::ImageFormat::WebP=>"webp",image::ImageFormat::Gif=>"gif",_=>return Err(AppError::storage())}
        } else if background {
            crate::persona::background::validate(&name, &bytes)?
        } else {
            crate::accessories::images::validate_png(&bytes)?;
            "png"
        };
        let sha256 = hash(&bytes);
        let path = format!("files/{sha256}.{extension}");
        if written.insert(path.clone()) {
            zip.start_file(&path, options).map_err(zip_error)?;
            zip.write_all(&bytes).map_err(io_error)?;
            files.push(FileEntry {
                path: path.clone(),
                size: bytes.len() as u64,
                sha256,
            });
        }
        references.insert((background, name), Some(path));
    }
    let mut record_counts = BTreeMap::new();
    let mut warnings = Vec::new();
    zip.start_file("data.json", options).map_err(zip_error)?;
    let (data_size, data_sha256) = {
        let mut data = HashedWriter {
            inner: &mut zip,
            hash: Sha256::new(),
            size: 0,
        };
        data.write_all(b"{").map_err(io_error)?;
        let mut tables = super::protocol::COLUMNS
            .iter()
            .filter(|(name, _)| scope.includes(name))
            .collect::<Vec<_>>();
        tables.sort_by_key(|(name, _)| *name);
        for (index, (table, columns)) in tables.into_iter().enumerate() {
            if index > 0 {
                data.write_all(b",").map_err(io_error)?;
            }
            serde_json::to_writer(&mut data, table).map_err(|_| AppError::storage())?;
            data.write_all(b":[").map_err(io_error)?;
            let names = columns.split(',').collect::<Vec<_>>();
            let mut statement = c.prepare(&scope.query(table, columns))?;
            let mut rows = statement.query([])?;
            let mut count = 0u64;
            while let Some(row) = rows.next()? {
                let record = super::protocol::record(row, &names)?;
                let Some(record) =
                    snapshot::project(c, table, record, scope, &references, &mut warnings)?
                else {
                    continue;
                };
                if count > 0 {
                    data.write_all(b",").map_err(io_error)?;
                }
                serde_json::to_writer(&mut data, &record).map_err(|_| AppError::storage())?;
                count += 1;
            }
            data.write_all(b"]").map_err(io_error)?;
            record_counts.insert((*table).into(), count);
        }
        data.write_all(b"}").map_err(io_error)?;
        (data.size, format!("{:x}", data.hash.finalize()))
    };
    if ["game_plugin_data", "game_plugin_settings"]
        .iter()
        .any(|t| record_counts.get(*t).copied().unwrap_or(0) > 0)
    {
        snapshot::warn(
            &mut warnings,
            "PLUGIN_PRIVATE_DATA",
            "game_plugin_data",
            "",
            "插件私有数据原样备份；插件自行保存的服务凭据可能包含在其中，请妥善保管备份。",
        );
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let date = runtime.clock.sample()?.utc;
    let manifest = Manifest {
        format_version: FORMAT_VERSION,
        schema_version: SCHEMA_VERSION,
        app_version: env!("CARGO_PKG_VERSION").into(),
        package_type: scope.request.kind,
        exported_at: date.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        selection: scope.request.clone(),
        record_counts,
        data_size,
        data_sha256,
        files,
        warnings,
    };
    zip.start_file("manifest.json", options)
        .map_err(zip_error)?;
    serde_json::to_writer(&mut zip, &manifest).map_err(|_| AppError::storage())?;
    let writer = zip.finish().map_err(zip_error)?;
    Ok((manifest, writer))
}
