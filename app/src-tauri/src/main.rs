#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! lucas-translate 桌面应用（Tauri 2）。
//!
//! 快捷键默认对齐 Bob：
//! - ⌥D (Option+D) 划词翻译：捕获选中文本 → 主窗口翻译
//! - ⌥A (Option+A) 输入翻译：唤起主窗口聚焦输入框
//! - ⌥S (Option+S) 截图翻译：选区 → OCR → 翻译窗口
//! - ⌥C (Option+C) 静默截图 OCR：选区 → OCR → 直接进剪贴板

mod config;
mod db;
mod desktop_control;
mod diagnostics;
mod hotkeys;
mod ocr_diagnostics;
mod platform;
mod provider_guard;
mod result_cache;
mod settings_commands;
mod translation;
mod updates;

use lucas_core::services::{
    bing_free::BingFree, deepl_free::DeepLFree, google_free::GoogleFree,
    openai_compat::OpenAiCompat, youdao_dict::YoudaoDict, TranslateService,
};
use serde::Deserialize;
use tauri::{
    menu::{Menu, MenuItem},
    path::BaseDirectory,
    tray::TrayIconBuilder,
    AppHandle, Emitter, Listener, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_global_shortcut::{Builder as ShortcutBuilder, Shortcut, ShortcutState};

/// overlay 的初始化事件可能早于其前端脚本注册监听器，暂存当前截图模式，
/// 由 overlay-ready 事件再次可靠地下发。
static OVERLAY_SILENT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 防止按键连发或截图尚未清理时重入第二个选区窗口。
static CAPTURE_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static CAPTURE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static OCR_RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static OVERLAY_ACK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// 权限状态（设置页展示用）
#[derive(serde::Serialize)]
struct PermissionStatus {
    platform: &'static str,
    accessibility: bool,
    screen_capture: bool,
}

/// 构建服务链：按用户开关启用服务
fn build_services(
    ai: &config::AiConfig,
    toggles: &config::ServiceToggles,
) -> Vec<Box<dyn TranslateService>> {
    let mut services: Vec<Box<dyn TranslateService>> = Vec::new();
    if toggles.youdao {
        services.push(Box::new(YoudaoDict));
    }
    if toggles.deepl {
        services.push(Box::new(DeepLFree::new()));
    }
    if toggles.bing {
        services.push(Box::new(BingFree));
    }
    if toggles.google {
        services.push(Box::new(GoogleFree));
    }
    if toggles.deepl_api {
        match config::official_service() {
            Ok(service) => services.push(Box::new(service)),
            Err(error) => services.push(Box::new(UnavailableOfficial(error))),
        }
    }
    if ai.enabled {
        if let Some(error) = ai.error.clone().or_else(|| {
            (ai.base_url.trim().is_empty() || ai.model.trim().is_empty())
                .then(|| "请在设置中补全 AI 地址和模型".into())
        }) {
            services.push(Box::new(UnavailableAi(error)));
        } else {
            services.push(Box::new(OpenAiCompat::new(
                ai.base_url.clone(),
                ai.api_key.clone(),
                ai.model.clone(),
            )));
        }
    }
    services
}
struct UnavailableAi(String);
struct UnavailableOfficial(String);
impl TranslateService for UnavailableOfficial {
    fn name(&self) -> &'static str {
        "DeepLApi"
    }
    fn translate(
        &self,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<Vec<String>, lucas_core::services::ServiceError> {
        Err(lucas_core::services::ServiceError::Configuration(
            self.0.clone(),
        ))
    }
}
impl TranslateService for UnavailableAi {
    fn name(&self) -> &'static str {
        "AI"
    }
    fn translate(
        &self,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<Vec<String>, lucas_core::services::ServiceError> {
        Err(lucas_core::services::ServiceError::Configuration(
            self.0.clone(),
        ))
    }
}

fn init_data_paths(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let dir = app.path().resolve("", BaseDirectory::AppData)?;
    std::fs::create_dir_all(&dir)?;
    db::set_db_path(dir.join("lucas.db"));
    config::set_config_dir(dir);
    Ok(())
}

// ---------------------------------------------------------------------------
// 划词捕获流程
// ---------------------------------------------------------------------------

fn selection_translate(app: &AppHandle) {
    static SELECTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if SELECTING.swap(true, std::sync::atomic::Ordering::AcqRel) {
        return;
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
    let app = app.clone();
    std::thread::spawn(move || {
        struct SelectionGuard;
        impl Drop for SelectionGuard {
            fn drop(&mut self) {
                SELECTING.store(false, std::sync::atomic::Ordering::Release);
            }
        }
        let _guard = SelectionGuard;
        // 无权限：弹系统原生授权提示 + 前端引导卡片
        if !platform::accessibility_available() {
            platform::prompt_accessibility();
            show_main(&app);
            let _ = app.emit("lucas://capture-failed", "permission");
            return;
        }
        let text = platform::capture_selection_text();
        show_main(&app);
        match text {
            Some(t) => {
                let _ = app.emit_to("main", "lucas://translate-text", t);
            }
            None => {
                let _ = app.emit("lucas://capture-failed", "no-selection");
            }
        }
    });
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        #[cfg(target_os = "linux")]
        desktop_control::focus_main(app);
    }
}

// ---------------------------------------------------------------------------
// 快捷键分发
// ---------------------------------------------------------------------------

fn handle_shortcut(app: &AppHandle, shortcut: &Shortcut) {
    // global-hotkey 的 Display 输出形如 "alt+KeyD"（alt 前缀 + Code 枚举名）
    let key = hotkeys::action(shortcut);
    match key.as_str() {
        // 划词翻译：隐藏窗口 → 捕获选中文本 → 回到窗口并翻译
        "selection" => selection_translate(app),
        // 输入翻译：唤起主窗口聚焦输入框
        "input" => {
            show_main(app);
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.emit("lucas://focus-input", ());
            }
        }
        // 截图翻译 / 静默截图 OCR
        "screenshot" => start_region_capture(app, false),
        "ocr" => start_region_capture(app, true),
        _ => {
            let _ = app.emit("lucas://shortcut", key);
        }
    }
}

