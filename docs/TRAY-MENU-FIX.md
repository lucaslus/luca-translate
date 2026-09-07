# 托盘菜单意外唤起翻译窗口

## 原因与修复

主托盘同时设置了 `show_menu_on_left_click(true)`，以及在左键 `MouseButtonState::Up` 时调用 `show_main` 的鼠标回调。同一交互有两个动作入口：显示菜单和显示主窗口。

核对锁定依赖 Tauri 2.11.5 / tray-icon 0.24.2：macOS 的 `mouseDown:` 会打开原生菜单，`mouseUp:` 仍有独立鼠标事件分发入口。菜单选项处理和托盘鼠标事件不是二选一；不应依赖某次菜单追踪是否吞掉 mouse-up。参见 [Tauri 系统托盘文档](https://v2.tauri.app/learn/system-tray/)。

移除主托盘的 `on_tray_icon_event` 回调，仅保留菜单项处理：

- 点击托盘：显示菜单，不额外调用 `show_main`。
- 选择“打开翻译窗口”：显示并聚焦主窗口。
- 选择“偏好设置…”：只走设置窗口入口。
- 选择截图/静默截图：维持各自原有截图流程；截图后的 OCR 结果展示逻辑不变。
- 取消菜单：没有主窗口显示动作。

没有加入延时、防抖、轮询或新的窗口状态；没有更改窗口置顶、失焦隐藏、快捷键和权限逻辑。

## 验证与边界

新增 `app/src-tauri/tests/tray_menu.rs` 配置结构回归测试：修改前能检测出重复的鼠标事件绑定，修复后通过。该测试防止重新引入相同配置冲突，**不是 AppKit 原生点击自动化**。

本次通过 21 个桌面单元测试、2 个屏幕权限 ABI 测试和 1 个托盘配置回归测试；`git diff --check` 通过。

2026-09-07 10:16（Asia/Shanghai）已按用户要求重新构建并安装到 `/Applications/lucas-translate.app`。Release 产物和安装后的应用均通过 `codesign --verify --deep --strict`；安装后二进制 SHA-256 为 `3d5086542791bc2fa1151bccd9a9f675b8a4e75f342f98a88478933d45f5c777`，PID 31258 从固定安装路径启动且复核时保持单实例运行。没有重置权限或修改用户数据。

旧版本已备份至 `/Users/lucas/Library/Application Support/lucas-translate-backups/install.7aAuuQ`，其中压缩包已通过完整性检查。当前另一个状态组件任务的正式透明图片素材仍待处理，安装版本使用其内置降级图标。

仍需用户进行原生交互验收：主窗口隐藏时重复打开菜单后 Escape/点击外部取消；选择偏好设置；选择打开翻译窗口后再次开/关菜单；左右键、按住拖选以及主窗口置顶分别检查。预期只有明确选择打开翻译窗口或原有快捷键/OCR 流程才主动显示主窗口，不再由托盘松键额外唤起。

复跑：

```sh
cargo test --locked --offline --manifest-path app/src-tauri/Cargo.toml --bin lucas-translate --test tray_menu --test screen_capture_abi
```
