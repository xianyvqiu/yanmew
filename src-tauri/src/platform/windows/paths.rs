use crate::{
    error::{AppError, AppResult},
    platform::contracts::AppPaths,
};

pub fn application_paths() -> AppResult<AppPaths> {
    let exe = std::env::current_exe().map_err(|_| AppError::storage())?;
    let parent = exe.parent().ok_or_else(AppError::storage)?;
    Ok(AppPaths {
        data_root: parent.join("data"),
    })
}
