use super::*;
use std::{collections::BTreeSet, fs::{self, OpenOptions}, io::{self, Write}, path::{PathBuf, Component}, time::{SystemTime, UNIX_EPOCH}};

pub const PENDING: &str=".yanmew-update-pending.json";
const BACKUPS:&str="YanMew-backups";
pub struct Staged { pub root:PathBuf, pub package:Package }
#[derive(Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Journal { directory:String, files:Vec<String>, originals:Vec<PackageFile> }

pub fn workspace()->Result<PathBuf> {
    let root=std::env::temp_dir().join(format!("YanMew-update-{}-{}",std::process::id(),nonce()));
    fs::create_dir(&root).map_err(|_|"无法创建更新工作目录")?;Ok(root)
}
pub fn nonce()->u128 {SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()}
pub fn safe_relative(value:&str)->bool {
    if value.is_empty()||value.contains('\\')||value.chars().any(|c|c.is_control()||"<>:\"|?*".contains(c)){return false;}
    value.split('/').all(|part|{
        let base=part.split('.').next().unwrap_or("").to_ascii_uppercase();
        !part.is_empty()&&part!="."&&part!=".."&&!part.ends_with(['.',' '])
            &&!matches!(base.as_str(),"CON"|"PRN"|"AUX"|"NUL")
            &&!(base.len()==4&&(base.starts_with("COM")||base.starts_with("LPT"))&&base.as_bytes()[3].is_ascii_digit())
    })
}
fn managed(value:&str)->bool {
    safe_relative(value)&&(value.starts_with("resources/")||value.starts_with("licenses/")
        ||matches!(value,"yanwenzi.exe"|"yanmew-update.exe"|"yanmew-package.json"|"LICENSE"|"THIRD_PARTY_NOTICES.md"|"使用说明.md"|"msvcp140.dll"|"msvcp140_1.dll"|"msvcp140_2.dll"|"msvcp140_atomic_wait.dll"|"msvcp140_codecvt_ids.dll"|"vcruntime140.dll"|"vcruntime140_1.dll"|"vcruntime140_threads.dll"|"concrt140.dll"|"vccorlib140.dll"|"onnxruntime.dll"))
}
pub fn no_links(path:&Path)->Result<()> {
    let mut cursor=PathBuf::new();
    for part in path.components(){
        if matches!(part,Component::ParentDir){return Err("不接受包含上级跳转的更新路径".into());}
        cursor.push(part);
        if matches!(part,Component::Prefix(_)){continue;}
        match fs::symlink_metadata(&cursor) {
            Ok(metadata)=>{
                #[cfg(windows)] {use std::os::windows::fs::MetadataExt;if metadata.file_attributes()&0x400!=0{return Err("更新路径包含链接或重解析点，请使用普通目录".into());}}
                if metadata.file_type().is_symlink(){return Err("更新路径包含符号链接".into());}
            },Err(e) if e.kind()==io::ErrorKind::NotFound=>{},Err(_)=>return Err("无法核对更新目录权限".into()),
        }
    } Ok(())
}
pub fn target(path:&Path)->Result<PathBuf> {
    if !path.is_absolute(){return Err("请选择颜喵所在的完整目录".into());}
    no_links(path)?;
    let root=path.canonicalize().map_err(|_|"颜喵目录不存在")?;
    if root.parent().is_none()||!root.join("resources").is_dir(){return Err("该目录不是完整颜喵便携/安装目录，请选择yanwenzi.exe所在文件夹".into());}
    if root.join(PENDING).exists(){no_links(&root.join(PENDING))?;return Ok(root);}
    no_links(&root.join("yanwenzi.exe"))?;no_links(&root.join("resources"))?;
    let mut file=File::open(root.join("yanwenzi.exe")).map_err(|_|"无法读取旧版颜喵")?;let mut magic=[0;2];
    file.read_exact(&mut magic).map_err(|_|"旧程序不完整")?;if &magic!=b"MZ"{return Err("旧程序不是Windows可执行文件".into());}
    Ok(root)
}
pub fn validate_package(package:&Package)->Result<()> {
    if package.schema_version!=1||package.product!="com.yanmew.pet"||!valid_build(&package.build_id)
        ||semver::Version::parse(&package.version).is_err()||package.files.is_empty()||package.files.len()>20000{return Err("便携包清单版本或产品标识无效".into());}
    let mut names=BTreeSet::new();let mut total=0u64;
    for file in &package.files {
        if !managed(&file.path)||file.path==PACKAGE_MANIFEST||!valid_hash(&file.sha256)||!names.insert(file.path.to_lowercase())||file.size>1024*1024*1024{return Err("包清单包含不受管路径、重名文件或无效校验".into());}
        total=total.checked_add(file.size).ok_or("包大小溢出")?;
    }
    if total>3*1024*1024*1024||!names.contains("yanwenzi.exe")||!names.contains(HELPER){return Err("更新包缺少主程序/助手或超过大小限制".into());}Ok(())
}
pub fn extract(archive:&Path,folder:&Path,expected:&ReleaseManifest)->Result<Staged> {
    if fs::metadata(archive).map_err(|_|"找不到下载包")?.len()!=expected.size||hash_file(archive)?!=expected.sha256.to_ascii_lowercase(){return Err("更新包校验失败".into());}
    let mut zip=zip::ZipArchive::new(File::open(archive).map_err(|_|"无法打开更新包")?).map_err(|_|"更新包不是有效ZIP")?;
    if zip.len()>20001{return Err("更新包文件过多".into());}
    let mut names=BTreeSet::new();let mut prefix=None;let mut manifest_index=None;
    for i in 0..zip.len(){let file=zip.by_index(i).map_err(|_|"ZIP目录损坏")?;let name=file.name().trim_end_matches('/');
        if !safe_relative(name)||!names.insert(name.to_lowercase())||file.unix_mode().is_some_and(|m|m&0o170000==0o120000){return Err("ZIP包含危险路径、链接或重复文件".into());}
        let first=name.split('/').next().ok_or("ZIP目录无效")?;
        if let Some(p)=&prefix{if p!=first{return Err("更新包必须只有一个程序根目录".into());}}else{prefix=Some(first.to_string());}
        if name==format!("{first}/{PACKAGE_MANIFEST}"){if file.size()>4*1024*1024{return Err("文件清单过大".into());}manifest_index=Some(i);}
    }
    let index=manifest_index.ok_or("更新包缺少程序清单")?;
    let package:Package=serde_json::from_reader(zip.by_index(index).map_err(|_|"无法读取清单")?).map_err(|_|"便携包清单格式无效")?;validate_package(&package)?;
    if package.version!=expected.version||package.build_id!=expected.build_id{return Err("程序清单与Release版本不一致".into());}
    no_links(folder)?;fs::create_dir(folder).map_err(|_|"无法创建解包目录，或目录已存在")?;
    let prefix=format!("{}/",prefix.unwrap());let listed:BTreeSet<_>=package.files.iter().map(|f|f.path.as_str()).chain(std::iter::once(PACKAGE_MANIFEST)).collect();
    for i in 0..zip.len(){let mut file=zip.by_index(i).map_err(|_|"更新包损坏")?;if file.is_dir(){continue;}
        let relative=file.name().strip_prefix(&prefix).ok_or("ZIP路径无效")?.to_string();
        if !listed.contains(relative.as_str()){return Err("ZIP含未登记文件或个人资料，拒绝更新".into());}
        let limit=if relative==PACKAGE_MANIFEST{4*1024*1024}else{package.files.iter().find(|f|f.path==relative).ok_or("文件未登记")?.size};
        if file.size()!=limit&&relative!=PACKAGE_MANIFEST{return Err("ZIP文件大小与清单不符".into());}
        let output=folder.join(&relative);if let Some(parent)=output.parent(){fs::create_dir_all(parent).map_err(|_|"解包目录创建失败")?;}
        let mut out=OpenOptions::new().create_new(true).write(true).open(output).map_err(|_|"无法写入解包文件")?;
        let copied=io::copy(&mut (&mut file).take(limit+1),&mut out).map_err(|_|"ZIP解包失败")?;
        if copied>limit{return Err("解包大小超过限制".into());}
    }
    verify_files(folder,&package)?;Ok(Staged{root:folder.into(),package})
}
pub fn verify_files(root:&Path,package:&Package)->Result<()> {
    for file in &package.files {let path=root.join(&file.path);no_links(&path)?;
        if fs::metadata(&path).map_err(|_|"更新包缺少清单文件")?.len()!=file.size||hash_file(&path)?!=file.sha256.to_ascii_lowercase(){return Err("解包文件校验失败".into());}
    }Ok(())
}
fn copy_file(from:&Path,to:&Path)->Result<()> {
    no_links(from)?;no_links(to)?;
    if let Some(parent)=to.parent(){fs::create_dir_all(parent).map_err(|_|"无法创建文件目录")?;}
    fs::copy(from,to).map_err(|_|"复制文件失败，请检查占用、权限及磁盘空间")?;
    if hash_file(from)?!=hash_file(to)?{return Err("复制后的文件校验失败".into());}Ok(())
}
pub fn copy_data(from:&Path,to:&Path)->Result<()> {
    copy_tree(from,to,true)
}
fn copy_tree(from:&Path,to:&Path,skip_lock:bool)->Result<()> {
    no_links(from)?;no_links(to)?;if !from.exists(){fs::create_dir_all(to).map_err(|_|"备份目录创建失败")?;return Ok(());}
    fs::create_dir_all(to).map_err(|_|"备份目录创建失败")?;
    for item in fs::read_dir(from).map_err(|_|"无法读取资料目录")?{
        let item=item.map_err(|_|"无法读取资料文件")?;if skip_lock&&item.file_name()=="instance.lock"{continue;}
        let source=item.path();no_links(&source)?;let dest=to.join(item.file_name());
        if item.file_type().map_err(|_|"无法读取资料类型")?.is_dir(){copy_tree(&source,&dest,false)?;}else{copy_file(&source,&dest)?;}
    }Ok(())
}
pub fn old_files(root:&Path)->Result<Vec<String>> {
    if root.join(PACKAGE_MANIFEST).exists(){
        no_links(&root.join(PACKAGE_MANIFEST))?;
        if fs::metadata(root.join(PACKAGE_MANIFEST)).map_err(|_|"无法读取旧清单")?.len()>4*1024*1024{return Err("旧程序清单过大".into());}
        let package:Package=serde_json::from_reader(File::open(root.join(PACKAGE_MANIFEST)).map_err(|_|"无法打开旧清单")?).map_err(|_|"旧程序清单无效，请保留原目录")?;validate_package(&package)?;
        return Ok(package.files.into_iter().map(|f|f.path).chain(std::iter::once(PACKAGE_MANIFEST.into())).collect());
    }
    fn scan(root:&Path,relative:&str,out:&mut Vec<String>)->Result<()> {
        let path=root.join(relative);no_links(&path)?;
        if path.is_dir(){for item in fs::read_dir(path).map_err(|_|"无法读取旧资源目录")?{let item=item.map_err(|_|"无法读取旧资源")?;scan(root,&format!("{relative}/{}",item.file_name().to_string_lossy()),out)?;}}
        else if path.is_file()&&managed(relative){out.push(relative.into());}Ok(())
    }
    let mut result=Vec::new();for item in fs::read_dir(root).map_err(|_|"无法读取旧程序")?{let item=item.map_err(|_|"无法读取旧程序文件")?;let name=item.file_name().to_string_lossy().into_owned();if name=="resources"||name=="licenses"||managed(&name){scan(root,&name,&mut result)?;}}
    Ok(result)
}
fn journal_path(root:&Path,journal:&Journal)->Result<PathBuf>{
    if !safe_relative(&journal.directory)||journal.directory.contains('/')||!journal.directory.starts_with("update-"){return Err("恢复目录无效".into());}
    let path=root.parent().ok_or("程序目录无上级")?.join(BACKUPS).join(&journal.directory);no_links(&path)?;Ok(path)
}
pub fn recover(root:&Path)->Result<bool>{
    let marker=root.join(PENDING);if !marker.exists(){return Ok(false);}no_links(&marker)?;
    if fs::metadata(&marker).map_err(|_|"恢复标记不可读")?.len()>4*1024*1024{return Err("恢复记录过大".into());}
    let journal:Journal=serde_json::from_reader(File::open(&marker).map_err(|_|"恢复记录不可读")?).map_err(|_|"更新恢复记录无效，请保留备份并联系维护者")?;
    let backup=journal_path(root,&journal)?;let original:BTreeSet<_>=journal.originals.iter().map(|f|f.path.as_str()).collect();
    for file in &journal.originals {if !managed(&file.path)||!valid_hash(&file.sha256)||hash_file(&backup.join("program").join(&file.path))?!=file.sha256{return Err("旧程序备份校验失败，未恢复或删除任何程序文件".into());}}
    if journal.files.iter().any(|f|!managed(f)){return Err("更新恢复包含非程序路径".into());}
    for file in &journal.files {let path=root.join(file);no_links(&path)?;if !original.contains(file.as_str())&&path.exists(){fs::remove_file(path).map_err(|_|"清理未完成程序文件失败")?;}}
    for file in &journal.originals {copy_file(&backup.join("program").join(&file.path),&root.join(&file.path))?;}
    fs::remove_file(marker).map_err(|_|"恢复成功但无法移除事务标记")?;Ok(true)
}

