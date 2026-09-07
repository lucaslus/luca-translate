# lucas-translate

> 一款开源、跨平台（macOS / Windows / Linux）的翻译 & OCR 工具，目标是做 Bob 的平替，并在其基础上做得更好。

## 为什么做这个

[Bob](https://bobtranslate.com/) 是 macOS 上优秀的翻译/OCR 软件，但它有两个问题：

1. **只有 macOS 版**，Windows / Linux 用户没有选择；
2. **音标支持很差**：Bob 的音标只存在于金山词霸等词典通道，走普通翻译服务（火山/腾讯/DeepL/OpenAI 等）时完全没有音标，查单词体验割裂。

lucas-translate 的核心设计差异：**词典层与翻译层分离** ——
- 启用有道且查询英文单词/短语的中文释义时，展示 **英式/美式音标 + 词性词义 + 英/美发音**（以渠道返回为准）；关闭的渠道不会被偷偷调用；
- 原文或译文包含中文时，自动生成 **拼音标注**（带声调）；
- 句子翻译走多家翻译服务并行/多开，与 Bob 对齐。

## 功能规划

| 功能 | Bob | lucas-translate |
|---|---|---|
| 划词 / 输入 / 截图翻译 | ✅ | ✅ 计划 |
| 静默划词 / 输入框替换 | ✅ | ✅ 计划 |
| 截图 OCR / 静默 OCR / 连续识别 | ✅ | ✅ 计划 |
| 多翻译服务同时展示 | ✅ 最多 12 个 | ✅ 已实现 |
| **英文单词音标（英/美）** | ⚠️ 仅词典服务 | ✅ 内置词典渠道，可独立关闭 |
| **中文拼音标注（带声调）** | ❌ | ✅ **内置** |
| TTS 发音 | ✅ | ✅ 词典直发 |
| 多平台 | ❌ 仅 macOS | ✅ Win / Linux / macOS |
| 插件系统 | ✅ JavaScriptCore | ✅ 计划（内嵌 JS 运行时） |
| 开源 | ❌ | ✅ MIT |

## 快速开始

渠道限流、错误恢复、本地日志入口及 UI 回归说明见 [错误处理与诊断](docs/ERROR-RECOVERY.md)。

```bash
# 核心库单元/集成测试（纯 Rust，无 UI 依赖）
cargo test --manifest-path crates/lucas-core/Cargo.toml -- --skip network

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
- [x] Windows / Linux 划词/截图/OCR 实现（SendInput+WinRT OCR；X11 PRIMARY+xclip+tesseract，平台模块已过交叉编译检查，真机待验证）
- [x] 多服务开关、单渠道重试、取消等待、错误隔离
- [ ] 服务拖拽排序
- [ ] 流式输出（LLM SSE）
- [ ] 插件系统（内嵌 JS 运行时，兼容 Bob 插件格式是远期目标）
- [ ] Linux (AppImage/deb) / Windows (msi) / macOS (dmg) 打包发布

## macOS 权限说明

首次使用需要授权：
- **辅助功能**（划词翻译）：系统设置 → 隐私与安全性 → 辅助功能
- **屏幕录制**（截图 OCR）：系统设置 → 隐私与安全性 → 屏幕录制

调试工具：

```bash
cd app/src-tauri
cargo run --example ocr_test /path/to/image.png   # 单独测试 Vision OCR
```

授权后若 macOS 提示重启，请彻底退出再从 `/Applications/lucas-translate.app` 打开。
本地更新应保持 Bundle ID 和代码签名身份一致；不要每次更新都重置 TCC。
系统开关已开启但应用报告无权限时，应先检查运行的应用路径、签名和权限调用，
只有确认签名身份变更造成授权失效后，才考虑移除本应用的旧授权条目并重新授权。

macOS 权限检查依赖 `core-graphics >= 0.25`：0.24 错把 C `bool` 声明为
`boolean_t`，会在 Intel Mac 上误判已授权状态，见
[上游修复 #698](https://github.com/servo/core-foundation-rs/pull/698)。
`cargo test --test screen_capture_abi` 用模拟返回值覆盖这一问题，不触发真实授权。
截图流程向标准错误输出权限状态和截图/OCR 耗时，不记录图片或识别文本。

已安装应用还支持 `--diagnose-ocr x,y,w,h`：仅对指定矩形连续执行三次本地
截图/OCR，输出权限、耗时和文字数量，不请求权限或发送翻译请求。
启动方法和实测记录见 [OCR 权限修复与验证](docs/OCR-PERMISSION-FIX.md)。
不要直接运行命令行构建来推断已安装应用的 TCC 权限。

## 数据与交互边界

截图图片仅用于本机 OCR，不上传。截图翻译会把识别文字发送给**已启用**的翻译渠道；静默 OCR 只复制文字，不调用翻译。静默结果通过系统通知反馈，不抢焦点；请允许应用通知以看到提醒。

API Key 保存在系统凭据存储（macOS Keychain / Windows Credential Manager / Linux Secret Service），不返回给翻译窗口。首次加载会尝试安全迁移旧 JSON 明文密钥；迁移失败会保留原文件并报错。设置中密钥留空表示保留，移除需明确确认并保存。

Linux 划词目前使用 X11 PRIMARY selection，无法读取时不会通过模拟复制破坏剪贴板；Wayland 截图和划词兼容性仍需真机验证。免费网页翻译端点可用性受服务商及网络影响。

## 回归测试

```bash
npm ci --ignore-scripts
npm run lint
npm test
npx playwright install chromium
npm run test:ui
cargo test --manifest-path crates/lucas-core/Cargo.toml -- --skip network
cargo test --manifest-path app/src-tauri/Cargo.toml
```

前端回归使用真实 HTML/CSS/JS 和 CSP、模拟 IPC 与合成数据，不会读取真实历史记录、凭据或调用翻译渠道。macOS/Windows/Linux 完整构建已加入 CI；本地平台模块检查不能代替三平台真机测试。
本轮修复、性能边界与验证记录见 [完整 review 修复说明](docs/REVIEW-FIXES-2026-09-04.md)。

## License

MIT
