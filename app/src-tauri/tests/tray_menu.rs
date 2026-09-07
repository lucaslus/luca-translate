//! Structural guard for the native tray configuration, not an AppKit UI test.
//! Menu tracking can also deliver pointer events; attaching a second action to
//! those events must not reopen the main window while using/dismissing the menu.

#[test]
fn native_tray_menu_does_not_also_handle_pointer_events() {
    let source = include_str!("../src/main.rs");
    let builder = source
        .split_once("let _tray = TrayIconBuilder::with_id(\"main-tray\")")
        .expect("main tray builder must exist")
        .1
        .split_once(".build(app)?;")
        .expect("main tray builder must be completed")
        .0;

    assert!(builder.contains(".show_menu_on_left_click(true)"));
    assert!(builder.contains(".on_menu_event("));
    assert!(builder.contains("\"open\" => show_main(app)"));
    assert!(
        !builder.contains(".on_tray_icon_event("),
        "The tray opens a native menu; pointer events must not also show a window"
    );
}
