//! Only trusted release metadata can select an installer; renderer supplies a version, never a URL.
use std::{
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, WebviewWindow};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio_util::sync::CancellationToken;
fn pending() -> &'static tokio::sync::Mutex<Option<Update>> {
    static STATE: OnceLock<tokio::sync::Mutex<Option<Update>>> = OnceLock::new();
    STATE.get_or_init(Default::default)
}
fn cancellation() -> &'static Mutex<Option<CancellationToken>> {
    static TOKEN: OnceLock<Mutex<Option<CancellationToken>>> = OnceLock::new();
    TOKEN.get_or_init(Default::default)
}
fn message(error: tauri_plugin_updater::Error) -> String {
    use tauri_plugin_updater::Error::*;
    match error {
        ReleaseNotFound => "尚无可用的更新信息，或更新源暂时无法访问；请稍后检查",
        TargetNotFound(_) | TargetsNotFound(_) => "此版本尚未提供当前系统的更新包",
        Minisign(_) | Base64(_) | SignatureUtf8(_) => {
            "更新包签名校验失败，已停止安装；当前版本未更改"
        }
        Io(_) => "无法写入应用目录，请检查权限和磁盘空间，或手动安装",
        _ => "更新未能完成，请检查网络后重试，或从 GitHub Releases 手动下载",
    }
    .into()
}
#[tauri::command]
pub async fn check_update(
    app: AppHandle,
    window: WebviewWindow,
) -> Result<serde_json::Value, String> {
    crate::allow_window(&window, &["settings"])?;
    let mut state = pending().try_lock().map_err(|_| "更新操作正在进行")?;
    *state = None;
    let update = app
        .updater_builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(message)?
        .check()
        .await
        .map_err(message)?;
    let installable = !cfg!(debug_assertions)
        && (!cfg!(target_os = "linux") || std::env::var_os("APPIMAGE").is_some());
    let view = serde_json::json!({"current":app.package_info().version.to_string(),"version":update.as_ref().map(|u|&u.version),"installable":installable});
    *state = update;
    Ok(view)
}
#[tauri::command]
pub fn cancel_update(window: WebviewWindow) -> Result<(), String> {
    crate::allow_window(&window, &["settings"])?;
    if let Some(token) = cancellation()
        .lock()
        .map_err(|_| "更新状态不可用")?
        .as_ref()
    {
        token.cancel();
    }
    Ok(())
}
#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    window: WebviewWindow,
    version: String,
) -> Result<(), String> {
    crate::allow_window(&window, &["settings"])?;
    if cfg!(debug_assertions) {
        return Err("开发版本不覆盖安装，请使用发布安装包".into());
    }
    if cfg!(target_os = "linux") && std::env::var_os("APPIMAGE").is_none() {
        return Err("Linux 自动更新仅用于 AppImage；DEB / RPM 请手动更新".into());
    }
    let mut state = pending().try_lock().map_err(|_| "更新操作正在进行")?;
    let mut update = state
        .as_ref()
        .filter(|u| u.version == version)
        .cloned()
        .ok_or("更新信息已失效，请重新检查")?;
    if update.download_url.scheme() != "https" {
        return Err("更新下载地址不安全，已停止".into());
    }
    update.timeout = Some(Duration::from_secs(300));
    let token = CancellationToken::new();
    *cancellation().lock().map_err(|_| "更新状态不可用")? = Some(token.clone());
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            if let Ok(mut t) = cancellation().lock() {
                *t = None;
            }
        }
    }
    let _guard = Guard;
    let mut downloaded = 0u64;
    let mut emitted = Instant::now() - Duration::from_secs(1);
    let mut too_large = false;
    let data = tokio::select! {
        biased;
        _=token.cancelled()=>return Err("已取消下载，当前版本未更改".into()),
        result=update.download(|bytes,total| {
            downloaded=downloaded.saturating_add(bytes as u64);
            if downloaded>512*1024*1024 || total.is_some_and(|n|n>512*1024*1024) { too_large=true;token.cancel(); }
            if emitted.elapsed()>=Duration::from_millis(250) {
                let _=app.emit_to("settings","lucas://update-progress",serde_json::json!({"phase":"downloading","downloaded":downloaded,"total":total}));
                emitted=Instant::now();
            }
        },||{})=>result.map_err(message)?,
    };
    if too_large || token.is_cancelled() {
        return Err("下载已停止，当前版本未更改".into());
    }
    let _ = app.emit_to(
        "settings",
        "lucas://update-progress",
        serde_json::json!({"phase":"installing"}),
    );
    // Verification completes in download(), before any installed files are touched.
    *cancellation().lock().map_err(|_| "更新状态不可用")? = None;
    crate::storage(move || update.install(data).map_err(message)).await?;
    *state = None;
    app.request_restart();
    Ok(())
}
