//! 本地透明检测及一次性抠图；不访问网络，不修改原文件。
use crate::{
    error::{AppError, AppResult},
    platform::contracts::AppPaths,
    state::Cancellation,
};
use image::{ImageDecoder, ImageFormat, RgbaImage};
use sha2::{Digest, Sha256};
use std::{
    io::{Cursor, Read},
    path::PathBuf,
    sync::Mutex,
};

static MODEL: Mutex<Option<(PathBuf, ort::session::Session)>> = Mutex::new(None);
fn failed(message: &str) -> AppError {
    AppError::new("BACKGROUND_REMOVAL_FAILED", message)
}
pub fn decode(bytes: &[u8]) -> AppResult<RgbaImage> {
    if bytes.is_empty() || bytes.len() > 16 * 1024 * 1024 {
        return Err(failed("图片为空或超过16MB"));
    }
    let format = image::guess_format(bytes).map_err(|_| failed("无法识别图片格式"))?;
    match format {
        ImageFormat::Png => {
            if image::codecs::png::PngDecoder::new(Cursor::new(bytes))
                .map_err(|_| failed("图片损坏"))?
                .is_apng()
                .map_err(|_| failed("图片损坏"))?
            {
                return Err(failed("不支持动画图片"));
            }
        }
        ImageFormat::WebP => {
            if image::codecs::webp::WebPDecoder::new(Cursor::new(bytes))
                .map_err(|_| failed("图片损坏"))?
                .has_animation()
            {
                return Err(failed("不支持动画图片"));
            }
        }
        ImageFormat::Jpeg => (),
        _ => return Err(failed("支持静态PNG、JPEG和WebP图片")),
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let decoder = reader
        .into_decoder()
        .map_err(|_| failed("图片尺寸过大或数据损坏"))?;
    if decoder.dimensions().0 == 0 || decoder.dimensions().1 == 0 {
        return Err(failed("图片尺寸无效"));
    }
    Ok(image::DynamicImage::from_decoder(decoder)
        .map_err(|_| failed("图片无法解码"))?
        .into_rgba8())
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transparency {
    pub width: u32,
    pub height: u32,
    pub transparent_pixels: u64,
    pub visible_pixels: u64,
    pub has_transparency: bool,
    pub transparent_border: bool,
}
pub fn inspect(image: &RgbaImage) -> Transparency {
    let transparent_pixels = image.pixels().filter(|p| p[3] == 0).count() as u64;
    let visible_pixels = image.pixels().filter(|p| p[3] > 0).count() as u64;
    let (w, h) = image.dimensions();
    let transparent_border = (0..w)
        .all(|x| image.get_pixel(x, 0)[3] == 0 && image.get_pixel(x, h - 1)[3] == 0)
        && (0..h).all(|y| image.get_pixel(0, y)[3] == 0 && image.get_pixel(w - 1, y)[3] == 0);
    Transparency {
        width: w,
        height: h,
        transparent_pixels,
        visible_pixels,
        has_transparency: transparent_pixels > 0 && visible_pixels > 0,
        transparent_border,
    }
}
fn png(image: RgbaImage) -> AppResult<Vec<u8>> {
    let mut data = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut data, ImageFormat::Png)
        .map_err(|_| failed("透明PNG编码失败"))?;
    Ok(data.into_inner())
}
fn root(paths: &AppPaths) -> PathBuf {
    if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/models/background-removal")
    } else {
        paths.resource_root.join("models/background-removal")
    }
}
fn load(paths: &AppPaths) -> AppResult<ort::session::Session> {
    let root = root(paths);
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("manifest.json"))
            .map_err(|_| failed("本地抠图模型缺失，请恢复程序资源"))?,
    )
    .map_err(|_| failed("抠图模型清单无效"))?;
    let mut file =
        std::fs::File::open(root.join("model.onnx")).map_err(|_| failed("本地抠图模型缺失"))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(|_| failed("模型读取失败"))?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    if manifest["sha256"].as_str() != Some(format!("{:x}", hash.finalize()).as_str()) {
        return Err(failed("抠图模型校验失败，请恢复程序资源"));
    }
    let dll = paths.embedding_resources()?.join("onnxruntime.dll");
    ort::init_from(dll.to_string_lossy())
        .with_name("yanmew-local")
        .commit()
        .map_err(|_| failed("本地推理运行库加载失败"))?;
    ort::session::Session::builder()
        .and_then(|s| s.with_intra_threads(2))
        .and_then(|s| s.with_parallel_execution(false))
        .and_then(|s| s.with_config_entry("session.intra_op.allow_spinning", "0"))
        .and_then(|s| s.commit_from_file(root.join("model.onnx")))
        .map_err(|_| failed("抠图模型加载失败"))
}
pub fn remove(paths: &AppPaths, bytes: &[u8], cancel: &Cancellation) -> AppResult<Vec<u8>> {
    cancel.check()?;
    let mut original = decode(bytes)?;
    if inspect(&original).has_transparency {
        return Ok(bytes.to_vec());
    }
    let rgb = image::DynamicImage::ImageRgba8(original.clone()).to_rgb8();
    let resized = image::imageops::resize(&rgb, 320, 320, image::imageops::FilterType::Lanczos3);
    let max = resized.as_raw().iter().copied().max().unwrap_or(1).max(1) as f32;
    let mut input = vec![0f32; 3 * 320 * 320];
    let means = [0.485, 0.456, 0.406];
    let std = [0.229, 0.224, 0.225];
    for (i, p) in resized.pixels().enumerate() {
        for c in 0..3 {
            input[c * 320 * 320 + i] = (p[c] as f32 / max - means[c]) / std[c];
        }
    }
    let mut cache = MODEL.lock().map_err(|_| failed("本地抠图暂不可用"))?;
    cancel.check()?;
    if cache.as_ref().is_none_or(|(path, _)| *path != root(paths)) {
        *cache = Some((root(paths), load(paths)?));
    }
    let session = &mut cache.as_mut().unwrap().1;
    let name = session.inputs[0].name.clone();
    let tensor = ort::value::Tensor::from_array(([1usize, 3, 320, 320], input.into_boxed_slice()))
        .map_err(|_| failed("抠图输入无效"))?;
    let output = session
        .run(ort::inputs![name.as_str()=>tensor])
        .map_err(|_| failed("本地抠图推理失败"))?;
    let (_, values) = output[0]
        .try_extract_tensor::<f32>()
        .map_err(|_| failed("抠图结果格式无效"))?;
    if values.len() != 320 * 320 || values.iter().any(|v| !v.is_finite()) {
        return Err(failed("抠图结果无效"));
    }
    let low = values.iter().copied().fold(f32::INFINITY, f32::min);
    let high = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    if high - low < 1e-6 {
        return Err(failed("未能识别前景主体，原图保留"));
    }
    let mask = image::GrayImage::from_raw(
        320,
        320,
        values
            .iter()
            .map(|v| (((v - low) / (high - low)) * 255.0).clamp(0.0, 255.0) as u8)
            .collect(),
    )
    .ok_or_else(|| failed("抠图结果尺寸无效"))?;
    drop(output);
    drop(cache);
    cancel.check()?;
    let mask = image::imageops::resize(
        &mask,
        original.width(),
        original.height(),
        image::imageops::FilterType::Lanczos3,
    );
    for (pixel, alpha) in original.pixels_mut().zip(mask.pixels()) {
        let a = if alpha[0] <= 3 {
            0
        } else if alpha[0] >= 252 {
            255
        } else {
            alpha[0]
        };
        pixel[3] = ((pixel[3] as u16 * a as u16) / 255) as u8;
    }
    let info = inspect(&original);
    if !info.has_transparency {
        return Err(failed("抠图未产生有效透明主体，原图保留"));
    }
    png(original)
}
/// 自动兜底仅用于必须透明的生成资产；用户上传仍沿用原验证和预览。
pub fn ensure(
    paths: &AppPaths,
    bytes: &[u8],
    cancel: &Cancellation,
    border: bool,
) -> AppResult<Vec<u8>> {
    cancel.check()?;
    let result = if inspect(&decode(bytes)?).has_transparency {
        bytes.to_vec()
    } else {
        remove(paths, bytes, cancel)?
    };
    let image = decode(&result)?;
    let info = inspect(&image);
    if border && !info.transparent_border {
        if image.width() > 4094 || image.height() > 4094 {
            return Err(failed("透明图片过大，无法保留安全边距"));
        }
        let mut padded = RgbaImage::new(image.width() + 2, image.height() + 2);
        image::imageops::overlay(&mut padded, &image, 1, 1);
        return png(padded);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transparent_input_bypasses_model_and_padding_keeps_subject() {
        let mut image = RgbaImage::new(4, 4);
        image.put_pixel(0, 0, image::Rgba([10, 20, 30, 255]));
        let bytes = png(image).unwrap();
        let paths = AppPaths::portable(std::env::temp_dir().join("unused-cutout-data"));
        assert_eq!(
            ensure(&paths, &bytes, &Cancellation::default(), false).unwrap(),
            bytes
        );
        let padded =
            decode(&ensure(&paths, &bytes, &Cancellation::default(), true).unwrap()).unwrap();
        assert!(inspect(&padded).transparent_border);
        assert_eq!(padded.get_pixel(1, 1).0, [10, 20, 30, 255]);
    }
    #[test]
    fn rgba_opaque_is_not_transparent_and_corrupt_input_rejected() {
        let image = RgbaImage::from_pixel(4, 4, image::Rgba([255, 255, 255, 255]));
        assert!(!inspect(&image).has_transparency);
        assert!(decode(b"not an image").is_err());
        assert!(!inspect(&RgbaImage::new(4, 4)).has_transparency);
    }
    #[test]
    #[ignore = "本地完整模型推理验收，按需单独运行"]
    fn local_model_removes_background() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
        let source = std::fs::read(root.join("resources/personas/deepseek/base.png")).unwrap();
        let rgba = decode(&source).unwrap();
        let mut opaque = RgbaImage::from_pixel(
            rgba.width(),
            rgba.height(),
            image::Rgba([255, 255, 255, 255]),
        );
        image::imageops::overlay(&mut opaque, &rgba, 0, 0);
        let input = png(opaque).unwrap();
        let paths = AppPaths::portable(root.join(".test-temp/cutout-data"));
        let output = remove(&paths, &input, &Cancellation::default()).unwrap();
        let info = inspect(&decode(&output).unwrap());
        assert!(info.has_transparency);
        assert!(info.visible_pixels > 100);
        std::fs::write(root.join(".test-temp/170-cutout.png"), output).unwrap();
    }
}
