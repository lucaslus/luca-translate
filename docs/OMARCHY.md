# Omarchy / Hyprland 浮动翻译面板

适用于使用 Lua 配置的 Omarchy / Hyprland。Arch 包附带配置，不会在 pacman 安装时自动改写个人桌面配置。

安装包后，从应用菜单打开一次 Lucas Translate，程序会以当前用户自动启用集成；配置没有变化时不会重复改写或重载。自动启用失败会显示错误，解决后可手动重试（不用 sudo）：

```sh
lucas-translate-setup-omarchy
```

需要 `python`、`wl-clipboard`、`grim`、`slurp` 和包内声明的 Tesseract 中英文语言数据。安装器读取当前 Hyprland 绑定，逐项检测冲突；被占用的默认键位留空，其余快捷键正常启用。设置页会显示冲突来源并提示重新设置，不覆盖系统或用户原有绑定。它备份 `~/.config/hypr/hyprland.lua`，引入 `~/.config/hypr/lucas-translate.lua`，重载并检查配置错误；失败恢复原配置。已有自定义集成文件不会被覆盖。

| 快捷键 | 行为 |
| --- | --- |
| Super + Ctrl + Shift + I | 打开空白输入，清空旧原文和结果并聚焦；Enter 翻译，Shift+Enter 换行 |
| Super + Ctrl + Shift + T | 未运行时启动；已隐藏时唤起；主窗口已聚焦时隐藏 |
| Super + Ctrl + Shift + D | 读取 Wayland PRIMARY 选区并翻译 |
| Super + Ctrl + Shift + S | 框选截图，OCR 后翻译 |
| Super + Ctrl + Shift + C | 框选截图，OCR 文字复制到剪贴板，不打开翻译窗口 |
| Super + Ctrl + Shift + P | 系统框选截图并编辑；✔ 复制并关闭，× / Esc / 点击外部丢弃 |
| Esc | 主界面关闭浮层/取消当前工作后隐藏；截图选区中取消截图 |

主窗口默认浮动、居中，420×650（逻辑像素）。Omarchy 下隐藏原生标题栏和置顶按钮，设置入口移至底栏，按 Esc 隐藏；移动和调整尺寸使用窗口管理器快捷操作。Omarchy 下失焦不会自动隐藏，避免划词完成后的异步聚焦导致窗口闪现消失；点击浮窗外部、按 Esc 或主窗口已聚焦时再次使用切换快捷键隐藏。托盘菜单的“退出”才会退出进程。这是普通 Wayland 应用浮窗，不是 Omarchy menu 的 layer-shell 表面；锁屏或独占全屏上的覆盖不保证。

在 Hyprland 会话中，六个快捷键均可在「设置 → 快捷键」中修改：点击输入框，等待“录入保护已开启”后按下组合键，也可输入或粘贴组合键文字。录入期间通过专用 Hyprland submap 屏蔽常规桌面快捷键，离开输入框、按 Esc、切换窗口或关闭设置时恢复；心跳中断约 4 秒也会自动恢复。用户定义的跨 submap 通用绑定或系统保留按键仍由桌面处理。修改后自动检测占用，每行输入框后用 ✓ / × 显示结果，悬停查看原因，保存后立即生效。留空禁用；同一组合分配给多个操作也会提示冲突。保存时冲突项留空，其他项照常保存。首次安装和以后启动都会检测已有占用，冲突说明会保留到重新设置该项。检测覆盖本应用重复分配和桌面注册的快捷键；浏览器、终端、编辑器内部的快捷键无法自动枚举，✓ 不表示它们也没有冲突。字号修改不会重新注册快捷键。

快捷键和冲突说明保存于 `~/.config/hypr/lucas-translate-shortcuts.json`，由应用生成同目录下的 `lucas-translate.lua`。升级会保留通过设置页保存的自定义键位及禁用项。保存前备份，重载失败时恢复；直接手改过 Lua 的文件会被保留并提示迁移，避免覆盖用户脚本。

命令接口：`lucas-translate --input` / `--toggle` / `--show` / `--hide` / `--selection` / `--screenshot` / `--ocr`。命令转交给现有单实例，冷启动会等待前端监听器就绪。划词仅读选区，不模拟 Ctrl+C，也不会拿旧剪贴板冒充选区；不支持 PRIMARY 的应用请手动复制、打开面板粘贴。截图用 slurp/grim，OCR 本地执行；静默 OCR 只写剪贴板，不调用翻译服务。选区等待上限 120 秒，Esc 可取消；OCR 执行期间重复截图请求忽略。

撤销集成：从 `~/.config/hypr/hyprland.lua` 删除 Lucas Translate 的 `dofile(...)` 行，删除 `~/.config/hypr/lucas-translate.lua` 和 `~/.config/hypr/lucas-translate-shortcuts.json`，重载并检查配置。首次启动会备份并升级已知原版集成；自行修改过的集成不会覆盖。可与 `/usr/share/lucas-translate/omarchy/lucas-translate.lua` 比较后手动更新。

截图编辑仅在 Omarchy 提供，不要求翻译应用正在运行。截图前隐藏主面板，框选后打开包内的 `lucas-screenshot-editor`：✔ 复制并关闭；×、Esc 或点击外部丢弃，不改写剪贴板。截图仅使用临时文件，退出后自动清理，不保存到图片目录。

专用编辑器基于 Tensaku 0.29.0 的固定提交构建，补丁只添加确认/取消按钮和优先处理 Esc，不替换系统 `tensaku`。构建方式：`bash scripts/build-screenshot-editor.sh`；本地安装可将 `target/screenshot-editor/source/target/release/tensaku` 复制为 `~/.local/bin/lucas-screenshot-editor`。Arch 打包会自动构建并附带此编辑器、原项目许可和补丁。需要 GTK4、libadwaita、gtk4-layer-shell 及 Rust 构建环境。

Omarchy 下不显示 macOS 专用「权限」菜单。当前原生 Arch 包直接使用桌面会话的 Wayland 选区和截图接口，没有 macOS 式辅助功能／屏幕录制授权弹窗；这不表示安装器预先授予了系统权限。缺少 `wl-clipboard`、`grim`、`slurp`、Tesseract 语言数据，应用不支持 PRIMARY 选区，或在锁屏／不支持的会话中运行，仍可能导致功能不可用。保存 API 密钥还需要可用且已解锁的 Secret Service。安装器会安装必需依赖，Wayland 工具属于包声明的可选依赖，使用前需确认已安装。
