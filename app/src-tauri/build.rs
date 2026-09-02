fn main() {
    if cfg!(target_os = "macos") {
        // AX 辅助功能 API 位于 ApplicationServices 的子框架 HIServices，
        // 子框架不在默认链接搜索路径中，需要显式加入
        println!(
            "cargo:rustc-link-search=framework=/System/Library/Frameworks/ApplicationServices.framework/Frameworks"
        );
    }
    tauri_build::build()
}
