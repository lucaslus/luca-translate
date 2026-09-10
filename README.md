<h1><img src="app/src-tauri/icons/128x128.png" alt="Lucas Translate app icon" width="40" height="40" align="absmiddle" /> Lucas Translate</h1>

**划词即译，截图取字。 / Select to translate. Capture to extract text.**

[![Build & tests](https://github.com/lucaslus/luca-translate/actions/workflows/check.yml/badge.svg)](https://github.com/lucaslus/luca-translate/actions/workflows/check.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[中文](#中文) · [English](#english) · [快速开始 / Quick start](#quick-start) · [文档 / Docs](#docs)

## 中文

面向 macOS、Windows 和 Linux 的**开源 Bob Translate 平替**。划词翻译、截图 OCR、AI 翻译和查词，一个桌面工具完成。基于 Rust + Tauri 2 构建。

- **翻译对照**：划词、输入、截图翻译，多渠道并行，失败可单独重试。
- **截图取字**：本地离线 OCR，静默模式直接复制文字，不弹翻译窗口。
- **自由选服务**：有道、DeepL、Bing、Google，支持 DeepL 官方 API、OpenAI 兼容服务和本地 Ollama。
- **查词与记录**：音标及发音、中文拼音、历史与收藏；自定义快捷键、字号、窗口大小与主题。

**隐私**：图片不上传，翻译文字仅发送给已启用的服务；静默 OCR 不调用翻译。API Key 存于系统凭据存储。免费网页通道的可用性受网络和服务商限流影响。

## English

An **open-source Bob Translate alternative** for macOS, Windows, and Linux. Text translation, screenshot OCR, AI translation, and dictionary lookups in one desktop app, built with Rust and Tauri 2.

- **Compare translations**: translate selected text, input, or screenshots with parallel results and per-service retries.
- **Extract text locally**: offline OCR with a silent copy-to-clipboard mode—no translation window.
- **Choose your services**: Youdao, DeepL, Bing, Google, the official DeepL API, and OpenAI-compatible endpoints, including local Ollama.
- **Look up and save**: phonetics, pronunciation, Chinese pinyin, history, and favorites. Customize shortcuts, text size, window size, and theme.

**Privacy**: images stay local; only text is sent to enabled translation services. Silent OCR makes no translation requests. API keys stay in the system credential store. Free web endpoints are subject to network availability and rate limits.

## 开发状态 / Status

持续开发中，安装包以 [Releases](https://github.com/lucaslus/luca-translate/releases) 为准；尚未发布时可从源码运行。CI 配置覆盖 macOS Apple Silicon / Intel、Windows x64、Linux x64 的测试与安装包构建，包含 Arch 原生包。已在 Omarchy 真机验证安装、浮动窗口、聚焦和 Esc 隐藏；其他平台及完整翻译/OCR 流程仍需持续验证。Linux 支持 X11，并提供 Omarchy / Hyprland 专用集成；其他 Wayland 桌面尚未完整支持。

Under active development. See [Releases](https://github.com/lucaslus/luca-translate/releases) for published installers, or build from source. CI covers macOS Apple Silicon / Intel, Windows x64, and Linux x64, including native Arch packages. Installation, floating windows, focus, and Escape dismissal have been verified on Omarchy. Linux supports X11 and dedicated Omarchy / Hyprland integration; other Wayland desktops and complete translation/OCR workflows still need further validation.

### 安装与更新 / Install & update

**Arch Linux / Omarchy（x86_64）**：下载 Release 中的 `lucas-translate-<版本>-1-x86_64.pkg.tar.zst`，运行 `sudo pacman -U ./lucas-translate-<版本>-1-x86_64.pkg.tar.zst`。安装后在应用菜单打开 **Lucas Translate**，或运行 `lucas-translate`。依赖包含划词工具及中英文 OCR 语言包；API 凭据还需可用且已解锁的 Secret Service（例如 GNOME Keyring）。后续版本使用相同的 `pacman -U` 命令升级；卸载用 `sudo pacman -R lucas-translate`。Omarchy / Hyprland 首次启动会自动启用下方专用集成；其他 Wayland 桌面仍未完整适配。

**Arch Linux / Omarchy (x86_64)**: download the `.pkg.tar.zst` Release asset and install/upgrade with `sudo pacman -U ./lucas-translate-<version>-1-x86_64.pkg.tar.zst`. Launch **Lucas Translate** from the application menu. Native packages include X11 selection and Chinese/English OCR dependencies and use manual package-manager updates. The integration below is enabled automatically on first launch on Omarchy with Lua configuration.

macOS 版本未使用 Apple Developer ID 签名或公证，也不上架 App Store。**确认来自本仓库 Releases** 后，如系统阻止打开，请先尝试启动，再到「系统设置 → 隐私与安全性 → 仍要打开」。这是安装信任确认，与辅助功能/屏幕录制权限不同。无需关闭系统安全保护。[Apple 说明](https://support.apple.com/en-us/102445)

The macOS app is not Developer ID–signed or notarized and is not distributed through the App Store. **Only for a release you trust from this repository**, try launching it, then choose **System Settings → Privacy & Security → Open Anyway** if blocked. This is separate from Accessibility/Screen Recording permissions; do not disable system-wide security. [Apple guidance](https://support.apple.com/en-us/102445)

在「设置 → 关于」检查更新，确认后下载、验证签名并安装。使用 [Tauri Updater](https://v2.tauri.app/plugin/updater/)，无需 Apple 开发者账号；项目更新签名不等于 Apple 公证。Linux 自动更新适用于 AppImage，DEB/RPM/Arch 请手动更新。未发布更新源时会提示暂不可用。

Check for updates in **Settings → About**, then confirm to download, verify, and install. [Tauri Updater](https://v2.tauri.app/plugin/updater/) verifies project signatures independently of Apple notarization. Linux in-app updates support AppImage; update DEB/RPM/Arch manually. Until a release feed is published, update checks may be unavailable.

### Omarchy / Hyprland 浮动面板 / Floating panel

使用 Lua 配置的 Omarchy 安装 Arch 包后，从应用菜单打开一次 **Lucas Translate**，程序会以当前用户自动启用快捷键与浮窗规则。无需手动配置；自动启用失败时，可在解决提示的问题后运行以下命令重试：

On Omarchy with Lua configuration, open **Lucas Translate** once after installing the Arch package to enable the integration automatically. To retry manually after resolving a reported problem:

```sh
sudo pacman -S --needed python wl-clipboard grim slurp
lucas-translate-setup-omarchy
```

第二条命令以当前桌面用户运行，**不要加 sudo**。安装器检查快捷键冲突、备份个人配置并验证重载；发现占用时退出，不覆盖系统绑定。pacman 安装阶段不修改桌面配置，自动配置在当前用户首次启动应用时完成。

Run the setup command **without sudo**. It checks shortcut conflicts, backs up your configuration, and validates the reload. Occupied shortcuts are not replaced. Desktop configuration is applied on the first app launch as the desktop user, not by pacman.

| Omarchy 快捷键 / Shortcut | 功能 / Action |
| --- | --- |
| `Super+Ctrl+Shift+T` | 唤起并聚焦；已聚焦时隐藏 / Show and focus; hide when focused |
| `Super+Ctrl+Shift+D` | 划词翻译 / Translate selected text |
| `Super+Ctrl+Shift+S` | 框选截图翻译 / Translate a screenshot region |
| `Super+Ctrl+Shift+C` | 静默 OCR，仅复制文字 / OCR to clipboard without opening the panel |
| `Super+Ctrl+Shift+P` | 系统框选截图，复制图片（不做 OCR 或翻译） / Native region screenshot to image clipboard |
| `Esc` | 隐藏主面板，保留输入；框选时取消 / Hide and preserve input; cancel region selection |

- Omarchy 主面板隐藏原生标题栏和置顶控件，设置入口位于底栏；按 Esc 隐藏。
- 默认 **420×650 居中浮窗**，可拖动和调整尺寸；应用未运行时快捷键会启动它。
- **失焦不会自动隐藏**，避免 Wayland 聚焦时序导致划词结果闪现消失。点击浮窗外部、按 Esc 或聚焦时再次按切换键隐藏；托盘菜单“退出”才会结束进程。
- **纯截图复制**仅在 Omarchy 提供：调用系统截图工具，先隐藏翻译面板；框选后可直接粘贴图片，Esc 取消，翻译程序未运行时也可用。
- 快捷键由 Hyprland 管理；修改应用内的 Alt/Option 快捷键不会改变上述绑定。
- 划词通过 `wl-paste` 读取 PRIMARY 选区；部分应用不支持它，需要手动复制并粘贴。截图通过 `slurp` / `grim`，OCR 在本地执行。

The panel floats centered at **420×650** and can be moved or resized. It explicitly requests compositor focus and **stays visible when focus changes**. The native title bar and pin control are hidden, with Settings available in the bottom bar. Clicking outside, Escape, or toggling the focused panel hides it without quitting; use the tray menu to exit. Hyprland owns these shortcuts. Selection capture requires the source application to expose a PRIMARY selection; otherwise copy and paste manually. Region capture uses `slurp` / `grim` with local OCR.

配置位置、命令接口和卸载集成方法见 [Omarchy 集成说明 / Integration guide](docs/OMARCHY.md)。

<a id="quick-start"></a>

## 快速开始 / Quick start

安装 Rust 和 [Tauri 2 平台依赖](https://v2.tauri.app/start/prerequisites/)，然后从源码运行。

Install Rust and the [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/), then run from source.

```bash
git clone https://github.com/lucaslus/luca-translate.git
cd luca-translate
cargo install tauri-cli --version "^2" --locked
cd app
cargo tauri dev
```

macOS 首次使用需授予辅助功能与屏幕录制权限；仅在系统提示时重启。Linux 划词和 OCR 还需安装 `xclip`、Tesseract 及 `eng` / `chi_sim` 语言包。

On macOS, grant Accessibility and Screen Recording access when requested; restart only if prompted. On Linux, selection capture and OCR also require `xclip`, Tesseract, and the `eng` / `chi_sim` language packs.

| 快捷键 / Shortcut | 功能 / Action |
| --- | --- |
| `Alt/Option + A` | 输入翻译 / Open translation input |
| `Alt/Option + D` | 划词翻译 / Translate selected text |
| `Alt/Option + S` | 截图翻译 / Translate a screenshot |
| `Alt/Option + C` | 静默 OCR，仅复制文字 / Silent OCR, copy text only |

以上为非 Hyprland 会话的默认快捷键，可在设置中修改；冲突时保留原设置。Omarchy 使用上方的 Super 组合，由桌面配置管理。 / These defaults apply outside Hyprland and can be edited in Settings; conflicts preserve existing bindings. Omarchy uses the compositor-managed Super combinations above.

<a id="docs"></a>

## 文档 / Docs

[架构 / Architecture](docs/ARCHITECTURE.md) · [服务 / Services](docs/SERVICES.md) · [错误与日志 / Troubleshooting](docs/ERROR-RECOVERY.md) · [OCR 权限 / Permissions](docs/OCR-PERMISSION-FIX.md) · [发布 / Releases](docs/RELEASE.md) · [Omarchy 集成 / Integration](docs/OMARCHY.md)

## License

[MIT](LICENSE). Independent project; not affiliated with Bob Translate. / 独立项目，与 Bob Translate 无隶属关系。
