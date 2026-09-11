use crate::{allow_window, config, hotkeys, storage};
use tauri::{AppHandle, Emitter, WebviewWindow};
#[tauri::command]
pub async fn get_preferences(window: WebviewWindow) -> Result<serde_json::Value, String> {
    allow_window(&window, &["main", "settings"])?;
    let preferences = storage(config::preferences).await?;
    Ok(serde_json::json!({"preferences":preferences,"warnings":hotkeys::warnings(),"desktop_managed":crate::desktop_control::hyprland()}))
}
#[tauri::command]
pub async fn set_preferences(
    app: AppHandle,
    window: WebviewWindow,
    preferences: config::Preferences,
    font_only: Option<bool>,
) -> Result<(), String> {
    allow_window(&window, &["settings"])?;
    let handle = app.clone();
    storage(move || {
        if font_only.unwrap_or(false) {
            // A font edit must neither rewrite shortcuts nor retry conflicted registrations.
            config::save_font_size(preferences.font_size)
        } else {
            hotkeys::save(&handle, preferences)
        }
    })
    .await?;
    let _ = app.emit("lucas://preferences-changed", ());
    Ok(())
}
#[tauri::command]
pub async fn get_official_config(window: WebviewWindow) -> Result<config::OfficialView, String> {
    allow_window(&window, &["settings"])?;
    storage(config::official_view).await
}
#[tauri::command]
pub async fn set_official_config(
    app: AppHandle,
    window: WebviewWindow,
    config: config::OfficialUpdate,
) -> Result<(), String> {
    allow_window(&window, &["settings"])?;
    storage(move || config::save_official(config)).await?;
    let _ = app.emit("lucas://config-changed", ());
    Ok(())
}
#[tauri::command]
pub async fn test_official_connection(window: WebviewWindow) -> Result<(), String> {
    allow_window(&window, &["settings"])?;
    // Single-flight and fixed usage endpoint: no user text or billable translation.
    static BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if BUSY.swap(true, std::sync::atomic::Ordering::AcqRel) {
        return Err("正在检测，请稍候".into());
    }
    storage(|| {
        struct Guard;
        impl Drop for Guard {
            fn drop(&mut self) {
                BUSY.store(false, std::sync::atomic::Ordering::Release);
            }
        }
        let _guard = Guard;
        config::official_service()?
            .test_connection()
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub fn quit_app(app: AppHandle, window: WebviewWindow) -> Result<(), String> {
    allow_window(&window, &["settings"])?;
    app.exit(0);
    Ok(())
}