// ---------------------------------------------------------------------------
// 截图选区（OCR）
// ---------------------------------------------------------------------------

fn start_region_capture(app: &AppHandle, silent: bool) {
    #[cfg(target_os = "linux")]
    if desktop_control::hyprland() {
        desktop_control::capture(app, silent);
        return;
    }
    if OCR_RUNNING.load(std::sync::atomic::Ordering::Acquire) {
        show_main(app);
        let _ = app.emit(
            "lucas://error",
            "上次识别仍在结束，请稍候；若持续无响应，请重启应用",
        );
        return;
    }
    if CAPTURE_ACTIVE.swap(true, std::sync::atomic::Ordering::AcqRel) {
        return;
    }

    let preflight = platform::screen_capture_available();
    diagnostics::record(diagnostics::Record::new(
        diagnostics::Event::CapturePermission,
        "",
        "",
        preflight,
        0,
        None,
    ));
    let has_screen_access = if preflight {
        true
    } else {
        // 首次使用时请求一次，并在请求后重新读取真实授权状态。
        platform::request_screen_capture()
    };
    if !has_screen_access {
        // 仅当前进程确实未获授权时显示引导，不用历史成功状态代替检查。
        CAPTURE_ACTIVE.store(false, std::sync::atomic::Ordering::Release);
        show_main(app);
        let _ = app.emit("lucas://screen-permission", silent);
        return;
    }
    OVERLAY_SILENT.store(silent, std::sync::atomic::Ordering::Relaxed);
    let monitor = match app
        .cursor_position()
        .and_then(|p| app.monitor_from_point(p.x, p.y))
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten())
    {
        Some(m) => m,
        _ => {
            CAPTURE_ACTIVE.store(false, std::sync::atomic::Ordering::Release);
            let _ = app.emit("lucas://error", "无法获取显示器信息");
            return;
        }
    };
    let capture_id = CAPTURE_ID.fetch_add(1, std::sync::atomic::Ordering::AcqRel) + 1;
    let watchdog = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        if CAPTURE_ID.load(std::sync::atomic::Ordering::Acquire) == capture_id
            && CAPTURE_ACTIVE.load(std::sync::atomic::Ordering::Acquire)
            && OVERLAY_ACK.load(std::sync::atomic::Ordering::Acquire) != capture_id
        {
            cancel_ocr(watchdog.clone());
            show_main(&watchdog);
            let _ = watchdog.emit("lucas://error", "选区窗口加载失败，请重新截图");
        }
    });
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }
    // 复用隐藏的覆盖层，避免 close 尚未处理完就用同名窗口重建。
    if let Some(overlay) = app.get_webview_window("overlay") {
        let reset = overlay
            .set_position(*monitor.position())
            .and_then(|_| overlay.set_size(*monitor.size()))
            .and_then(|_| {
                overlay.emit(
                    "lucas://overlay-init",
                    serde_json::json!({"silent":silent,"capture_id":capture_id}),
                )
            });
        if let Err(e) = reset {
            CAPTURE_ACTIVE.store(false, std::sync::atomic::Ordering::Release);
            show_main(app);
            let _ = app.emit("lucas://error", format!("打开截图窗口失败: {e}"));
        }
        return;
    }
    let pos = monitor.position().to_logical::<f64>(monitor.scale_factor());
    let size = monitor.size().to_logical::<f64>(monitor.scale_factor());
    let win = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("overlay.html".into()))
        .title("")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false) // 前端注册监听器并初始化完成后再显示
        .focused(false)
        .position(pos.x, pos.y)
        .inner_size(size.width, size.height)
        .build();

    match win {
        Ok(_) => {} // 首次初始化由 overlay-ready 握手触发
        Err(e) => {
            CAPTURE_ACTIVE.store(false, std::sync::atomic::Ordering::Release);
            show_main(app);
            let _ = app.emit("lucas://error", format!("创建截图窗口失败: {e}"));
        }
    }
}

