//! 旧内置默认图只有与其256像素版本逐字节一致时，才使用随包高清原图；自定义图不替换。
use super::*;
pub(crate) fn png(paths:&crate::platform::contracts::AppPaths,file:&str)->AppResult<Vec<u8>> {
 let data=crate::accessories::images::read(paths,file)?;
 let bytes=base64::engine::general_purpose::STANDARD.decode(data.split_once(',').ok_or_else(AppError::storage)?.1).map_err(|_|AppError::storage())?;
 let resources=paths.persona_resources()?;
 for key in ["gpt","claude","gemini","deepseek"] {
  let path=resources.join(key).join("base.png");
  let Ok(meta)=std::fs::metadata(&path) else{continue};if meta.len()>crate::accessories::images::MAX_BYTES as u64{continue;}
  let Ok(original)=std::fs::read(path) else{continue};
  if normalize(&original).is_ok_and(|small|small==bytes){return normalize_size(&original,1024);}
 }
 Ok(bytes)
}

#[cfg(test)]mod tests {
 use super::*;
 #[test]fn old_builtin_uses_high_resolution_but_custom_reference_is_not_replaced(){
  let root=std::env::temp_dir().join(format!("yanmew-reference-{}-{}",std::process::id(),chrono::Utc::now().timestamp_nanos_opt().unwrap()));
  std::fs::create_dir_all(&root).unwrap();let paths=crate::platform::contracts::AppPaths::portable(root.clone());
  let original=std::fs::read(paths.persona_resources().unwrap().join("gpt/base.png")).unwrap();
  let small=normalize(&original).unwrap();let file=crate::accessories::images::publish(&paths,&small).unwrap();
  let resolved=png(&paths,&file).unwrap();assert_eq!(image::load_from_memory(&resolved).unwrap().width(),1024);
  let mut pixels=image::RgbaImage::new(64,64);pixels.put_pixel(20,20,image::Rgba([7,90,200,255]));let mut cursor=std::io::Cursor::new(Vec::new());image::DynamicImage::ImageRgba8(pixels).write_to(&mut cursor,image::ImageFormat::Png).unwrap();let custom=normalize(cursor.get_ref()).unwrap();let file=crate::accessories::images::publish(&paths,&custom).unwrap();
  assert_eq!(png(&paths,&file).unwrap(),custom);
  // 本测试只清理自己在临时目录新建的独立资料。
  assert!(root.starts_with(std::env::temp_dir()));std::fs::remove_dir_all(root).unwrap();
 }
}
