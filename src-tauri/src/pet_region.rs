use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub fn physical_rects(rects: &[Rect], scale: f64) -> Result<Vec<[i32; 4]>, String> {
    if !scale.is_finite() || !(0.25..=8.0).contains(&scale) || rects.len() > 64 {
        return Err("交互区域数量或缩放比例无效".into());
    }
    rects
        .iter()
        .map(|r| {
            if [r.x, r.y, r.width, r.height]
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 100_000.0)
                || r.width <= 0.0
                || r.height <= 0.0
            {
                return Err("交互区域坐标或尺寸无效".into());
            }
            Ok([
                (r.x * scale).floor() as i32,
                (r.y * scale).floor() as i32,
                ((r.x + r.width) * scale).ceil() as i32,
                ((r.y + r.height) * scale).ceil() as i32,
            ])
        })
        .collect()
}

#[cfg(windows)]
pub fn apply(window: &tauri::WebviewWindow, rects: Option<&[[i32; 4]]>) -> Result<(), String> {
    use windows::Win32::{
        Foundation::HWND,
        Graphics::Gdi::{CombineRgn, CreateRectRgn, DeleteObject, SetWindowRgn, HRGN, RGN_OR},
    };
    struct Region(HRGN);
    impl Drop for Region {
        fn drop(&mut self) {
            unsafe {
                let _ = DeleteObject(self.0.into());
            }
        }
    }
    fn create(r: [i32; 4]) -> Result<Region, String> {
        let handle = unsafe { CreateRectRgn(r[0], r[1], r[2], r[3]) };
        if handle.0.is_null() {
            Err("创建窗口交互区域失败".into())
        } else {
            Ok(Region(handle))
        }
    }
    let hwnd = HWND(
        window
            .hwnd()
            .map_err(|e| format!("获取宠窗句柄失败：{e}"))?
            .0,
    );
    let Some(rects) = rects else {
        return if unsafe { SetWindowRgn(hwnd, None, true) } == 0 {
            Err("重置宠窗交互区域失败".into())
        } else {
            Ok(())
        };
    };
    let union = create([0, 0, 0, 0])?;
    for rect in rects {
        let part = create(*rect)?;
        if unsafe { CombineRgn(Some(union.0), Some(union.0), Some(part.0), RGN_OR) }.0 == 0 {
            return Err("合并宠窗交互区域失败".into());
        }
    }
    if unsafe { SetWindowRgn(hwnd, Some(union.0), true) } == 0 {
        return Err("设置宠窗交互区域失败".into());
    }
    // 成功后区域归系统所有，不能再释放。
    std::mem::forget(union);
    Ok(())
}

#[cfg(not(windows))]
pub fn apply(_: &tauri::WebviewWindow, _: Option<&[[i32; 4]]>) -> Result<(), String> {
    Err("宠窗交互区域仅支持 Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rounds_outward_once_for_dpi() {
        let r = Rect {
            x: 1.1,
            y: -1.1,
            width: 10.0,
            height: 5.0,
        };
        assert_eq!(physical_rects(&[r], 1.5).unwrap(), vec![[1, -2, 17, 6]]);
    }
    #[test]
    fn rejects_invalid_resource_inputs() {
        let r = Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 5.0,
        };
        for scale in [f64::NAN, f64::INFINITY, 0.0, 9.0] {
            assert!(physical_rects(&[r], scale).is_err());
        }
        assert!(physical_rects(&[r; 65], 1.0).is_err());
        for bad in [
            Rect { x: f64::NAN, ..r },
            Rect { width: -1.0, ..r },
            Rect { y: 100_001.0, ..r },
        ] {
            assert!(physical_rects(&[bad], 1.0).is_err());
        }
        assert!(physical_rects(&[], 1.0).unwrap().is_empty());
    }
}
