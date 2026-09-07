//! macOS 实现：
//! - 划词：优先 AX；必要时模拟 Cmd+C，保存并按 changeCount 恢复全部剪贴板类型
//! - 截图：`screencapture -x -R` CLI（需屏幕录制权限）
//! - OCR：Apple Vision 框架（VNRecognizeTextRequest，离线，高质量）

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
    use core_foundation::{
        base::{CFGetTypeID, TCFType},
        string::{CFString, CFStringRef},
    };
    use std::ffi::c_void;
    #[link(name = "HIServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateSystemWide() -> *mut c_void;
        fn AXUIElementSetMessagingTimeout(element: *const c_void, seconds: f32) -> i32;
        fn AXUIElementCopyAttributeValue(
            element: *const c_void,
            attribute: *const c_void,
            value: *mut *mut c_void,
        ) -> i32;
        fn CFRelease(value: *const c_void);
    }
    struct Owned(*mut c_void);
    impl Drop for Owned {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { CFRelease(self.0) };
            }
        }
    }
    unsafe {
        let system = Owned(AXUIElementCreateSystemWide());
        if system.0.is_null() {
            return None;
        }
        AXUIElementSetMessagingTimeout(system.0, 0.8);
        let mut focused = Owned(std::ptr::null_mut());
        if AXUIElementCopyAttributeValue(
            system.0,
            CFString::new("AXFocusedUIElement").as_CFTypeRef(),
            &mut focused.0,
        ) != 0
            || focused.0.is_null()
        {
            return None;
        }
        AXUIElementSetMessagingTimeout(focused.0, 0.8);
        let mut value = Owned(std::ptr::null_mut());
        if AXUIElementCopyAttributeValue(
            focused.0,
            CFString::new("AXSelectedText").as_CFTypeRef(),
            &mut value.0,
        ) != 0
            || value.0.is_null()
        {
            return None;
        }
        if CFGetTypeID(value.0) != CFString::type_id() {
            return None;
        }
        let text = CFString::wrap_under_get_rule(value.0 as CFStringRef).to_string();
        (!text.trim().is_empty()).then_some(text)
    }
}

pub fn capture_selection_text() -> Option<String> {
    if let Some(text) = ax_selected_text() {
        return Some(text);
    }
    // Snapshot every item/type before invoking Copy. If a lazy/promised payload
    // cannot be preserved or is huge, leave it untouched rather than destroy it.
    use objc2::{rc::Retained, runtime::ProtocolObject};
    use objc2_app_kit::{NSPasteboard, NSPasteboardItem, NSPasteboardWriting};
    use objc2_foundation::{NSArray, NSString};
    let board = NSPasteboard::generalPasteboard();
    let original_count = board.changeCount();
    let mut originals: Vec<Retained<NSPasteboardItem>> = Vec::new();
    let mut bytes = 0;
    if let Some(items) = board.pasteboardItems() {
        for item in items.iter() {
            let saved = NSPasteboardItem::new();
            for kind in item.types().iter() {
                let data = item.dataForType(&kind)?;
                bytes += data.length();
                if bytes > 32 * 1024 * 1024 || !saved.setData_forType(&data, &kind) {
                    return None;
                }
            }
            originals.push(saved);
        }
    }
    if board.changeCount() != original_count {
        return None;
    }
    post_copy_keystroke().ok()?; // Never clear the pasteboard to detect failure.
    let deadline = std::time::Instant::now() + Duration::from_millis(400);
    while board.changeCount() == original_count {
        if std::time::Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let copied_count = board.changeCount();
    let text = board
        .stringForType(&NSString::from_str("public.utf8-plain-text"))
        .map(|s| s.to_string());
    // Restore immediately, only if no newer owner has written meanwhile.
    // No delayed task is allowed to overwrite a subsequent user copy.
    if board.changeCount() == copied_count {
        let objects: Vec<&ProtocolObject<dyn NSPasteboardWriting>> = originals
            .iter()
            .map(|i| ProtocolObject::from_ref(&**i))
            .collect();
        board.clearContents();
        if !objects.is_empty() {
            board.writeObjects(&NSArray::from_slice(&objects));
        }
    }
    text.filter(|s| !s.trim().is_empty())
}

// ---------------------------------------------------------------------------
// 截图：screencapture CLI
// ---------------------------------------------------------------------------

pub fn screen_capture_available() -> bool {
    use core_graphics::access::ScreenCaptureAccess;
    ScreenCaptureAccess.preflight()
}

pub fn request_screen_capture() -> bool {
    use core_graphics::access::ScreenCaptureAccess;
    let access = ScreenCaptureAccess;
    if access.preflight() {
        return true;
    }
    // 请求后重新检查当前进程的授权；不缓存历史成功状态。
    let _ = access.request();
    access.preflight()
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
        AXIsProcessTrustedWithOptions(dict.as_concrete_TypeRef() as *const c_void);
    }
}