/// 调用者须在正常退出后持有data的实例锁；资料只备份，不用新包覆盖。
pub fn apply(stage:&Staged,root:&Path,data:&Path,progress:impl Fn(&str))->Result<PathBuf>{
    apply_inner(stage,root,data,progress,None)
}
fn apply_inner(stage:&Staged,root:&Path,data:&Path,progress:impl Fn(&str),fail_after:Option<usize>)->Result<PathBuf>{
    let root=target(root)?;no_links(data)?;
    let had_manifest=root.join(PACKAGE_MANIFEST).is_file();
    if root.join(PENDING).exists(){return Err("存在未完成更新，请先运行更新助手恢复".into());}
    validate_package(&stage.package)?;verify_files(&stage.root,&stage.package)?;
    if root.join(PACKAGE_MANIFEST).exists(){let old:Package=serde_json::from_reader(File::open(root.join(PACKAGE_MANIFEST)).map_err(|_|"无法读取当前版本")?).map_err(|_|"旧版本清单无效")?;
        if !newer(&stage.package.version,&stage.package.build_id,&old.version,&old.build_id)?{return Err("更新包不是更新版本，拒绝降级或重复覆盖".into());}}
    let mut files:BTreeSet<String>=old_files(&root)?.into_iter().chain(stage.package.files.iter().map(|f|f.path.clone())).collect();files.insert(PACKAGE_MANIFEST.into());
    for file in &files {no_links(&root.join(file))?;}
    let directory=format!("update-{}-{}",stage.package.build_id,nonce());let backup=root.parent().ok_or("程序目录无上级")?.join(BACKUPS).join(&directory);no_links(&backup)?;
    fs::create_dir_all(&backup).map_err(|_|"无法建立更新备份目录")?;
    progress("正在备份旧程序与完整资料…");let mut originals=Vec::new();
    for file in &files {let path=root.join(file);if path.exists(){copy_file(&path,&backup.join("program").join(file))?;originals.push(PackageFile{path:file.clone(),size:fs::metadata(&path).map_err(|_|"无法读取旧文件大小")?.len(),sha256:hash_file(&path)?});}}
    copy_data(data,&backup.join("data"))?;
    let journal=Journal{directory,files:files.iter().cloned().collect(),originals};
    let mut marker=OpenOptions::new().write(true).create_new(true).open(root.join(PENDING)).map_err(|_|"无法建立更新事务")?;
    marker.write_all(&serde_json::to_vec(&journal).map_err(|_|"无法记录更新事务")?).map_err(|_|"无法保存更新事务")?;marker.sync_all().map_err(|_|"事务保存失败")?;drop(marker);
    let result=(||{
        progress("资料备份完成，正在替换程序文件…");
        let next:BTreeSet<_>=stage.package.files.iter().map(|f|f.path.as_str()).chain(std::iter::once(PACKAGE_MANIFEST)).collect();
        for (index,file) in files.iter().enumerate(){
            if fail_after==Some(index){return Err("模拟替换中断".to_string());}
            if next.contains(file.as_str()){copy_file(&stage.root.join(file),&root.join(file))?;}
            else if had_manifest&&root.join(file).exists(){fs::remove_file(root.join(file)).map_err(|_|"旧程序文件仍被占用")?;}
        }
        verify_files(&root,&stage.package)?;fs::remove_file(root.join(PENDING)).map_err(|_|"无法完成更新事务")?;Ok(())
    })();
    if let Err(error)=result {return match recover(&root){Ok(_)=>Err(format!("{error}；已恢复旧程序，资料保持原样")),Err(recovery)=>Err(format!("{error}；恢复未完成：{recovery}。请保留YanMew-backups备份，再运行更新助手"))};}
    progress("更新完成，资料与更新前备份均已保留。");Ok(backup)
}

