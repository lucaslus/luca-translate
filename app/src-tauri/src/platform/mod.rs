//! Platform dispatch. Keep full application and platform-check builds aligned.
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "linux")]
use linux as implementation;
#[cfg(target_os = "macos")]
use macos as implementation;
#[cfg(target_os = "linux")]
pub use screenshots as screenshots_impl;
#[cfg(target_os = "windows")]
use windows as implementation;
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}
pub fn capture_selection_text() -> Option<String> {
    implementation::capture_selection_text()
}
pub fn capture_screen_region(region: &Region) -> Result<Vec<u8>, String> {
    implementation::capture_screen_region(region)
}
pub fn ocr_png(png: &[u8]) -> Result<Vec<String>, String> {
    implementation::ocr_png(png)
}
pub fn screen_capture_available() -> bool {
    implementation::screen_capture_available()
}
pub fn accessibility_available() -> bool {
    implementation::accessibility_available()
}
pub fn request_screen_capture() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::request_screen_capture()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}
pub fn prompt_accessibility() {
    #[cfg(target_os = "macos")]
    macos::prompt_accessibility();
}