#[derive(Debug, Deserialize)]
struct RegionPayload {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    #[serde(default)]
    silent: bool,
    capture_id: u64,
}

#[tauri::command]
fn cancel_ocr(app: AppHandle) {
    CAPTURE_ID.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    CAPTURE_ACTIVE.store(false, std::sync::atomic::Ordering::Release);
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.hide();
    }
}
fn silent_feedback(app: &AppHandle, message: &str) {
    use tauri_plugin_notification::NotificationExt;
    // Native notification, never contains recognized text or steals focus.
    let _ = app
        .notification()
        .builder()
        .title("截图 OCR")
        .body(message)
        .show();
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_tooltip(Some(format!("Lucas Translate · {message}")));
    }
}
fn run_ocr(app: &AppHandle, payload: RegionPayload) {
    use std::sync::atomic::Ordering;
    if !CAPTURE_ACTIVE.load(Ordering::Acquire)
        || CAPTURE_ID.load(Ordering::Acquire) != payload.capture_id
        || payload.w < 10
        || payload.h < 10
        || payload.w > 32768
        || payload.h > 32768
        || OCR_RUNNING.swap(true, Ordering::AcqRel)
    {
        return;
    }
    let id = payload.capture_id;
    let app = app.clone();
    let watch = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(25)).await;
        if CAPTURE_ID.load(Ordering::Acquire) == id && OCR_RUNNING.load(Ordering::Acquire) {
            cancel_ocr(watch.clone());
            if payload.silent {
                silent_feedback(&watch, "识别超时，请重新尝试");
            } else {
                show_main(&watch);
                let _ = watch.emit(
                    "lucas://error",
                    "识别超时，已停止等待；若再次截图仍无响应，请重启应用",
                );
            }
        }
    });
    std::thread::spawn(move || {
        struct RunningGuard;
        impl Drop for RunningGuard {
            fn drop(&mut self) {
                OCR_RUNNING.store(false, Ordering::Release);
            }
        }
        let _guard = RunningGuard;
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || -> Result<Option<String>, String> {
                if let Some(w) = app.get_webview_window("overlay") {
                    w.hide().map_err(|_| "隐藏截图选区失败")?;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
                if CAPTURE_ID.load(Ordering::Acquire) != id {
                    return Ok(None);
                }
                let started = std::time::Instant::now();
                let captured = platform::capture_screen_region(&platform::Region {
                    x: payload.x,
                    y: payload.y,
                    w: payload.w,
                    h: payload.h,
                });
                diagnostics::record(diagnostics::Record::new(
                    diagnostics::Event::CaptureResult,
                    "",
                    "",
                    captured.is_ok(),
                    started.elapsed().as_millis() as u64,
                    None,
                ));
                let png = captured?;
                if CAPTURE_ID.load(Ordering::Acquire) != id {
                    return Ok(None);
                }
                if !payload.silent {
                    show_main(&app);
                    let _ = app.emit("lucas://ocr-started", ());
                }
                let started = std::time::Instant::now();
                let recognized = platform::ocr_png(&png)
                    .map(|lines| lucas_core::paragraph::merge_ocr_lines(&lines).join("\n"));
                diagnostics::record(diagnostics::Record::new(
                    diagnostics::Event::OcrResult,
                    "",
                    "",
                    recognized.is_ok(),
                    started.elapsed().as_millis() as u64,
                    None,
                ));
                let text = recognized?;
                Ok(Some(text))
            },
        ))
        .unwrap_or_else(|_| Err("识别服务异常，请重试".into()));
        if CAPTURE_ID.load(Ordering::Acquire) != id {
            return;
        }
        CAPTURE_ACTIVE.store(false, Ordering::Release);
        match outcome {
            Ok(Some(text)) if text.trim().is_empty() => {
                if payload.silent {
                    silent_feedback(&app, "未发现文字，请重新选择区域");
                } else {
                    let _ = app.emit("lucas://error", "未发现文字，请选择包含清晰文字的区域");
                }
            }
            Ok(Some(text)) if payload.silent => {
                let lines = text.lines().count();
                match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text)) {
                    Ok(()) => {
                        silent_feedback(&app, &format!("已复制 {lines} 行文字"));
                        let _ = app.emit("lucas://ocr-copied", lines);
                    }
                    Err(_) => silent_feedback(&app, "识别完成，但复制失败，请重试"),
                }
            }
            Ok(Some(text)) => {
                let _ = app.emit_to("main", "lucas://ocr-result", text);
            }
            Ok(None) => {}
            Err(e) => {
                if payload.silent {
                    silent_feedback(&app, &e);
                } else {
                    show_main(&app);
                    if e.starts_with("SCREEN_CAPTURE_DENIED:") {
                        let _ = app.emit("lucas://screen-permission", false);
                    } else {
                        let _ = app.emit("lucas://error", e);
                    }
                }
            }
        }
    });
}

