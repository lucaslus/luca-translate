//! 平台模块交叉编译检查用 crate（不参与应用构建）。
//! 用法：cargo check --target x86_64-pc-windows-msvc / x86_64-unknown-linux-gnu
//!
//! Linux 检查时用同 API 的 stub 替代 screenshots crate（其 Linux 后端
//! 依赖 dbus C 库无法在 macOS 交叉编译；真实 API 已在 Windows 检查中验证，
//! screenshots crate 各平台 API 一致）。

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[cfg(target_os = "windows")]
#[path = "../../../app/src-tauri/src/platform/windows.rs"]
pub mod windows;

#[cfg(target_os = "linux")]
#[path = "../../../app/src-tauri/src/platform/linux.rs"]
pub mod linux;

// Linux 检查用 stub：与 screenshots crate 公开 API 一致
#[cfg(target_os = "linux")]
pub mod screenshots_impl {
    use image::RgbaImage;

    #[derive(Debug, Clone, Copy)]
    pub struct DisplayInfo {
        pub x: i32,
        pub y: i32,
        pub width: u32,
        pub height: u32,
    }

    #[derive(Debug, Clone, Copy)]
    pub struct Screen {
        pub display_info: DisplayInfo,
    }

    #[derive(Debug)]
    pub struct Error;

    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "stub error")
        }
    }

    impl Screen {
        pub fn all() -> Result<Vec<Screen>, Error> {
            Ok(vec![Screen {
                display_info: DisplayInfo { x: 0, y: 0, width: 1920, height: 1080 },
            }])
        }

        pub fn capture_area(
            &self,
            _x: i32,
            _y: i32,
            width: u32,
            height: u32,
        ) -> Result<RgbaImage, Error> {
            Ok(RgbaImage::new(width.max(1), height.max(1)))
        }
    }
}
