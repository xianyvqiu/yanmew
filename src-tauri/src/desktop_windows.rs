use crate::error::{AppError, AppResult};
use serde::Deserialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// 只接受已实现的主程序页面，不允许前端传 URL、权限或任意窗口标签。
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Page {
    Chat,
    Settings,
}
impl Page {
    fn specification(self) -> (&'static str, &'static str, f64, f64) {
        match self {
            Self::Chat => ("chat", "聊天 · 颜喵 YanMew", 800.0, 640.0),
            Self::Settings => ("settings", "设置 · 颜喵 YanMew", 960.0, 700.0),
        }
    }
}
pub fn is_application_window(label: &str) -> bool {
    matches!(label, "main" | "chat" | "settings")
}
fn window_error(_: tauri::Error) -> AppError {
    AppError::new("WINDOW_ERROR", "窗口打开失败，请稍后重试")
}

/// 主线程串行执行查找与创建，快速重复点击也不会同时创建两个实例。
pub async fn open(app: AppHandle, page: Page) -> AppResult<()> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let owner = app.clone();
    app.run_on_main_thread(move || {
        let result = (|| {
            let (label, title, width, height) = page.specification();
            // 工作区按 DPI 换算为逻辑尺寸，并为原生边框、标题栏留出空间。
            let (max_width, max_height) = owner
                .primary_monitor()
                .map_err(window_error)?
                .map(|monitor| {
                    (
                        (monitor.work_area().size.width as f64 / monitor.scale_factor() - 32.0)
                            .max(1.0),
                        (monitor.work_area().size.height as f64 / monitor.scale_factor() - 72.0)
                            .max(1.0),
                    )
                })
                .unwrap_or((width, height));
            let window = match owner.get_webview_window(label) {
                Some(window) => window,
                None => {
                    WebviewWindowBuilder::new(&owner, label, WebviewUrl::App("index.html".into()))
                        .title(title)
                        .inner_size(width.min(max_width), height.min(max_height))
                        .min_inner_size(360.0_f64.min(max_width), 420.0_f64.min(max_height))
                        .resizable(true)
                        .decorations(true)
                        .transparent(false)
                        .always_on_top(false)
                        .background_color(tauri::window::Color(242, 240, 235, 255))
                        .visible(false)
                        .center()
                        .build()
                        .map_err(window_error)?
                }
            };
            window.show().map_err(window_error)?;
            window.unminimize().map_err(window_error)?;
            window.set_focus().map_err(window_error)?;
            Ok(())
        })();
        let _ = sender.send(result);
    })
    .map_err(window_error)?;
    receiver
        .await
        .map_err(|_| AppError::new("WINDOW_ERROR", "窗口创建任务已结束，请重试"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_fixed_application_pages_are_authorized() {
        for page in [Page::Chat, Page::Settings] {
            assert!(is_application_window(page.specification().0));
        }
        for label in [
            "plugin",
            "plugin-chat",
            "chat?window=settings",
            "",
            "main-copy",
        ] {
            assert!(!is_application_window(label));
        }
        assert!(serde_json::from_str::<Page>("\"plugin\"").is_err());
    }
}
