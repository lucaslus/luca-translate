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
mod platform;

use lucas_core::services::{
bing_free::BingFree, deepl_free::DeepLFree, google_free::GoogleFree,
    openai_compat::OpenAiCompat, youdao_dict::YoudaoDict, TranslateService,
};
use lucas_core::QueryResult;
use serde::Deserialize;
use tauri::{
    menu::{Menu, MenuItem},
    path::BaseDirectory,
    tray::{TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Listener, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_global_shortcut::{Builder as ShortcutBuilder, Shortcut, ShortcutState};

/// 权限状态（设置页展示用）
#[derive(serde::Serialize)]
struct PermissionStatus {
    accessibility: bool,
    screen_capture: bool,
}

/// 构建服务链：按用户开关启用服务
fn build_services(ai: &config::AiConfig, toggles: &config::ServiceToggles) -> Vec<Box<dyn TranslateService>> {
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
    if ai.enabled && !ai.base_url.trim().is_empty() && !ai.model.trim().is_empty() {
        services.push(Box::new(OpenAiCompat::new(
            ai.base_url.clone(),
            ai.api_key.clone(),
            ai.model.clone(),
        )));
    }
    services
}

fn init_data_paths(app: &AppHandle) {
    let dir = app
        .path()
        .resolve("", BaseDirectory::AppData)
        .unwrap_or_else(|_| std::env::temp_dir().join("lucas-translate"));
    let _ = std::fs::create_dir_all(&dir);
    db::set_db_path(dir.join("lucas.db"));
    config::set_config_dir(dir);
}

// ---------------------------------------------------------------------------
// 划词捕获流程
// ---------------------------------------------------------------------------

fn selection_translate(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
    let app = app.clone();
    std::thread::spawn(move || {
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
                let _ = app.emit("lucas://translate-text", t);
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
    }
}

// ---------------------------------------------------------------------------
// 快捷键分发
// ---------------------------------------------------------------------------

fn handle_shortcut(app: &AppHandle, shortcut: &Shortcut) {
    // global-hotkey 的 Display 输出形如 "alt+KeyD"（alt 前缀 + Code 枚举名）
    let key = shortcut.to_string().to_lowercase();
    match key.as_str() {
        // 划词翻译：隐藏窗口 → 捕获选中文本 → 回到窗口并翻译
        "alt+keyd" | "option+d" | "alt+d" => selection_translate(app),
        // 输入翻译：唤起主窗口聚焦输入框
        "alt+keya" | "option+a" | "alt+a" => {
            show_main(app);
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.emit("lucas://focus-input", ());
            }
        }
        // 截图翻译 / 静默截图 OCR
        "alt+keys" | "option+s" | "alt+s" => start_region_capture(app, false),
        "alt+keyc" | "option+c" | "alt+c" => start_region_capture(app, true),
        _ => {
            let _ = app.emit("lucas://shortcut", key);
        }
    }
}

// ---------------------------------------------------------------------------
// 截图选区（OCR）
// ---------------------------------------------------------------------------

fn start_region_capture(app: &AppHandle, silent: bool) {
    if !platform::screen_capture_available() {
        let _ = app.emit("lucas://screen-permission", ());
        return;
    }
    // 重建 overlay 窗口（保证干净状态）
    if let Some(old) = app.get_webview_window("overlay") {
        let _ = old.close();
    }
    let monitor = match app.primary_monitor() {
        Ok(Some(m)) => m,
        _ => {
            let _ = app.emit("lucas://error", "无法获取显示器信息");
            return;
        }
    };
    let pos = monitor.position();
    let size = monitor.size();
    let win = WebviewWindowBuilder::new(
        app,
        "overlay",
        WebviewUrl::App("overlay.html".into()),
    )
    .title("")
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .focused(true)
    .position(pos.x as f64, pos.y as f64)
    .inner_size(size.width as f64, size.height as f64)
    .build();

    match win {
        Ok(w) => {
            // 告诉 overlay 工作模式（silent: 识别后不弹窗直接进剪贴板）
            let _ = w.emit("lucas://overlay-init", silent);
        }
        Err(e) => {
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
}

fn run_ocr(app: &AppHandle, payload: RegionPayload) {
    let app = app.clone();
    std::thread::spawn(move || {
        // 等 overlay 真正隐藏，避免截进选区框
        if let Some(w) = app.get_webview_window("overlay") {
            let _ = w.hide();
        }
        std::thread::sleep(std::time::Duration::from_millis(300));

        let region = platform::Region {
            x: payload.x,
            y: payload.y,
            w: payload.w,
            h: payload.h,
        };
        let result = platform::capture_screen_region(&region)
            .and_then(|png| platform::ocr_png(&png))
            .map(|lines| lucas_core::paragraph::merge_ocr_lines(&lines).join("\n"));

        match result {
            Ok(text) if text.is_empty() => {
                let _ = app.emit("lucas://error", "未识别到文本");
            }
            Ok(text) => {
                if payload.silent {
                    // 静默模式：直接进剪贴板
                    let line_count = text.lines().count();
                    match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text).map_err(|e| e.into())) {
                        Ok(_) => {
                            let _ = app.emit("lucas://ocr-copied", line_count);
                        }
                        Err(e) => {
                            let _ = app.emit("lucas://error", format!("写入剪贴板失败: {e}"));
                        }
                    }
                } else {
                    show_main(&app);
                    let _ = app.emit("lucas://ocr-result", text);
                }
            }
            Err(e) => {
                let _ = app.emit("lucas://error", e);
            }
        }

        // 清理 overlay
        if let Some(w) = app.get_webview_window("overlay") {
            let _ = w.close();
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
    WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
        .title("lucas-translate 偏好设置")
        .inner_size(640.0, 470.0)
        .min_inner_size(580.0, 420.0)
        .center()
        .resizable(true)
        // Codex 风格：侧边栏通顶，红绿灯叠加在侧边栏上，无原生标题条
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(cfg!(target_os = "macos"))
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 应用入口
// ---------------------------------------------------------------------------

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            ShortcutBuilder::new()
                .with_shortcuts(["option+d", "option+a", "option+s", "option+c"])
                .expect("注册全局快捷键失败")
                .with_handler(|app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        handle_shortcut(app, shortcut);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            translate,
            service_names,
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
            start_screenshot,
            get_routing,
            set_routing,
            get_services,
            set_services
        ])
        .on_window_event(|window, event| {
            // 关闭主窗口 = 隐藏到托盘，继续后台运行（托盘菜单可退出）
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
            // Bob 式行为：release 版失焦自动隐藏
            if let WindowEvent::Focused(false) = event {
                if window.label() == "main" && !cfg!(debug_assertions) {
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            init_data_paths(app.handle());

            // ---- 系统托盘 ----
            // 单色 template 图（圆环+两点），macOS 菜单栏深浅色自适应
            let tray_image = tauri::image::Image::from_bytes(include_bytes!(
                "../icons/tray.png"
            ))?
            .clone();
            let open = MenuItem::with_id(app, "open", "打开翻译窗口", true, None::<&str>)?;
            let shot = MenuItem::with_id(app, "shot", "截图 OCR", true, None::<&str>)?;
            let silent = MenuItem::with_id(app, "silent", "静默截图 OCR", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "偏好设置…", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &shot, &silent, &settings, &quit])?;
            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(tray_image)
                .icon_as_template(true)
                .tooltip("lucas-translate")
                .menu(&menu)
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
                .on_tray_icon_event(|tray, event| {
                    // 左键点击托盘图标 = 打开主窗口
                    if let TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main(tray.app_handle());
                    }
                })
                .build(app)?;

            // ---- 截图选区事件 ----
            let handle = app.handle().clone();
            app.listen("lucas://region-selected", move |event| {
                if let Ok(payload) = serde_json::from_str::<RegionPayload>(event.payload()) {
                    run_ocr(&handle, payload);
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

/// 多服务流式翻译：单词→词典卡(带音标)+其他服务翻译卡；句子→全部服务并发。
/// 每个服务完成立刻推送 `lucas://service-result`，全部完成推送 `lucas://translate-done`。
#[tauri::command]
async fn translate(
    app: AppHandle,
    text: String,
    from: String,
    to: String,
) -> Result<(), String> {
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut history_recorded = false;
        let trimmed = text.trim().to_string();
        if trimmed.is_empty() {
            let _ = app2.emit("lucas://error", "输入为空");
            let _ = app2.emit("lucas://translate-done", ());
            return;
        }

        let ai = config::load_ai_config();
        let routing = config::load_routing();
        let toggles = config::load_services();
        let services = build_services(&ai, &toggles);
        if services.is_empty() {
            let _ = app2.emit("lucas://error", "没有启用的翻译服务，请到偏好设置中开启");
            let _ = app2.emit("lucas://translate-done", ());
            return;
        }

        // 目标语言解析：显式指定优先；auto 时按用户配置的路由规则
        let resolve_target = |detected: &str| -> String {
            if to != "auto" {
                return to.clone();
            }
            for rule in &routing.rules {
                if rule.from == detected {
                    return rule.to.clone();
                }
            }
            routing.fallback.clone()
        };

        let is_word = lucas_core::lang::is_english_word_like(&trimmed);
        let detected_hint = if is_word {
            "en"
        } else if from == "auto" {
            lucas_core::lang::detect_source(&trimmed)
        } else {
            from.as_str()
        };
        let resolved_to = resolve_target(detected_hint);
        let _ = app2.emit(
            "lucas://lang-resolved",
            serde_json::json!({"from": detected_hint, "to": resolved_to}),
        );

        // 广播参与服务名单 → 前端渲染全部占位卡
        let names: Vec<&str> = services.iter().map(|s| s.name()).collect();
        let _ = app2.emit("lucas://services-start", serde_json::json!(names));

        let (tx, rx) = std::sync::mpsc::channel::<(String, Result<Vec<String>, lucas_core::ServiceError>)>();

        // 单词：先出词典卡（同步，很快）
        let mut overall_success = false;
        if is_word {
            match lucas_core::services::dict_route(&trimmed, &resolved_to) {
                Ok(mut r) => {
                    r.pinyin = lucas_core::pinyin::annotate(&r.paragraphs.join("\n"));
                    db::add_history(&r.text, &r.paragraphs.join("\n"), &r.service);
                    overall_success = true;
                    let _ = app2.emit("lucas://service-result", r);
                }
                Err(e) => {
                    let _ = app2.emit(
                        "lucas://service-result",
                        QueryResult {
                            text: trimmed.clone(),
                            detected_from: "en".into(),
                            detected_to: resolved_to.clone(),
                            paragraphs: vec![],
                            dict: None,
                            pinyin: None,
                            service: "YoudaoDict".into(),
                            error: Some(e.to_string()),
                        },
                    );
                }
            }
        }

        // 其余服务并发翻译（单词时跳过词典服务，避免重复）
        for svc in services {
            if is_word && svc.name() == "YoudaoDict" {
                continue;
            }
            let tx = tx.clone();
            let text2 = trimmed.clone();
            let from2 = from.clone();
            let to2 = resolved_to.clone();
            std::thread::spawn(move || {
                let name = svc.name().to_string();
                let result = svc.translate(&text2, &from2, &to2);
                let _ = tx.send((name, result));
            });
        }
        drop(tx);

        for (name, result) in rx {
            let mut qr = QueryResult {
                text: trimmed.clone(),
                detected_from: detected_hint.to_string(),
                detected_to: resolved_to.clone(),
                paragraphs: vec![],
                dict: None,
                pinyin: None,
                service: name.clone(),
                error: None,
            };
            match result {
                Ok(paras) => {
                    qr.paragraphs = paras;
                    qr.pinyin = lucas_core::pinyin::annotate(&qr.paragraphs.join("\n"));
                    overall_success = true;
                    if !history_recorded {
                        db::add_history(&qr.text, &qr.paragraphs.join("\n"), &name);
                        history_recorded = true;
                    }
                }
                Err(e) => {
                    qr.error = Some(e.to_string());
                }
            }
            let _ = app2.emit("lucas://service-result", qr);
        }

        if !overall_success {
            let _ = app2.emit("lucas://error", "所有翻译服务均失败，请检查网络后重试");
        }
        let _ = app2.emit("lucas://translate-done", ());
    });
    Ok(())
}

/// 列出可用的翻译服务（UI 展示用）
#[tauri::command]
fn service_names() -> Vec<&'static str> {
    lucas_core::services::default_translate_services()
        .iter()
        .map(|s| s.name())
        .collect()
}

// ---- 划词 ----

/// 重试划词捕获（前端引导卡片的"重试"按钮）
#[tauri::command]
fn capture_selection(app: AppHandle) {
    selection_translate(&app);
}

// ---- 历史记录 & 收藏夹 ----

#[tauri::command]
fn history_list(limit: Option<i64>) -> Vec<db::HistoryItem> {
    db::list_history(limit.unwrap_or(50))
}

#[tauri::command]
fn history_clear() {
    db::clear_history();
}

#[tauri::command]
fn favorite_add(text: String, result: String, service: String) {
    db::add_favorite(&text, &result, &service);
}

#[tauri::command]
fn favorites_list() -> Vec<db::HistoryItem> {
    db::list_favorites()
}

#[tauri::command]
fn favorite_remove(id: i64) {
    db::remove_favorite(id);
}

// ---- AI 服务配置 ----

#[tauri::command]
fn get_ai_config() -> config::AiConfig {
    config::load_ai_config()
}

#[tauri::command]
fn set_ai_config(cfg: config::AiConfig) -> Result<(), String> {
    config::save_ai_config(&cfg)
}

// ---- 权限 & 设置 ----

/// 检查系统权限状态
#[tauri::command]
fn permission_status() -> PermissionStatus {
    PermissionStatus {
        accessibility: platform::accessibility_available(),
        screen_capture: platform::screen_capture_available(),
    }
}

/// 打开对应的系统隐私设置面板
#[tauri::command]
fn open_permission_settings(kind: String) {
    let url = match kind.as_str() {
        "accessibility" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        "screen" => "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
        _ => return,
    };
    let _ = std::process::Command::new("open").arg(url).spawn();
}

/// 打开偏好设置窗口
#[tauri::command]
fn open_settings(app: AppHandle) -> Result<(), String> {
    open_settings_impl(&app)
}

/// 重新发起截图（权限引导卡片的"重试"）
#[tauri::command]
fn start_screenshot(app: AppHandle) {
    start_region_capture(&app, false);
}

/// 读取服务开关
#[tauri::command]
fn get_services() -> config::ServiceToggles {
    config::load_services()
}

/// 保存服务开关
#[tauri::command]
fn set_services(services: config::ServiceToggles) -> Result<(), String> {
    config::save_services(&services)
}

/// 读取默认翻译规则
#[tauri::command]
fn get_routing() -> config::RoutingConfig {
    config::load_routing()
}

/// 保存默认翻译规则
#[tauri::command]
fn set_routing(cfg: config::RoutingConfig) -> Result<(), String> {
    config::save_routing(&cfg)
}
