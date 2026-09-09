# Omarchy / Hyprland 浮动翻译面板

适用于使用 Lua 配置的 Omarchy / Hyprland。Arch 包附带配置，不会在 pacman 安装时自动改写个人桌面配置。

安装包后，以当前桌面用户运行（不用 sudo）：

```sh
lucas-translate-setup-omarchy
```

需要 `python`、`wl-clipboard`、`grim`、`slurp` 和包内声明的 Tesseract 中英文语言数据。安装器读取当前 Hyprland 绑定，发现目标快捷键冲突就退出，不覆盖系统绑定。它备份 `~/.config/hypr/hyprland.lua`，引入 `~/.config/hypr/lucas-translate.lua`，重载并检查配置错误；失败恢复原配置。已有自定义集成文件不会被覆盖。

| 快捷键 | 行为 |
| --- | --- |
| Super + Ctrl + Shift + T | 未运行时启动；已隐藏时唤起；主窗口已聚焦时隐藏 |
| Super + Ctrl + Shift + D | 读取 Wayland PRIMARY 选区并翻译 |
| Super + Ctrl + Shift + S | 框选截图，OCR 后翻译 |
| Super + Ctrl + Shift + C | 框选截图，OCR 文字复制到剪贴板，不打开翻译窗口 |
| Esc | 主界面关闭浮层/取消当前工作后隐藏；截图选区中取消截图 |

主窗口默认浮动、居中，420×650（逻辑像素），可以拖动和调整尺寸。关闭按钮隐藏到后台；Omarchy 下失焦不会自动隐藏，避免划词完成后的异步聚焦导致窗口闪现消失；请按 Esc、再次使用切换快捷键或点击关闭按钮隐藏。托盘菜单的“退出”才会退出进程。这是普通 Wayland 应用浮窗，不是 Omarchy menu 的 layer-shell 表面；锁屏或独占全屏上的覆盖不保证。

在 Hyprland 会话中，应用不再注册自己的四个全局快捷键。系统绑定由上面的 Lua 文件管理，应用设置中的跨平台快捷键配置不会改写它。改键时先查 `omarchy menu keybindings --print`，编辑个人 Lua 文件后执行 `hyprctl reload` 和 `hyprctl configerrors`。

命令接口：`lucas-translate --toggle` / `--show` / `--hide` / `--selection` / `--screenshot` / `--ocr`。命令转交给现有单实例，冷启动会等待前端监听器就绪。划词仅读选区，不模拟 Ctrl+C，也不会拿旧剪贴板冒充选区；不支持 PRIMARY 的应用请手动复制、打开面板粘贴。截图用 slurp/grim，OCR 本地执行；静默 OCR 只写剪贴板，不调用翻译服务。选区等待上限 120 秒，Esc 可取消；OCR 执行期间重复截图请求忽略。

撤销集成：从 `~/.config/hypr/hyprland.lua` 删除 Lucas Translate 的 `dofile(...)` 行，删除 `~/.config/hypr/lucas-translate.lua`，重载并检查配置。包升级不改写个人配置；可与 `/usr/share/lucas-translate/omarchy/lucas-translate.lua` 比较后手动更新。
