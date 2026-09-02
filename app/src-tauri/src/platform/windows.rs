//! Windows 实现：
//! - 划词：arboard 剪贴板 + SendInput 模拟 Ctrl+C（Windows 无 TCC 类权限体系，无需引导）
//! - 截图：screenshots crate（GDI 封装，跨 Windows 版本）
//! - OCR：Windows.Media.Ocr（WinRT，系统内置离线识别）

use super::Region;

// ---------------------------------------------------------------------------
// 划词：模拟 Ctrl+C
// ---------------------------------------------------------------------------

/// VK codes
const VK_CONTROL: u16 = 0x11;
const VK_C: u16 = 0x43;

fn key_input(vk: u16, key_up: bool) -> windows::Win32::UI::Input::KeyboardAndMouse::INPUT {
    use windows::Win32::UI::Input::KeyboardAndMouse::{INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY};
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: if key_up { KEYEVENTF_KEYUP } else { Default::default() },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn post_copy_keystroke() -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::SendInput;
    unsafe {
        let inputs = [
            key_input(VK_CONTROL, false),
            key_input(VK_C, false),
            key_input(VK_C, true),
            key_input(VK_CONTROL, true),
        ];
        let sent = SendInput(
            &inputs,
            std::mem::size_of::<windows::Win32::UI::Input::KeyboardAndMouse::INPUT>() as i32,
        );
        if sent as usize != inputs.len() {
            return Err("SendInput 发送失败".into());
        }
    }
    Ok(())
}

pub fn capture_selection_text() -> Option<String> {
    let mut cb = arboard::Clipboard::new().ok()?;
    let original = cb.get_text().ok().unwrap_or_default();
    let _ = cb.set_text(String::new());
    post_copy_keystroke().ok()?;
    std::thread::sleep(std::time::Duration::from_millis(250));
    let text = cb.get_text().ok().filter(|s| !s.trim().is_empty());
    if !original.is_empty() {
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(600));
            if let Ok(mut cb) = arboard::Clipboard::new() {
                let _ = cb.set_text(original);
            }
        });
    }
    text
}

pub fn accessibility_available() -> bool {
    true // Windows 无 TCC 权限体系
}

pub fn screen_capture_available() -> bool {
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
            let (x, y) = (s.display_info.x, s.display_info.y);
            region.x >= x && region.y >= y
        })
        .or_else(|| screens.first())
        .ok_or("无可用显示器")?;

    // capture_area 接收相对当前显示器的坐标（内部会加回显示器原点）
    let rel_x = (region.x - screen.display_info.x).max(0);
    let rel_y = (region.y - screen.display_info.y).max(0);
    let img = screen
        .capture_area(rel_x, rel_y, region.w.max(1) as u32, region.h.max(1) as u32)
        .map_err(|e| format!("截图失败: {e}"))?;

    encode_png(img.width(), img.height(), &img.into_raw())
}

/// RGBA → PNG（image crate）
fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    use image::ImageEncoder;
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(std::io::Cursor::new(&mut out))
        .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|e| format!("PNG 编码失败: {e}"))?;
    Ok(out)
}

// ---------------------------------------------------------------------------
// OCR：Windows.Media.Ocr（WinRT）
// ---------------------------------------------------------------------------

pub fn ocr_png(png: &[u8]) -> Result<Vec<String>, String> {
    use windows::Globalization::Language;
    use windows::Graphics::Imaging::BitmapDecoder;
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

    // PNG → InMemoryRandomAccessStream
    let stream = InMemoryRandomAccessStream::new().map_err(|e| e.to_string())?;
    let writer = DataWriter::CreateDataWriter(&stream).map_err(|e| e.to_string())?;
    writer
        .WriteBytes(png)
        .map_err(|e| e.to_string())?;
    writer
        .StoreAsync()
        .map_err(|e| e.to_string())?
        .get()
        .map_err(|e| e.to_string())?;
    writer
        .FlushAsync()
        .map_err(|e| e.to_string())?
        .get()
        .map_err(|e| e.to_string())?;
    writer.DetachStream().map_err(|e| e.to_string())?;
    stream.Seek(0).map_err(|e| e.to_string())?;

    let decoder = BitmapDecoder::CreateAsync(&stream)
        .map_err(|e| e.to_string())?
        .get()
        .map_err(|e| e.to_string())?;
    let bitmap = decoder
        .GetSoftwareBitmapAsync()
        .map_err(|e| e.to_string())?
        .get()
        .map_err(|e| e.to_string())?;

    // 优先中文引擎（支持中英混排），失败回退用户语言
    let engine = match Language::CreateLanguage(&windows::core::HSTRING::from("zh-Hans-CN")) {
        Ok(lang) => OcrEngine::TryCreateFromLanguage(&lang)
            .ok()
            .or_else(|| OcrEngine::TryCreateFromUserProfileLanguages().ok()),
        Err(_) => OcrEngine::TryCreateFromUserProfileLanguages().ok(),
    }
    .ok_or("系统未安装可用的 OCR 语言包（设置 → 时间和语言 → 语言）")?;

    let result = engine
        .RecognizeAsync(&bitmap)
        .map_err(|e| e.to_string())?
        .get()
        .map_err(|e| e.to_string())?;
    let text = result
        .Text()
        .map_err(|e| e.to_string())?
        .to_string();

    Ok(text
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}