pub fn capture_screen_region(region: &Region) -> Result<Vec<u8>, String> {
    if region.w <= 0 || region.h <= 0 {
        return Err("截图区域无效".into());
    }
    if !screen_capture_available() {
        return Err(capture_error("当前进程没有屏幕录制权限", false));
    }
    let dir = tempfile::tempdir().map_err(|e| format!("创建截图临时目录失败: {e}"))?;
    let path = dir.path().join("capture.png");
    let mut child = Command::new("/usr/sbin/screencapture")
        .arg("-x")
        .arg("-T0")
        .arg(format!(
            "-R{},{},{},{}",
            region.x, region.y, region.w, region.h
        ))
        .arg(&path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("执行 screencapture 失败: {e}"))?;
    let deadline = std::time::Instant::now() + Duration::from_secs(8);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20))
            }
            state => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(match state {
                    Err(e) => format!("截图进程异常: {e}"),
                    _ => "截图超时，请重新选择区域".into(),
                });
            }
        }
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("读取截图进程结果失败: {e}"))?;
    if !out.status.success() {
        let _ = std::fs::remove_file(&path);
        let detail = format!(
            "screencapture 失败（{}）: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
        return Err(capture_error(&detail, screen_capture_available()));
    }
    let png = std::fs::read(&path).map_err(|e| format!("读取截图失败: {e}"));
    let _ = std::fs::remove_file(&path);
    let png = png?;
    if png.is_empty() {
        return Err(capture_error("截图为空", screen_capture_available()));
    }
    Ok(png)
}

fn capture_error(detail: &str, has_access: bool) -> String {
    if has_access {
        format!("截图失败: {detail}")
    } else {
        format!("SCREEN_CAPTURE_DENIED:{detail}")
    }
}

// ---------------------------------------------------------------------------
// OCR：Apple Vision（离线）
// ---------------------------------------------------------------------------

pub fn ocr_png(png: &[u8]) -> Result<Vec<String>, String> {
    use objc2::rc::{autoreleasepool, Retained};
    use objc2::AnyThread;
    use objc2_foundation::{NSArray, NSData, NSDictionary, NSString};
    use objc2_vision::{
        VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel,
    };

    autoreleasepool(|_| {
        // 直接识别内存中的 PNG，避免为每次 OCR 再写读一次临时文件。
        let data = NSData::with_bytes(png);
        let handler = VNImageRequestHandler::initWithData_options(
            VNImageRequestHandler::alloc(),
            &data,
            &NSDictionary::new(),
        );

        let request = VNRecognizeTextRequest::new();
        request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
        request.setUsesLanguageCorrection(true);
        // 此值是相对图片高度的比例，不是像素。0 保留截图里的小字。
        request.setMinimumTextHeight(0.0);
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_failure_with_permission_does_not_show_permission_card() {
        assert!(!capture_error("invalid display", true).starts_with("SCREEN_CAPTURE_DENIED:"));
        assert!(capture_error("revoked", false).starts_with("SCREEN_CAPTURE_DENIED:"));
    }

    #[test]
    fn invalid_region_is_not_a_permission_error() {
        let error = capture_screen_region(&Region {
            x: 0,
            y: 0,
            w: 0,
            h: 10,
        })
        .unwrap_err();
        assert_eq!(error, "截图区域无效");
    }
}
