#[path = "../src/platform/mod.rs"]
mod platform;

fn main() {
    println!("辅助功能权限: {}", platform::accessibility_available());
    println!("屏幕录制权限: {}", platform::screen_capture_available());
    println!("AX 选中文本: {:?}", platform::capture_selection_text()
        .map(|t| format!("{} 字符: {}", t.chars().count(), t.chars().take(30).collect::<String>())));
}