#[cfg(test)] mod tests {
    use super::*;
    fn fixture()->(PathBuf,PathBuf,Staged){
        let base=workspace().unwrap();let root=base.join("旧颜喵");fs::create_dir_all(root.join("resources")).unwrap();fs::create_dir_all(root.join("data/plugins/game")).unwrap();
        fs::write(root.join("yanwenzi.exe"),b"MZold").unwrap();fs::write(root.join("data/db.sqlite"),b"my chat").unwrap();fs::write(root.join("data/plugins/game/save.json"),b"save").unwrap();fs::write(root.join("我的文件.txt"),b"keep").unwrap();
        let expanded=base.join("expanded");fs::create_dir(&expanded).unwrap();fs::write(expanded.join("yanwenzi.exe"),b"MZnew").unwrap();fs::write(expanded.join(HELPER),b"MZhelper").unwrap();
        let package=Package{schema_version:1,product:"com.yanmew.pet".into(),version:"0.1.0".into(),build_id:"20261110-170000".into(),files:["yanwenzi.exe",HELPER].into_iter().map(|name|PackageFile{path:name.into(),size:fs::metadata(expanded.join(name)).unwrap().len(),sha256:hash_file(&expanded.join(name)).unwrap()}).collect()};
        fs::write(expanded.join(PACKAGE_MANIFEST),serde_json::to_vec(&package).unwrap()).unwrap();(base,root,Staged{root:expanded,package})
    }
    #[test] fn blocks_traversal_windows_aliases_data_and_case_collisions(){
        for bad in ["../data/a","data/a","C:/bad","resources/../data/a","resources/CON.txt","resources/a.","resources/a:b","resources\\a"]{assert!(!managed(bad),"{bad}");}
        let (base,_,mut stage)=fixture();let mut extra=stage.package.files[0].clone();extra.path="YANWENZI.EXE".into();stage.package.files.push(extra);assert!(validate_package(&stage.package).is_err());fs::remove_dir_all(base).unwrap();
    }
    #[test] fn updates_old_portable_preserves_complete_data_and_foreign_files(){
        let (base,root,stage)=fixture();let backup=apply(&stage,&root,&root.join("data"),|_|{}).unwrap();
        assert_eq!(fs::read(root.join("yanwenzi.exe")).unwrap(),b"MZnew");assert_eq!(fs::read(root.join("data/db.sqlite")).unwrap(),b"my chat");assert_eq!(fs::read(backup.join("data/plugins/game/save.json")).unwrap(),b"save");assert_eq!(fs::read(root.join("我的文件.txt")).unwrap(),b"keep");assert!(!root.join(PENDING).exists());assert!(apply(&stage,&root,&root.join("data"),|_|{}).is_err());fs::remove_dir_all(base).unwrap();
    }
    #[test] fn replacement_failure_restores_program_without_restoring_or_losing_data(){
        let (base,root,stage)=fixture();let error=apply_inner(&stage,&root,&root.join("data"),|_|{},Some(1)).unwrap_err();assert!(error.contains("已恢复旧程序"));assert_eq!(fs::read(root.join("yanwenzi.exe")).unwrap(),b"MZold");assert!(!root.join(HELPER).exists());assert_eq!(fs::read(root.join("data/db.sqlite")).unwrap(),b"my chat");assert!(!root.join(PENDING).exists());fs::remove_dir_all(base).unwrap();
    }
    #[test] fn rejects_corrupt_staging_before_touching_target(){
        let (base,root,stage)=fixture();fs::write(stage.root.join("yanwenzi.exe"),b"corrupt").unwrap();assert!(apply(&stage,&root,&root.join("data"),|_|{}).is_err());assert_eq!(fs::read(root.join("yanwenzi.exe")).unwrap(),b"MZold");assert!(!base.join(BACKUPS).exists());fs::remove_dir_all(base).unwrap();
    }
    #[test] fn legacy_update_keeps_untracked_resource_files_and_nested_lock_named_saves(){
        let (base,root,stage)=fixture();fs::write(root.join("resources/用户放的文件.txt"),b"keep").unwrap();fs::write(root.join("data/plugins/game/instance.lock"),b"actual save").unwrap();
        let backup=apply(&stage,&root,&root.join("data"),|_|{}).unwrap();assert_eq!(fs::read(root.join("resources/用户放的文件.txt")).unwrap(),b"keep");assert_eq!(fs::read(backup.join("data/plugins/game/instance.lock")).unwrap(),b"actual save");fs::remove_dir_all(base).unwrap();
    }
    #[test] fn recovers_persisted_interruption_and_rejects_corrupted_backup_before_mutation(){
        let (base,root,_)=fixture();let directory="update-test";let backup=base.join(BACKUPS).join(directory);fs::create_dir_all(backup.join("program")).unwrap();fs::write(backup.join("program/yanwenzi.exe"),b"MZold").unwrap();
        let journal=Journal{directory:directory.into(),files:vec!["yanwenzi.exe".into(),HELPER.into()],originals:vec![PackageFile{path:"yanwenzi.exe".into(),size:5,sha256:hash_file(&backup.join("program/yanwenzi.exe")).unwrap()}]};
        fs::write(root.join(PENDING),serde_json::to_vec(&journal).unwrap()).unwrap();fs::write(root.join("yanwenzi.exe"),b"").unwrap();fs::write(root.join(HELPER),b"partial").unwrap();
        assert!(target(&root).is_ok());assert!(recover(&root).unwrap());assert_eq!(fs::read(root.join("yanwenzi.exe")).unwrap(),b"MZold");assert!(!root.join(HELPER).exists());
        fs::write(root.join(PENDING),serde_json::to_vec(&journal).unwrap()).unwrap();fs::write(backup.join("program/yanwenzi.exe"),b"tampered").unwrap();assert!(recover(&root).is_err());assert_eq!(fs::read(root.join("yanwenzi.exe")).unwrap(),b"MZold");fs::remove_dir_all(base).unwrap();
    }
    fn archive(stage:&Staged,path:&Path,extra:Option<&str>,corrupt:bool)->ReleaseManifest{
        let mut zip=zip::ZipWriter::new(File::create(path).unwrap());let options=zip::write::SimpleFileOptions::default();
        for name in stage.package.files.iter().map(|f|f.path.as_str()).chain(std::iter::once(PACKAGE_MANIFEST)){
            zip.start_file(format!("YanMew/{name}"),options).unwrap();let bytes=if corrupt&&name=="yanwenzi.exe"{b"wrong".to_vec()}else{fs::read(stage.root.join(name)).unwrap()};zip.write_all(&bytes).unwrap();
        }
        if let Some(name)=extra{zip.start_file(name,options).unwrap();zip.write_all(b"unexpected").unwrap();}zip.finish().unwrap();
        ReleaseManifest{schema_version:1,version:stage.package.version.clone(),build_id:stage.package.build_id.clone(),asset_name:"portable.zip".into(),size:fs::metadata(path).unwrap().len(),sha256:hash_file(path).unwrap()}
    }
    #[test] fn extracts_verified_zip_but_rejects_personal_data_unlisted_files_and_bad_payload(){
        let (base,_,stage)=fixture();
        for (index,extra,corrupt,okay) in [(0,None,false,true),(1,Some("YanMew/data/private.db"),false,false),(2,Some("YanMew/resources/unlisted.txt"),false,false),(3,None,true,false),(4,Some("YanMew/../escape.txt"),false,false)]{
            let path=base.join(format!("{index}.zip"));let expected=archive(&stage,&path,extra,corrupt);let result=extract(&path,&base.join(format!("extract-{index}")),&expected);assert_eq!(result.is_ok(),okay,"case {index}");
        }assert!(!base.join("escape.txt").exists());fs::remove_dir_all(base).unwrap();
    }
    #[test] #[ignore="构建后显式验证完整发布包，只写新建隔离目录"]
    fn full_release_package_round_trip(){
        let release=PathBuf::from(std::env::var_os("YANMEW_TEST_RELEASE_DIR").expect("需指定待检查发布目录"));
        let expected:ReleaseManifest=serde_json::from_reader(File::open(release.join("yanmew-update.json")).unwrap()).unwrap();
        let (base,root,_)=fixture();let stage=extract(&release.join(&expected.asset_name),&base.join("full-expanded"),&expected).unwrap();assert!(!stage.root.join("data").exists());
        let backup=apply(&stage,&root,&root.join("data"),|_|{}).unwrap();verify_files(&root,&stage.package).unwrap();assert_eq!(fs::read(root.join("data/db.sqlite")).unwrap(),b"my chat");assert_eq!(fs::read(backup.join("data/plugins/game/save.json")).unwrap(),b"save");assert_eq!(fs::read(root.join("我的文件.txt")).unwrap(),b"keep");println!("正式包解包、逐文件校验、旧版接续及备份验证通过：{}个受管文件",stage.package.files.len());fs::remove_dir_all(base).unwrap();
    }
}
