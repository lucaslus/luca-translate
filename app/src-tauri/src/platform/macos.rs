//! macOS 实现：
//! - 划词：CGEvent 模拟 Cmd+C + arboard 读写剪贴板（需辅助功能权限）
//! - 截图：`screencapture -x -R` CLI（需屏幕录制权限）
//! - OCR：Apple Vision 框架（VNRecognizeTextRequest，离线，高质量）

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::Region;

/// kVK_ANSI_C
const KEY_C: u16 = 8;

// ---------------------------------------------------------------------------
// 划词：模拟 Cmd+C
// ---------------------------------------------------------------------------

fn post_copy_keystroke() -> Result<(), String> {
    use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .map_err(|_| "创建 CGEventSource 失败".to_string())?;

    for keydown in [true, false] {
        let event = CGEvent::new_keyboard_event(source.clone(), KEY_C, keydown)
            .map_err(|_| "创建键盘事件失败".to_string())?;
        event.set_flags(CGEventFlags::CGEventFlagCommand);
        event.post(CGEventTapLocation::HID);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 划词：AX 取词优先（不碰剪贴板），失败再模拟 Cmd+C
// ---------------------------------------------------------------------------

/// AXSelectedText 快速路径：直接向当前焦点 App 询问选中文本，无需动剪贴板。
fn ax_selected_text() -> Option<String> {
    use core_foundation::base::TCFType;
    use core_foundation::string::{CFString, CFStringRef};
    use std::ffi::c_void;

    #[link(name = "HIServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateSystemWide() -> *mut c_void;
        fn AXUIElementCopyAttributeValue(
            element: *const c_void,
            attribute: *const c_void,
            value: *mut *mut c_void,
        ) -> i32;
        fn CFRelease(cf: *mut c_void);
    }

    unsafe {
        let selected_attr = CFString::new("AXSelectedText");
        let focused_attr = CFString::new("AXFocusedUIElement");
        let system_wide = AXUIElementCreateSystemWide();
        if system_wide.is_null() {
            return None;
        }

        // 路径 1：系统级元素直接拿选中文本（部分场景可用）
        let mut value: *mut c_void = std::ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(
            system_wide,
            selected_attr.as_CFTypeRef() as *const c_void,
            &mut value,
        );
        if err == 0 && !value.is_null() {
            let s = CFString::wrap_under_get_rule(value as CFStringRef).to_string();
            CFRelease(value);
            CFRelease(system_wide);
            if !s.trim().is_empty() {
                return Some(s);
            }
        } else if !value.is_null() {
            CFRelease(value);
        }

        // 路径 2：取焦点元素（属性方式，新 SDK 已移除专用函数），再取其选中文本
        let mut focused: *mut c_void = std::ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(
            system_wide,
            focused_attr.as_CFTypeRef() as *const c_void,
            &mut focused,
        );
        if err != 0 || focused.is_null() {
            CFRelease(system_wide);
            return None;
        }
        let mut value: *mut c_void = std::ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(
            focused,
            selected_attr.as_CFTypeRef() as *const c_void,
            &mut value,
        );
        CFRelease(focused);
        CFRelease(system_wide);
        if err != 0 || value.is_null() {
            return None;
        }
        let s = CFString::wrap_under_get_rule(value as CFStringRef).to_string();
        CFRelease(value);
        if s.trim().is_empty() {
            None
        } else {
            Some(s)
        }
    }
}

pub fn capture_selection_text() -> Option<String> {
    // 优先 AX 取词：不污染剪贴板、速度更快
    if let Some(text) = ax_selected_text() {
        return Some(text);
    }

    // 兜底：模拟 Cmd+C + 读写剪贴板
    let mut cb = arboard::Clipboard::new().ok()?;
    let original = cb.get_text().ok().unwrap_or_default();

    // 清空剪贴板：只有复制成功才会产生新文本，避免误读旧内容
    let _ = cb.set_text(String::new());
    post_copy_keystroke().ok()?;
    std::thread::sleep(Duration::from_millis(250));

    let text = cb
        .get_text()
        .ok()
        .filter(|s| !s.trim().is_empty());

    // 延迟恢复用户原剪贴板
    if !original.is_empty() {
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(600));
            if let Ok(mut cb) = arboard::Clipboard::new() {
                let _ = cb.set_text(original);
            }
        });
    }
    text
}

