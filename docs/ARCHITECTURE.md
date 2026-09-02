# lucas-translate 架构设计

## 总体架构

```
┌─────────────────────────────────────────────────────┐
│                UI 层（WebView，零构建）                │
│   翻译窗口 / OCR 窗口 / 偏好设置 / 历史记录             │
├─────────────────────────────────────────────────────┤
│                Tauri 2 桥接层（Rust）                 │
│   全局快捷键 · 窗口管理 · 系统集成 · 命令(translate…)   │
├─────────────────────────────────────────────────────┤
│                lucas-core（纯 Rust 核心库）           │
│   服务路由 ─ 词典层(音标) ─ 翻译层 ─ 拼音 ─ 语言检测    │
│   OCR 服务抽象 ─ TTS 服务抽象 ─ 插件运行时(规划)        │
└─────────────────────────────────────────────────────┘
```

### 核心差异设计：词典层与翻译层分离

Bob 的问题：音标只有词典类服务返回，普通翻译通道没有，导致查词体验割裂。

lucas-translate 的 `route()` 函数（`crates/lucas-core/src/services/mod.rs`）：

1. 输入是英文单词/短语（≤3 词）→ **强制路由到词典服务**，返回英/美音标 + 词性词义 + 发音 URL；
2. 输入是句子/段落 → 走翻译服务链，逐个降级重试；
3. 原文或译文含中文 → 自动生成带声调拼音行（无论哪个服务的结果）。

## 三平台能力矩阵

| 能力 | macOS | Windows | Linux |
|---|---|---|---|
| 全局快捷键 | Tauri global-shortcut (Carbon) | RegisterHotKey | X11 GrabKey / Wayland 桌面 IPC |
| 获取选中文本（划词） | ✅ AX 取词优先 → Cmd+C 兑底 | ✅ SendInput 模拟 Ctrl+C + 剪贴板恢复 | ✅ xclip 读 PRIMARY → xdotool Ctrl+C 兑底 |
| 截图 | ✅ screencapture CLI | ✅ screenshots crate (GDI) | ✅ screenshots crate（X11；Wayland 视桌面环境） |
| OCR | ✅ Apple Vision 离线 | ✅ Windows.Media.Ocr（WinRT，中文优先） | ✅ tesseract CLI（eng+chi_sim，回退默认） |
| TTS | AVSpeechSynthesizer 离线 | Windows SAPI（待接） | speech-dispatcher（待接） |
| 权限引导 | 辅助功能 + 屏幕录制（检测+系统弹窗+跳转设置） | 无需 | 无需 |
| 打包 | .dmg / App Store | msi / nsis | AppImage / deb / rpm |

> Windows / Linux 实现已通过交叉编译检查（`crates/platform-check`），真机运行测试待验证。

> 平台相关代码统一放在 `app/src-tauri/src/platform/`（规划目录），编译期按 `cfg(target_os)` 选择实现，对核心库暴露统一 trait。

## 服务层设计

- `DictService` trait：单词词典，产出 `DictCard { us_phonetic, uk_phonetic, meanings, uk_speech, us_speech }`
- `TranslateService` trait：文本翻译，产出分段 `Vec<String>`
- `route()`：自动路由 + 失败降级 + 拼音注入
- 语言代码兼容 Bob 的代码体系（auto/zh-Hans/en/...），为未来兼容 Bob 插件生态做准备
- `auto` 目标语言在调用服务商之前必须解析成具体语言（中文→英，其他→简中）

## 内置服务（免密钥）

| 服务 | 类型 | 说明 |
|---|---|---|
| 有道词典 jsonapi_s | 词典 + 整句翻译 | 无需密钥；单词返回英/美音标；Bob 第三方插件同款通道 |
| Google translate_a/single (gtx) | 整句翻译 | 无需密钥；注意有反爬风险，仅作降级通道 |
| （规划）DeepLX | 翻译 | 社区方案，可自建 |
| （规划）Ollama / OpenAI 兼容 | LLM 翻译 | 用户自填 endpoint，流式输出 |
| （规划）系统翻译 | 翻译 | macOS Translation framework |
| （规划）离线 OCR | OCR | Apple Vision / Windows.Media.Ocr / Tesseract |

## OCR 管线（规划）

```
截图/选图 → 预处理(缩放/二值化,可选) → OCR 服务 → 原始文本块
  → 智能分段算法(参考 Bob: 依据行间距/缩进/字号聚类还原段落)
  → 二维码检测(zbar) → 结果窗口(自动复制/连续识别可配置)
```

## 插件系统（规划，远期）

- 形态对齐 Bob：`.lucasplugin` 本质为 zip，含 `info.json` + `main.js`
- 运行时：`rquickjs`（QuickJS 绑定）实现与 Bob 类似的 JS 沙箱
- 注入 API 对齐 Bob 的 `$` 命名（$http/$info/$option/$log/$data...），降低 Bob 插件迁移成本
- 翻译插件协议对齐：`supportLanguages()` / `translate(query, completion)`

## 目录结构

```
crates/lucas-core/src/
├── lib.rs            # QueryResult / Phonetic / DictMeaning 类型
├── lang.rs           # 语言检测与 Bob 风格语言代码
├── pinyin.rs         # 带声调拼音生成
├── tts.rs            # 发音 URL（有道 dictvoice）
└── services/
    ├── mod.rs        # trait 定义 + route() 核心路由
    ├── youdao_dict.rs
    └── google_free.rs
app/
├── src-tauri/        # Tauri 2 应用（快捷键/窗口/命令）
└── ui/               # 翻译窗口（原生 HTML/CSS/JS）
```
