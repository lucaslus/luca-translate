//! Commands for compositor-owned shortcuts; dispatch only after the UI is listening.
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Action {
    Show,
    Hide,
    Toggle,
    Selection,
    Screenshot,
    Ocr,
}
fn parse(args: &[String]) -> Result<Action, &'static str> {
    match args.get(1).map(String::as_str) {
        None | Some("--show") => Ok(Action::Show),
        Some("--hide") => Ok(Action::Hide),
        Some("--toggle") => Ok(Action::Toggle),
        Some("--selection") => Ok(Action::Selection),
        Some("--screenshot") => Ok(Action::Screenshot),
        Some("--ocr") => Ok(Action::Ocr),
        _ => Err("Usage: lucas-translate [--show|--hide|--toggle|--selection|--screenshot|--ocr]"),
    }
    .and_then(|action| {
        if args.len() <= 2 {
            Ok(action)
        } else {
            Err("Only one action is allowed")
        }
    })
}
#[derive(Default)]
struct State {
    ready: bool,
    pending: Option<Action>,
}
fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(Default::default)
}
pub fn hyprland() -> bool {
    cfg!(target_os = "linux") && std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some()
}
fn blur_policy(debug: bool, compositor_managed: bool) -> bool {
    !debug && !compositor_managed
}
pub fn hide_on_blur() -> bool {
    blur_policy(cfg!(debug_assertions), hyprland())
}
pub fn initialize() {
    let args: Vec<_> = std::env::args().collect();
    let action = parse(&args).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    // A first-launch toggle always opens, even though Tauri initially creates a visible window.
    state().lock().unwrap().pending = Some(if action == Action::Toggle {
        Action::Show
    } else {
        action
    });
}
pub fn receive(app: &AppHandle, args: &[String]) {
    let Ok(action) = parse(args) else { return };
    let mut state = state().lock().unwrap();
    if !state.ready {
        if matches!(action, Action::Show | Action::Hide | Action::Toggle) {
            state.pending = None;
            drop(state);
            // A hidden webview may defer initialization; showing must never wait for it.
            schedule(
                app,
                if action == Action::Toggle {
                    Action::Show
                } else {
                    action
                },
            );
        } else {
            state.pending = Some(action);
        }
        return;
    }
    drop(state);
    schedule(app, action);
}
#[tauri::command]
pub fn desktop_ready(app: AppHandle, window: WebviewWindow) -> Result<bool, String> {
    crate::allow_window(&window, &["main"])?;
    let mut state = state().lock().map_err(|_| "桌面命令状态不可用")?;
    state.ready = true;
    let action = state.pending.take();
    drop(state);
    if let Some(action) = action {
        schedule(&app, action);
    }
    Ok(hyprland())
}
fn schedule(app: &AppHandle, action: Action) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || dispatch(&handle, action));
}

/// Run the packaged helper as the desktop user, never from pacman's root hooks.
#[tauri::command]
pub async fn setup_desktop(window: WebviewWindow) -> Result<(), String> {
    crate::allow_window(&window, &["main"])?;
    #[cfg(target_os = "linux")]
    if hyprland() {
        tauri::async_runtime::spawn_blocking(|| {
            let helper = std::path::Path::new("/usr/bin/lucas-translate-setup-omarchy");
            if !helper.is_file() {
                return Ok(()); // Source builds and non-Arch packages have no installer.
            }
            let output = crate::platform::linux::bounded_output(
                std::process::Command::new(helper).arg("--auto"),
                std::time::Duration::from_secs(15),
            )?;
            if output.status.success() {
                Ok(())
            } else {
                Err(format!(
                    "Omarchy 快捷键未自动启用：{}。可运行 lucas-translate-setup-omarchy 重试。",
                    String::from_utf8_lossy(&output.stderr).trim()
                ))
            }
        })
        .await
        .map_err(|e| e.to_string())??;
    }
    Ok(())
}
fn dispatch(app: &AppHandle, action: Action) {
    match action {
        Action::Hide => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.hide();
            }
        }
        Action::Toggle
            if app.get_webview_window("main").is_some_and(|w| {
                w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false)
            }) =>
        {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.hide();
            }
        }
        Action::Show | Action::Toggle => {
            crate::show_main(app);
            let _ = app.emit_to("main", "lucas://focus-input", ());
        }
        Action::Selection => crate::selection_translate(app),
        Action::Screenshot => crate::start_region_capture(app, false),
        Action::Ocr => crate::start_region_capture(app, true),
    }
}