// ---------------------------------------------------------------------------
// 截图：screencapture CLI
// ---------------------------------------------------------------------------

pub fn screen_capture_available() -> bool {
    use core_graphics::access::ScreenCaptureAccess;
    ScreenCaptureAccess.preflight()
}

// AXIsProcessTrusted：检测辅助功能权限（不弹系统提示，引导由我们自己控制）
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> u8;
}

pub fn accessibility_available() -> bool {
    unsafe { AXIsProcessTrusted() != 0 }
}

/// 主动触发系统辅助功能授权提示（弹系统对话框引导用户去开权限）
pub fn prompt_accessibility() {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;
    use std::ffi::c_void;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrustedWithOptions(options: *const c_void) -> u8;
    }

    unsafe {
        let key = CFString::new("AXTrustedCheckOptionPrompt");
        let dict = CFDictionary::from_CFType_pairs(&[(key, CFBoolean::true_value())]);
        AXIsProcessTrustedWithOptions(
            dict.as_concrete_TypeRef() as *const c_void,
        );
    }
}

pub fn capture_screen_region(region: &Region) -> Result<Vec<u8>, String> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let path = std::env::temp_dir().join(format!("lucas-capture-{ts}.png"));

    let out = Command::new("screencapture")
        .arg("-x") // 静默
        .arg(format!("-R{},{},{},{}", region.x, region.y, region.w, region.h))
        .arg(&path)
        .output()
        .map_err(|e| format!("执行 screencapture 失败: {e}"))?;

    if !out.status.success() {
        return Err(format!(
            "screencapture 失败: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let png = std::fs::read(&path).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&path);
    if png.is_empty() {
        return Err("截图为空（请检查屏幕录制权限）".into());
    }
    Ok(png)
}

// ---------------------------------------------------------------------------
// OCR：Apple Vision（离线）
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
    use objc2::rc::Retained;
    use objc2::AnyThread;
    use objc2_foundation::{NSArray, NSDictionary, NSString, NSURL};
    use objc2_vision::{
        VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel,
    };

    unsafe {
        let url = NSURL::from_file_path(path).ok_or("无效的图片路径")?;
        let handler = VNImageRequestHandler::initWithURL_options(
            VNImageRequestHandler::alloc(),
            &url,
            &NSDictionary::new(),
        );

        let request = VNRecognizeTextRequest::new();
        request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
        request.setUsesLanguageCorrection(true);
        request.setMinimumTextHeight(9.0);
        let langs = NSArray::from_retained_slice(&[
            NSString::from_str("zh-Hans"),
            NSString::from_str("zh-Hant"),
            NSString::from_str("en-US"),
        ]);
        request.setRecognitionLanguages(&langs);

        // VNRecognizeTextRequest -> VNImageBasedRequest -> VNRequest 两级向上转型
        let as_image_based: Retained<objc2_vision::VNImageBasedRequest> =
            request.clone().into_super();
        let req_super: Retained<VNRequest> = as_image_based.into_super();
        let requests = NSArray::from_retained_slice(&[req_super]);
        handler
            .performRequests_error(&requests)
            .map_err(|e| format!("Vision OCR 失败: {e}"))?;

        let mut lines = Vec::new();
        if let Some(observations) = request.results() {
            for obs in observations.iter() {
                for candidate in obs.topCandidates(1).iter() {
                    let s = candidate.string().to_string();
                    if !s.trim().is_empty() {
                        lines.push(s);
                    }
                }
            }
        }
        Ok(lines)
    }
}
