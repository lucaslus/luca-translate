//! 平台抽象层：划词捕获 / 屏幕截取 / OCR。
//! 每个平台一个实现文件，编译期按 target_os 选择。

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

// Linux 下给平台模块提供 screenshots crate 的稳定引用名
#[cfg(target_os = "linux")]
pub use screenshots as screenshots_impl;

/// 捕获当前选中的文本（模拟复制键 + 读取剪贴板 + 恢复剪贴板）
pub fn capture_selection_text() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        macos::capture_selection_text()
    }
    #[cfg(not(target_os = "macos"))]
    {
        None // Windows: UIA/Ctrl+C; Linux: X11 primary — 后续实现
    }
}

/// 屏幕区域（物理屏幕坐标，单位：点）
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// 截取屏幕区域，返回 PNG 字节
pub fn capture_screen_region(region: &Region) -> Result<Vec<u8>, String> {
    #[cfg(target_os = "macos")]
    {
        macos::capture_screen_region(region)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = region;
        Err("该平台暂未实现截图".into())
    }
}

/// 对 PNG 图片执行 OCR，返回识别出的文本行
pub fn ocr_png(png: &[u8]) -> Result<Vec<String>, String> {
    #[cfg(target_os = "macos")]
    {
        macos::ocr_png(png)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = png;
        Err("该平台暂未实现 OCR".into())
    }
}

/// 检查屏幕录制权限（截图 OCR 需要）
pub fn screen_capture_available() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::screen_capture_available()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// 检查辅助功能权限（划词模拟按键需要）
pub fn accessibility_available() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::accessibility_available()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// 主动触发系统辅助功能授权提示
pub fn prompt_accessibility() {
    #[cfg(target_os = "macos")]
    {
        macos::prompt_accessibility();
    }
}
