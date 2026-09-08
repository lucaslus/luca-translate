//! Register changes transactionally; an occupied shortcut never aborts startup.
use crate::config::{self, Preferences};
use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
};
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Modifiers, Shortcut};

#[derive(Default)]
struct Bindings {
    active: BTreeMap<u32, (Shortcut, String)>,
    warnings: Vec<String>,
}
fn state() -> &'static Mutex<Bindings> {
    static STATE: OnceLock<Mutex<Bindings>> = OnceLock::new();
    STATE.get_or_init(Default::default)
}
fn parse(p: &Preferences) -> Result<BTreeMap<u32, (Shortcut, String)>, String> {
    if !(11..=20).contains(&p.font_size) {
        return Err("字号范围为 11–20".into());
    }
    if p.shortcuts.len() != 4
        || ["input", "selection", "screenshot", "ocr"]
            .iter()
            .any(|k| !p.shortcuts.contains_key(*k))
    {
        return Err("快捷键配置不完整".into());
    }
    let mut result = BTreeMap::new();
    for (action, key) in &p.shortcuts {
        if key.len() > 80 {
            return Err("快捷键过长".into());
        }
        if key.trim().is_empty() {
            continue;
        }
        let shortcut: Shortcut = key
            .parse()
            .map_err(|_| format!("无法识别快捷键 {key}，示例：Alt+A"))?;
        if !shortcut
            .mods
            .intersects(Modifiers::ALT | Modifiers::CONTROL | Modifiers::SUPER)
        {
            return Err("快捷键至少包含 Alt、Ctrl 或 Super / Command".into());
        }
        if result
            .insert(shortcut.id(), (shortcut, action.clone()))
            .is_some()
        {
            return Err("两个操作不能使用相同的快捷键".into());
        }
    }
    Ok(result)
}
pub fn action(shortcut: &Shortcut) -> String {
    state()
        .try_lock()
        .ok()
        .and_then(|s| s.active.get(&shortcut.id()).map(|(_, a)| a.clone()))
        .unwrap_or_default()
}
pub fn warnings() -> Vec<String> {
    state()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .warnings
        .clone()
}
pub fn initialize(app: &AppHandle) {
    let mut state = state().lock().unwrap_or_else(|e| e.into_inner());
    match config::preferences().and_then(|p| parse(&p)) {
        Ok(bindings) => {
            for (id, (key, action)) in bindings {
                match app.global_shortcut().register(key) {
                    Ok(()) => {
                        state.active.insert(id, (key, action));
                    }
                    Err(_) => state
                        .warnings
                        .push(format!("{key} 注册失败，可能被占用；请修改快捷键")),
                }
            }
        }
        Err(e) => state.warnings.push(e),
    }
}
pub fn save(app: &AppHandle, p: Preferences) -> Result<(), String> {
    let next = parse(&p)?;
    let mut state = state().lock().map_err(|_| "快捷键设置不可用")?;
    let mut added = Vec::new();
    for (id, (key, _)) in &next {
        if !state.active.contains_key(id) {
            if app.global_shortcut().register(*key).is_err() {
                for key in added {
                    let _ = app.global_shortcut().unregister(key);
                }
                return Err(format!("{key} 注册失败，可能被其他应用占用；原设置未更改"));
            }
            added.push(*key);
        }
    }
    if let Err(e) = config::save_preferences(p) {
        for key in added {
            let _ = app.global_shortcut().unregister(key);
        }
        return Err(e);
    }
    for (id, (key, _)) in &state.active {
        if !next.contains_key(id) {
            let _ = app.global_shortcut().unregister(*key);
        }
    }
    state.active = next;
    state.warnings.clear();
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validation_rejects_duplicates_and_plain_keys() {
        let mut p = Preferences::default();
        assert_eq!(parse(&p).unwrap().len(), 4);
        p.shortcuts.insert("input".into(), "Alt+D".into());
        assert!(parse(&p).is_err());
        p.shortcuts.insert("input".into(), "A".into());
        assert!(parse(&p).is_err());
        p.shortcuts.insert("input".into(), "".into());
        assert_eq!(parse(&p).unwrap().len(), 3);
        p.font_size = 100;
        assert!(parse(&p).is_err());
    }
}
