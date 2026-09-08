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

持续开发中，安装包以 [Releases](https://github.com/lucaslus/luca-translate/releases) 为准；尚未发布时可从源码运行。CI 配置覆盖 macOS Apple Silicon / Intel、Windows x64、Linux x64 的测试与安装包构建；Windows/Linux 真机体验仍待完整验证。Linux 当前以 X11 为主，Wayland 尚不完整支持。

Under active development. See [Releases](https://github.com/lucaslus/luca-translate/releases) for published installers, or build from source. CI is configured for macOS Apple Silicon / Intel, Windows x64, and Linux x64 tests and installer builds. Windows/Linux desktop validation is still pending; Linux targets X11, with incomplete Wayland support.

### 安装与更新 / Install & update

macOS 版本未使用 Apple Developer ID 签名或公证，也不上架 App Store。**确认来自本仓库 Releases** 后，如系统阻止打开，请先尝试启动，再到「系统设置 → 隐私与安全性 → 仍要打开」。这是安装信任确认，与辅助功能/屏幕录制权限不同。无需关闭系统安全保护。[Apple 说明](https://support.apple.com/en-us/102445)

The macOS app is not Developer ID–signed or notarized and is not distributed through the App Store. **Only for a release you trust from this repository**, try launching it, then choose **System Settings → Privacy & Security → Open Anyway** if blocked. This is separate from Accessibility/Screen Recording permissions; do not disable system-wide security. [Apple guidance](https://support.apple.com/en-us/102445)

在「设置 → 关于」检查更新，确认后下载、验证签名并安装。使用 [Tauri Updater](https://v2.tauri.app/plugin/updater/)，无需 Apple 开发者账号；项目更新签名不等于 Apple 公证。Linux 自动更新适用于 AppImage，DEB/RPM 请手动更新。未发布更新源时会提示暂不可用。

Check for updates in **Settings → About**, then confirm to download, verify, and install. [Tauri Updater](https://v2.tauri.app/plugin/updater/) verifies project signatures independently of Apple notarization. Linux in-app updates support AppImage; update DEB/RPM manually. Until a release feed is published, update checks may be unavailable.

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

以上为默认快捷键，可在设置中修改；冲突时保留原设置。 / These defaults are editable in Settings; conflicts leave your previous bindings intact.

<a id="docs"></a>

## 文档 / Docs

[架构 / Architecture](docs/ARCHITECTURE.md) · [服务 / Services](docs/SERVICES.md) · [错误与日志 / Troubleshooting](docs/ERROR-RECOVERY.md) · [OCR 权限 / Permissions](docs/OCR-PERMISSION-FIX.md) · [发布 / Releases](docs/RELEASE.md)

## License

[MIT](LICENSE). Independent project; not affiliated with Bob Translate. / 独立项目，与 Bob Translate 无隶属关系。
