//! Linux 实现：
//! - 划词：仅读 X11 PRIMARY selection；无法安全读取时不修改 CLIPBOARD
//! - 截图：screenshots crate（X11；Wayland 视桌面环境支持程度）
//! - OCR：tesseract CLI（需用户安装：apt install tesseract-ocr tesseract-ocr-chi-sim）

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use super::Region;

// 引用平台截图实现（真实构建指向 screenshots crate，检查环境指向 stub）
use super::screenshots_impl as screenshots;

// ---------------------------------------------------------------------------
// 划词
// ---------------------------------------------------------------------------

fn xclip_read(selection: &str) -> Option<String> {
    let out = bounded_output(
        Command::new("xclip").args(["-selection", selection, "-o"]),
        Duration::from_millis(500),
    )
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

pub fn capture_selection_text() -> Option<String> {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        let out = bounded_output(
            Command::new("wl-paste").args(["--primary", "--no-newline", "--type", "text"]),
            Duration::from_millis(800),
        )
        .ok()?;
        let text = String::from_utf8(out.stdout).ok()?;
        return (out.status.success() && !text.trim().is_empty()).then_some(text);
    }
    // 路径 1：PRIMARY selection（X11 下选中即可读，无需模拟按键）
    if let Some(text) = xclip_read("primary") {
        return Some(text);
    }
    // Do not synthesize Copy when PRIMARY is unavailable: it destroys arbitrary
    // clipboard formats and can mistake stale clipboard text for the selection.
    None
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
        .find(|s| {
            region.x >= s.display_info.x
                && region.y >= s.display_info.y
                && (region.x as i64) < s.display_info.x as i64 + s.display_info.width as i64
                && (region.y as i64) < s.display_info.y as i64 + s.display_info.height as i64
        })
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
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let path = dir.path().join("capture.png");
    std::fs::write(&path, png).map_err(|e| e.to_string())?;
    ocr_file(&path)
}

fn ocr_file(path: &Path) -> Result<Vec<String>, String> {
    let out = bounded_output(
        Command::new("tesseract")
            .arg(path)
            .arg("stdout")
            .args(["-l", "eng+chi_sim"]),
        Duration::from_secs(8),
    )
    .map_err(|e| {
        format!("无法运行 OCR：{e}。请确认已安装 tesseract-ocr 和 tesseract-ocr-chi-sim")
    })?;

    if !out.status.success() {
        // eng+chi_sim 语言包可能不全，回退默认语言
        let out = bounded_output(
            Command::new("tesseract").arg(path).arg("stdout"),
            Duration::from_secs(8),
        )?;
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

/// Files avoid a full pipe deadlocking the child; deadlines kill and reap it.
/// Private temporary files are deleted on every return path. Reads are bounded.
pub(crate) fn bounded_output(
    command: &mut Command,
    timeout: Duration,
) -> Result<std::process::Output, String> {
    let mut stdout = tempfile::tempfile().map_err(|e| e.to_string())?;
    let mut stderr = tempfile::tempfile().map_err(|e| e.to_string())?;
    let mut child = command
        .stdin(std::process::Stdio::null())
        .stdout(stdout.try_clone().map_err(|e| e.to_string())?)
        .stderr(stderr.try_clone().map_err(|e| e.to_string())?)
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            state => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(match state {
                    Err(e) => e.to_string(),
                    _ => "处理超时，已结束子进程".into(),
                });
            }
        }
    };
    let read = |file: &mut std::fs::File| -> Result<Vec<u8>, String> {
        file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let mut data = Vec::new();
        file.take(1024 * 1024 + 1)
            .read_to_end(&mut data)
            .map_err(|e| e.to_string())?;
        if data.len() > 1024 * 1024 {
            return Err("返回内容过大，请缩小选区".into());
        }
        Ok(data)
    };
    Ok(std::process::Output {
        status,
        stdout: read(&mut stdout)?,
        stderr: read(&mut stderr)?,
    })
}

fn split_lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}