// ---------------------------------------------------------------------------
// 偏好设置窗口
// ---------------------------------------------------------------------------

fn open_settings_impl(app: &AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(());
    }
    let builder =
        WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
            .title("Lucas Translate 偏好设置")
            .inner_size(640.0, 470.0)
            .min_inner_size(580.0, 420.0)
            .center()
            .resizable(true);
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    builder.build().map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 应用入口
// ---------------------------------------------------------------------------

fn main() {
    if let Some(result) = ocr_diagnostics::run_if_requested() {
        if let Err(error) = result {
            eprintln!("[ocr-diagnostics] {error}");
            std::process::exit(1);
        }
        return;
    }
    desktop_control::initialize();
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            desktop_control::receive(app, &args);
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::SIZE
                        | tauri_plugin_window_state::StateFlags::POSITION,
                )
                .with_denylist(&["overlay", "settings"])
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            ShortcutBuilder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        handle_shortcut(app, shortcut);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            desktop_control::desktop_ready,
            settings_commands::get_preferences,
            settings_commands::set_preferences,
            settings_commands::get_official_config,
            settings_commands::set_official_config,
            settings_commands::test_official_connection,
            updates::check_update,
            updates::install_update,
            updates::cancel_update,
            translate,
            service_catalog,
            cancel_translation,
            cancel_ocr,
            capture_selection,
            history_list,
            history_clear,
            favorite_add,
            favorites_list,
            favorite_remove,
            get_ai_config,
            set_ai_config,
            permission_status,
            open_permission_settings,
            open_settings,
            restart_app,
            start_screenshot,
            get_routing,
            set_routing,
            get_services,
            set_service,
            diagnostics_status,
            open_diagnostics
        ])
        .on_window_event(|window, event| {
            // 关闭主窗口 = 隐藏到托盘，继续后台运行（托盘菜单可退出）
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
            // Hyprland activation is asynchronous; blur can arrive while showing selection results.
            // Keep its panel open until an explicit Esc / toggle / close.
            if let WindowEvent::Focused(false) = event {
                if window.label() == "main" && desktop_control::hide_on_blur() {
                    let pinned = window.is_always_on_top().unwrap_or(false);
                    if !pinned {
                        let _ = window.hide();
                    }
                }
            }
        })
        .setup(|app| {
            diagnostics::init(app.path().app_log_dir().ok());
            init_data_paths(app.handle())?;
            hotkeys::initialize(app.handle());

            // ---- 系统托盘 ----
            // 单色 template 图（圆环+两点），macOS 菜单栏深浅色自适应
            let tray_image =
                tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?.clone();
            let open = MenuItem::with_id(app, "open", "打开翻译窗口", true, None::<&str>)?;
            let shot = MenuItem::with_id(app, "shot", "截图 OCR", true, None::<&str>)?;
            let silent = MenuItem::with_id(app, "silent", "静默截图 OCR", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "偏好设置…", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &shot, &silent, &settings, &quit])?;
            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(tray_image)
                .icon_as_template(true)
                .tooltip("Lucas Translate")
                .menu(&menu)
                // 托盘点击只打开原生菜单。不要同时绑定鼠标事件唤起窗口，
                // 否则菜单交互期间的 mouse-up 也会触发 show_main。
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_main(app),
                    "shot" => start_region_capture(app, false),
                    "silent" => start_region_capture(app, true),
                    "settings" => {
                        if let Err(e) = open_settings_impl(app) {
                            let _ = app.emit("lucas://error", e);
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // ---- 截图选区事件 ----
            let handle = app.handle().clone();
            let overlay_handle = app.handle().clone();
            app.listen("lucas://overlay-ready", move |_| {
                if let Some(overlay) = overlay_handle.get_webview_window("overlay") {
                    let silent = OVERLAY_SILENT.load(std::sync::atomic::Ordering::Relaxed);
                    let id = CAPTURE_ID.load(std::sync::atomic::Ordering::Acquire);
                    if CAPTURE_ACTIVE.load(std::sync::atomic::Ordering::Acquire) {
                        let _ = overlay.emit(
                            "lucas://overlay-init",
                            serde_json::json!({"silent":silent,"capture_id":id}),
                        );
                    }
                }
            });
            app.listen("lucas://overlay-visible", move |event| {
                if let Ok(id) = serde_json::from_str::<u64>(event.payload()) {
                    OVERLAY_ACK.store(id, std::sync::atomic::Ordering::Release);
                }
            });
            let cancel_handle = app.handle().clone();
            app.listen("lucas://overlay-cancelled", move |event| {
                if serde_json::from_str::<u64>(event.payload()).ok()
                    == Some(CAPTURE_ID.load(std::sync::atomic::Ordering::Acquire))
                {
                    cancel_ocr(cancel_handle.clone());
                }
            });
            app.listen("lucas://region-selected", move |event| {
                if let Ok(payload) = serde_json::from_str::<RegionPayload>(event.payload()) {
                    run_ocr(&handle, payload);
                } else {
                    CAPTURE_ACTIVE.store(false, std::sync::atomic::Ordering::Release);
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("lucas-translate 启动失败");
}

// ---------------------------------------------------------------------------
// 命令
// ---------------------------------------------------------------------------

#[tauri::command]
fn translate(
    app: AppHandle,
    window: tauri::WebviewWindow,
    request_id: String,
    text: String,
    from: String,
    to: String,
    service: Option<String>,
) -> Result<(), String> {
    allow_window(&window, &["main"])?;
    translation::start(app, request_id, text, from, to, service)
}
#[tauri::command]
fn cancel_translation(window: tauri::WebviewWindow, request_id: String) -> Result<(), String> {
    allow_window(&window, &["main"])?;
    translation::cancel(&request_id);
    Ok(())
}
#[tauri::command]
fn service_catalog() -> Vec<translation::ServiceInfo> {
    translation::catalog()
}

#[tauri::command]
fn diagnostics_status() -> diagnostics::Status {
    diagnostics::status()
}
#[tauri::command]
async fn open_diagnostics() -> Result<(), String> {
    let path = diagnostics::directory()?;
    storage(move || {
        #[cfg(target_os = "macos")]
        let command = "open";
        #[cfg(target_os = "windows")]
        let command = "explorer";
        #[cfg(target_os = "linux")]
        let command = "xdg-open";
        let status = std::process::Command::new(command)
            .arg(path)
            .status()
            .map_err(|_| "无法打开日志目录".to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("无法打开日志目录，请检查文件管理器".into())
        }
    })
    .await
}
fn allow_window(window: &tauri::WebviewWindow, allowed: &[&str]) -> Result<(), String> {
    if allowed.contains(&window.label()) {
        Ok(())
    } else {
        Err("当前窗口无权执行此操作".into())
    }
}
async fn storage<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|_| "后台操作异常，请重试".to_string())?
}

// ---- 划词 ----

/// 重试划词捕获（前端引导卡片的"重试"按钮）
#[tauri::command]
fn capture_selection(app: AppHandle) {
    selection_translate(&app);
}

// ---- 历史记录 & 收藏夹 ----

#[tauri::command]
async fn history_list(
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<db::HistoryItem>, String> {
    storage(move || db::list_history(limit.unwrap_or(100), offset.unwrap_or(0))).await
}
#[tauri::command]
async fn history_clear() -> Result<(), String> {
    storage(db::clear_history).await
}
#[tauri::command]
async fn favorite_add(text: String, result: String, service: String) -> Result<(), String> {
    storage(move || db::add_favorite(&text, &result, &service)).await
}
#[tauri::command]
async fn favorites_list(
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<db::HistoryItem>, String> {
    storage(move || db::list_favorites(limit.unwrap_or(100), offset.unwrap_or(0))).await
}
#[tauri::command]
async fn favorite_remove(id: i64) -> Result<(), String> {
    storage(move || db::remove_favorite(id)).await
}
#[tauri::command]
async fn get_ai_config(window: tauri::WebviewWindow) -> Result<config::AiView, String> {
    allow_window(&window, &["settings"])?;
    storage(config::load_ai_config).await
}
#[tauri::command]
async fn set_ai_config(
    app: AppHandle,
    window: tauri::WebviewWindow,
    cfg: config::AiUpdate,
) -> Result<(), String> {
    allow_window(&window, &["settings"])?;
    storage(move || config::save_ai_config(cfg)).await?;
    let _ = app.emit("lucas://config-changed", ());
    Ok(())
}

// ---- 权限 & 设置 ----

/// 检查系统权限状态
#[tauri::command]
fn permission_status() -> PermissionStatus {
    PermissionStatus {
        platform: std::env::consts::OS,
        accessibility: platform::accessibility_available(),
        screen_capture: platform::screen_capture_available(),
    }
}

/// 打开对应的系统隐私设置面板
#[tauri::command]
fn open_permission_settings(kind: String) -> Result<(), String> {
    if !matches!(kind.as_str(), "screen" | "accessibility") {
        return Err("未知权限类型".into());
    }
    #[cfg(target_os = "macos")]
    {
        let url = match kind.as_str() {
            "accessibility" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            }
            "screen" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
            }
            _ => unreachable!(),
        };
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|_| "无法打开系统设置，请手动前往隐私与安全性".to_string())?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("此平台不使用 macOS 权限设置；请检查系统屏幕共享/辅助功能设置及截图组件".into())
    }
}

/// 打开偏好设置窗口
#[tauri::command]
fn open_settings(app: AppHandle) -> Result<(), String> {
    open_settings_impl(&app)
}

/// 用户按系统提示授权后，可彻底重启当前应用。
#[tauri::command]
fn restart_app(app: AppHandle) {
    app.request_restart();
}

/// 发起截图。
#[tauri::command]
fn start_screenshot(app: AppHandle, silent: Option<bool>) {
    start_region_capture(&app, silent.unwrap_or(false));
}

#[tauri::command]
async fn get_services() -> Result<config::ServiceToggles, String> {
    storage(config::load_services).await
}
#[tauri::command]
async fn set_service(
    app: AppHandle,
    id: String,
    enabled: bool,
) -> Result<config::ServiceToggles, String> {
    let result = storage(move || config::set_service(&id, enabled)).await?;
    let _ = app.emit("lucas://config-changed", ());
    Ok(result)
}
#[tauri::command]
async fn get_routing() -> Result<config::RoutingConfig, String> {
    storage(config::load_routing).await
}
#[tauri::command]
async fn set_routing(app: AppHandle, cfg: config::RoutingConfig) -> Result<(), String> {
    storage(move || config::save_routing(cfg)).await?;
    let _ = app.emit("lucas://config-changed", ());
    Ok(())
}
