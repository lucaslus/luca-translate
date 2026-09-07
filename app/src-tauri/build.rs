fn main() {
    if cfg!(target_os = "macos") {
        // AX 辅助功能 API 位于 ApplicationServices 的子框架 HIServices，
        // 子框架不在默认链接搜索路径中，需要显式加入
        println!(
            "cargo:rustc-link-search=framework=/System/Library/Frameworks/ApplicationServices.framework/Frameworks"
        );
    }
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "translate",
            "cancel_translation",
            "cancel_ocr",
            "service_catalog",
            "capture_selection",
            "history_list",
            "history_clear",
            "favorite_add",
            "favorites_list",
            "favorite_remove",
            "get_ai_config",
            "set_ai_config",
            "permission_status",
            "open_permission_settings",
            "open_settings",
            "restart_app",
            "start_screenshot",
            "get_routing",
            "set_routing",
            "get_services",
            "set_service",
            "diagnostics_status",
            "open_diagnostics",
        ]),
    ))
    .expect("生成应用权限失败")
}
