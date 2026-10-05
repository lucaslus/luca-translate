//! Keep desktop shortcut storage and conflict checks in the same packaged helper.
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
pub struct DesktopShortcuts {
    pub shortcuts: BTreeMap<String, String>,
    pub conflicts: BTreeMap<String, String>,
}

#[tauri::command]
pub async fn set_shortcut_recording(
    window: tauri::WebviewWindow,
    active: bool,
    token: String,
) -> Result<(), String> {
    crate::allow_window(&window, &["settings"])?;
    let token = uuid::Uuid::parse_str(&token)
        .map_err(|_| "无效录入会话")?
        .to_string();
    #[cfg(target_os = "linux")]
    if crate::desktop_control::hyprland() {
        if active && !window.is_focused().unwrap_or(false) {
            return Err("请先聚焦设置窗口".into());
        }
        return crate::storage(move || {
            let expression = format!(
                "assert(lucas_translate_shortcut_recording and lucas_translate_shortcut_recording({active}, \"{token}\", {}), \"无法进入快捷键录入模式，请重新打开设置\")",
                std::process::id(),
            );
            let output = crate::platform::linux::bounded_output(
                std::process::Command::new("hyprctl").args(["eval", &expression]),
                std::time::Duration::from_secs(2),
            )?;
            if output.status.success() { Ok(()) } else {
                Err("无法屏蔽桌面快捷键，请重新打开设置后重试；也可以粘贴组合键文字".into())
            }
        }).await;
    }
    let _ = (active, token);
    Err("此桌面不支持保护录入，请输入或粘贴组合键文字".into())
}

pub fn run(
    operation: &str,
    shortcuts: Option<&BTreeMap<String, String>>,
) -> Result<DesktopShortcuts, String> {
    #[cfg(target_os = "linux")]
    {
        let mut command = std::process::Command::new("python3");
        command.args([
            "-c",
            include_str!("../../../packaging/omarchy/lucas-translate-setup-omarchy"),
            "--template-text",
            include_str!("../../../packaging/omarchy/lucas-translate.lua"),
            operation,
        ]);
        if let Some(shortcuts) = shortcuts {
            command.arg(serde_json::to_string(shortcuts).map_err(|e| e.to_string())?);
        }
        let output = crate::platform::linux::bounded_output(
            &mut command,
            std::time::Duration::from_secs(30),
        )?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        serde_json::from_slice(&output.stdout).map_err(|e| format!("无法读取桌面快捷键：{e}"))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (operation, shortcuts);
        Err("当前平台不使用 Hyprland 快捷键".into())
    }
}
