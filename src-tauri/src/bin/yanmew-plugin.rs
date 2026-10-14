//! 作者构建工具与主程序共用包校验器，不运行插件代码。
use std::{
    error::Error,
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
};
use yanwenzi_lib::game_plugins::{manifest, package};
fn collect(
    root: &Path,
    dir: &Path,
    files: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), Box<dyn Error>> {
    for e in fs::read_dir(dir)? {
        let p = e?.path();
        let m = fs::symlink_metadata(&p)?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if m.file_attributes() & 0x400 != 0 {
                return Err("不允许 junction 或重解析点".into());
            }
        }
        if m.file_type().is_symlink() {
            return Err("不允许符号链接".into());
        }
        if m.is_dir() {
            collect(root, &p, files)?;
        } else if m.is_file() {
            let relative = p
                .strip_prefix(root)?
                .to_str()
                .ok_or("路径必须是 UTF-8")?
                .replace('\\', "/");
            manifest::path(&relative)?;
            if package::mime(&relative).is_none() || m.len() > package::MAX_FILE as u64 {
                return Err("不支持的文件格式或单文件过大".into());
            }
            files.push((relative, fs::read(p)?));
            if files.len() > package::MAX_FILES
                || files.iter().map(|(_, b)| b.len()).sum::<usize>() > package::MAX_EXPANDED
            {
                return Err("包展开容量或文件数量超限".into());
            }
        } else {
            return Err("只允许普通文件和目录".into());
        }
    }
    Ok(())
}
fn execute() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [mode, file] if mode == "validate" => {
            let file = PathBuf::from(file);
            if fs::metadata(&file)?.len() > package::MAX_PACKAGE as u64 {
                return Err("插件包超过 64 MiB".into());
            }
            let pkg = package::validate(fs::read(file)?)?;
            println!(
                "{} {} 校验通过，SHA-256 {}",
                pkg.manifest.id, pkg.manifest.version, pkg.sha
            );
        }
        [mode, source, destination] if mode == "pack" => {
            let root = PathBuf::from(source).canonicalize()?;
            let out = PathBuf::from(destination);
            if out.exists() || out.extension().and_then(|e| e.to_str()) != Some("yanmew-game") {
                return Err("输出须为不存在的 .yanmew-game 文件".into());
            }
            let mut files = vec![];
            collect(&root, &root, &mut files)?;
            files.sort_by(|a, b| a.0.cmp(&b.0));
            let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
            for (name, data) in files {
                zip.start_file(
                    name,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated)
                        .unix_permissions(0o644),
                )?;
                zip.write_all(&data)?;
            }
            let bytes = zip.finish()?.into_inner();
            let pkg = package::validate(bytes)?;
            let mut f = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(out)?;
            f.write_all(&pkg.bytes)?;
            f.sync_all()?;
            println!(
                "{} {} 已打包，SHA-256 {}",
                pkg.manifest.id, pkg.manifest.version, pkg.sha
            );
        }
        _ => {
            return Err(
                "用法：yanmew-plugin validate <包> 或 pack <插件目录> <输出.yanmew-game>".into(),
            )
        }
    }
    Ok(())
}
fn main() {
    if let Err(e) = execute() {
        eprintln!("插件构建失败：{e}");
        std::process::exit(1)
    }
}