// Wayland does not guarantee that GTK set_focus can activate a surface.
// Resolve the mapped main window, then ask the compositor to focus that exact address.
#[cfg(target_os = "linux")]
pub fn focus_main(app: &AppHandle) {
    if !hyprland() {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        use crate::platform::linux::bounded_output;
        use std::{process::Command, time::Duration};
        for _ in 0..10 {
            if !app
                .get_webview_window("main")
                .is_some_and(|w| w.is_visible().unwrap_or(false))
            {
                return;
            }
            let output = bounded_output(
                Command::new("hyprctl").args(["clients", "-j"]),
                Duration::from_millis(300),
            );
            if let Ok(output) = output {
                if let Ok(clients) =
                    serde_json::from_slice::<Vec<serde_json::Value>>(&output.stdout)
                {
                    let address = clients
                        .iter()
                        .find(|c| {
                            c["pid"].as_u64() == Some(std::process::id() as u64)
                                && c["title"].as_str() == Some("Lucas Translate")
                                && c["mapped"].as_bool() == Some(true)
                        })
                        .and_then(|c| c["address"].as_str());
                    if let Some(address) = address.filter(|a| {
                        a.strip_prefix("0x").is_some_and(|hex| {
                            !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit())
                        })
                    }) {
                        let expression =
                            format!("hl.dsp.focus({{window = \"address:{address}\"}})");
                        let _ = bounded_output(
                            Command::new("hyprctl").args(["dispatch", &expression]),
                            Duration::from_millis(300),
                        );
                        return;
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    });
}

#[cfg(target_os = "linux")]
pub fn capture(app: &AppHandle, silent: bool) {
    use crate::platform::linux::bounded_output;
    use std::{process::Command, sync::atomic::Ordering, time::Duration};
    if crate::OCR_RUNNING.swap(true, Ordering::AcqRel) {
        return;
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
    let app = app.clone();
    std::thread::spawn(move || {
        struct Guard;
        impl Drop for Guard {
            fn drop(&mut self) {
                crate::OCR_RUNNING.store(false, Ordering::Release);
            }
        }
        let _guard = Guard;
        let result = (|| -> Result<Option<String>, String> {
            let region = bounded_output(Command::new("slurp").arg("-d"), Duration::from_secs(120))?;
            if !region.status.success() {
                return Ok(None);
            } // Esc/right click: cancel.
            let region = String::from_utf8(region.stdout).map_err(|_| "无效截图区域")?;
            if region.trim().is_empty() {
                return Ok(None);
            }
            let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
            let png = dir.path().join("capture.png");
            let shot = bounded_output(
                Command::new("grim").arg("-g").arg(region.trim()).arg(&png),
                Duration::from_secs(10),
            )?;
            if !shot.status.success() {
                return Err("Wayland 截图失败，请检查 grim 和 slurp".into());
            }
            if std::fs::metadata(&png).map_err(|e| e.to_string())?.len() > 32 * 1024 * 1024 {
                return Err("截图过大，请缩小区域".into());
            }
            let bytes = std::fs::read(&png).map_err(|e| e.to_string())?;
            let lines = crate::platform::ocr_png(&bytes)?;
            let text = lucas_core::paragraph::merge_ocr_lines(&lines).join("\n");
            if text.trim().is_empty() {
                return Err("未发现文字，请重新选择区域".into());
            }
            if silent {
                let file = dir.path().join("text.txt");
                std::fs::write(&file, &text).map_err(|e| e.to_string())?;
                // bounded_output deliberately replaces stdin, so pass the private file to a child shell.
                // The path is a positional parameter, never interpolated into shell code.
                let mut command = Command::new("sh");
                command
                    .args(["-c", "wl-copy --type text/plain < \"$1\"", "lucas-copy"])
                    .arg(dir.path().join("text.txt"));
                let copied = bounded_output(&mut command, Duration::from_secs(3))?;
                if !copied.status.success() {
                    return Err("识别完成，但 Wayland 剪贴板写入失败".into());
                }
            }
            Ok(Some(text))
        })();
        match result {
            Ok(Some(_)) if silent => crate::silent_feedback(&app, "OCR 文字已复制"),
            Ok(Some(text)) => {
                crate::show_main(&app);
                let _ = app.emit_to("main", "lucas://ocr-result", text);
            }
            Ok(None) => {}
            Err(e) if silent => crate::silent_feedback(&app, &e),
            Err(e) => {
                crate::show_main(&app);
                let _ = app.emit_to("main", "lucas://error", e);
            }
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compositor_panels_survive_blur_during_activation() {
        assert!(!blur_policy(false, true));
        assert!(!blur_policy(true, true));
        assert!(blur_policy(false, false));
        assert!(!blur_policy(true, false));
    }
    #[test]
    fn accepts_only_known_desktop_actions() {
        for (arg, expected) in [
            ("--toggle", Action::Toggle),
            ("--show", Action::Show),
            ("--hide", Action::Hide),
            ("--selection", Action::Selection),
            ("--screenshot", Action::Screenshot),
            ("--ocr", Action::Ocr),
        ] {
            assert_eq!(parse(&["app".into(), arg.into()]), Ok(expected));
        }
        assert_eq!(parse(&["app".into()]), Ok(Action::Show));
        assert!(parse(&["app".into(), "--unknown".into()]).is_err());
        assert!(parse(&["app".into(), "--show".into(), "extra".into()]).is_err());
    }
}
