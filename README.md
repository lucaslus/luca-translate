# lucas-translate

> 一款开源、跨平台（macOS / Windows / Linux）的翻译 & OCR 工具，目标是做 Bob 的平替，并在其基础上做得更好。

## 为什么做这个

[Bob](https://bobtranslate.com/) 是 macOS 上优秀的翻译/OCR 软件，但它有两个问题：

1. **只有 macOS 版**，Windows / Linux 用户没有选择；
2. **音标支持很差**：Bob 的音标只存在于金山词霸等词典通道，走普通翻译服务（火山/腾讯/DeepL/OpenAI 等）时完全没有音标，查单词体验割裂。

lucas-translate 的核心设计差异：**词典层与翻译层分离** ——
- 检测到输入是英文单词/短语时，自动路由到词典服务，稳定返回 **英式/美式音标 + 词性词义 + 英/美发音**；
- 原文或译文包含中文时，自动生成 **拼音标注**（带声调）；
- 句子翻译走多家翻译服务并行/多开，与 Bob 对齐。

## 功能规划

| 功能 | Bob | lucas-translate |
|---|---|---|
| 划词 / 输入 / 截图翻译 | ✅ | ✅ 计划 |
| 静默划词 / 输入框替换 | ✅ | ✅ 计划 |
| 截图 OCR / 静默 OCR / 连续识别 | ✅ | ✅ 计划 |
| 多翻译服务同时展示 | ✅ 最多 12 个 | ✅ 计划 |
| **英文单词音标（英/美）** | ⚠️ 仅词典服务 | ✅ **词典层内置，永远有** |
| **中文拼音标注（带声调）** | ❌ | ✅ **内置** |
| TTS 发音 | ✅ | ✅ 词典直发 |
| 多平台 | ❌ 仅 macOS | ✅ Win / Linux / macOS |
| 插件系统 | ✅ JavaScriptCore | ✅ 计划（内嵌 JS 运行时） |
| 开源 | ❌ | ✅ MIT |

## 快速开始

```bash
# 核心库单元/集成测试（纯 Rust，无 UI 依赖）
cd crates/lucas-core && cargo test

# 运行桌面应用（需要安装 tauri-cli）
cargo install tauri-cli --version "^2"
cd app && cargo tauri dev
```

## 项目结构

```
lucas-translate/
├── crates/lucas-core/     # 纯 Rust 核心库：服务抽象、词典/翻译/拼音、语言检测
│   └── src/
│       ├── services/      #   翻译与词典服务实现（有道词典、Google 免费等）
│       ├── pinyin.rs      #   拼音生成（带声调）
│       ├── lang.rs        #   语言代码与检测
│       └── tts.rs         #   发音 URL 构造
├── app/                   # Tauri 2 桌面应用（Rust 后端 + Web 前端）
│   ├── src-tauri/         #   全局快捷键、窗口、命令桥接
│   └── ui/                #   翻译窗口 UI（原生 HTML/CSS/JS，零构建）
└── docs/
    ├── ARCHITECTURE.md    # 三平台技术方案（OCR/划词/截图/插件系统）
    └── SERVICES.md        # 翻译服务调研（Bob 用了什么、我们用什么）
```

## Roadmap

- [x] 核心库：词典（音标）+ 翻译 + 拼音
- [x] Tauri 应用骨架 + 翻译窗口 UI
- [x] 划词翻译（macOS：AX 取词优先 + Cmd+C 模拟兑底，剪贴板自动恢复）
- [x] 截图 OCR（macOS：选区覆盖层 + screencapture + Apple Vision 离线识别 + 智能分段）
- [x] 静默截图 OCR（识别结果直接进剪贴板）
- [x] DeepL 免 Key 通道（oneshot-free 端点，实测可用）
- [x] 多服务并行多开（tokio/scoped threads 并发，多卡片展示）
- [x] 历史记录 & 收藏夹（SQLite/WAL，主窗口面板，一键重译/收藏）
- [x] AI 翻译服务（OpenAI 兼容：DeepSeek/Kimi/智谱/Ollama 本地，设置面板配置）
- [x] 系统托盘 + 偏好设置（权限检测、一键跳系统设置、开机自启、关闭进后台）
- [x] Bob 式窗口行为（失焦自动隐藏、⌥D/⌥A/⌥S/⌥C 快捷键）
- [x] Windows / Linux 划词/截图/OCR 实现（SendInput+WinRT OCR；xclip+xdotool+tesseract，已过交叉编译检查，真机待验证）
- [ ] 多服务开关与拖拽排序
- [ ] 流式输出（LLM SSE）
- [ ] 插件系统（内嵌 JS 运行时，兼容 Bob 插件格式是远期目标）
- [ ] Linux (AppImage/deb) / Windows (msi) / macOS (dmg) 打包发布

## macOS 权限说明

首次使用需要授权（应用只提示一次）：
- **辅助功能**（划词翻译）：系统设置 → 隐私与安全性 → 辅助功能
- **屏幕录制**（截图 OCR）：系统设置 → 隐私与安全性 → 屏幕录制

调试工具：

```bash
cd app/src-tauri
cargo run --example ocr_test /path/to/image.png   # 单独测试 Vision OCR
```

## License

MIT
