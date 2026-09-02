//! Linux 实现：
//! - 划词：优先读 X11 PRIMARY selection（多数应用选中即写入），
//!   失败则 xdotool 模拟 Ctrl+C 后读 CLIPBOARD
//! - 截图：screenshots crate（X11；Wayland 视桌面环境支持程度）
//! - OCR：tesseract CLI（需用户安装：apt install tesseract-ocr tesseract-ocr-chi-sim）

use std::path::Path;
use std::process::Command;

use super::Region;

// 引用平台截图实现（真实构建指向 screenshots crate，检查环境指向 stub）
use super::screenshots_impl as screenshots;

// ---------------------------------------------------------------------------
// 划词
// ---------------------------------------------------------------------------

fn xclip_read(selection: &str) -> Option<String> {
    let out = Command::new("xclip")
        .args(["-selection", selection, "-o"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

fn xdotool_copy() {
    let _ = Command::new("xdotool")
        .args(["key", "--clearmodifiers", "ctrl+c"])
        .output();
}

pub fn capture_selection_text() -> Option<String> {
    // 路径 1：PRIMARY selection（X11 下选中即可读，无需模拟按键）
    if let Some(text) = xclip_read("primary") {
        return Some(text);
    }
    // 路径 2：模拟 Ctrl+C 后读 CLIPBOARD
    xdotool_copy();
    std::thread::sleep(std::time::Duration::from_millis(250));
    xclip_read("clipboard")
}

pub fn accessibility_available() -> bool {
    true // Linux 无统一权限体系
}

pub fn screen_capture_available() -> bool {
    // tesseract / xclip 缺失时给出友好提示，但不阻断截图
    true
}

// ---------------------------------------------------------------------------
// 截图：screenshots crate
// ---------------------------------------------------------------------------

pub fn capture_screen_region(region: &Region) -> Result<Vec<u8>, String> {
    let screens = screenshots::Screen::all().map_err(|e| format!("枚举显示器失败: {e}"))?;
    let screen = screens
        .iter()
        .find(|s| region.x >= s.display_info.x && region.y >= s.display_info.y)
        .or_else(|| screens.first())
        .ok_or("无可用显示器")?;

    // capture_area 接收相对当前显示器的坐标
    let rel_x = (region.x - screen.display_info.x).max(0);
    let rel_y = (region.y - screen.display_info.y).max(0);
    let img = screen
        .capture_area(rel_x, rel_y, region.w.max(1) as u32, region.h.max(1) as u32)
        .map_err(|e| format!("截图失败: {e}（Wayland 下可能需要桌面环境支持）"))?;

    use image::ImageEncoder;
    let (width, height) = (img.width(), img.height());
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(std::io::Cursor::new(&mut out))
        .write_image(
            &img.into_raw(),
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("PNG 编码失败: {e}"))?;
    Ok(out)
}

// ---------------------------------------------------------------------------
// OCR：tesseract CLI
// ---------------------------------------------------------------------------

pub fn ocr_png(png: &[u8]) -> Result<Vec<String>, String> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let path = std::env::temp_dir().join(format!("lucas-ocr-{ts}.png"));
    std::fs::write(&path, png).map_err(|e| e.to_string())?;
    let result = ocr_file(&path);
    let _ = std::fs::remove_file(&path);
    result
}

fn ocr_file(path: &Path) -> Result<Vec<String>, String> {
    let out = Command::new("tesseract")
        .arg(path)
        .arg("stdout")
        .args(["-l", "eng+chi_sim"])
        .output()
        .map_err(|_| "未找到 tesseract，请安装：sudo apt install tesseract-ocr tesseract-ocr-chi-sim".to_string())?;

    if !out.status.success() {
        // eng+chi_sim 语言包可能不全，回退默认语言
        let out = Command::new("tesseract")
            .arg(path)
            .arg("stdout")
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!(
                "tesseract 失败: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        return Ok(split_lines(&out.stdout));
    }
    Ok(split_lines(&out.stdout))
}

fn split_lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}
