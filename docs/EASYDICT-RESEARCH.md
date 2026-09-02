# Easydict 调研报告（2026-09）

> 仓库: https://github.com/tisfeng/Easydict · 14.4k stars · Swift · GPL-3.0 · 仅 macOS
> 结论先行：Easydict 的核心竞争力 = **"免 Key 优先"的渠道策略**（逆向 Web 端点 + 系统能力 + 作者内置加密 Key），把 20+ 服务全部做成开箱即用。它的短板是平台锁定、逆向端点维护成本、无插件生态。我们可以在这些点上做优。

---

## 1. 渠道是怎么搞的（三类机制）

### 类型 A：逆向免费端点（无 Key，主力）
| 服务 | 实现方式 | 代码证据 |
|---|---|---|
| 有道 | `fanyideskweb/webfanyi` 网页版协议，**动态抓取密钥**（`getYoudaoKey` 请求有道 web JS 拿 aesKey/secretKey，签名 MD5，响应 AES 解密） | `YoudaoService+Translate.swift` |
| DeepL | 逆向 DeepL **iOS 客户端**的 `oneshot-free.www.deepl.com/v1/translate` 匿名端点，伪造 appVersion/instanceID —— **免 Key 白嫖 DeepL**，注释里还引用了 DLX 社区的 issue | `DeepLService+Translate.swift` |
| Google | Web HTML/JSON 请求 + 伪造 Chrome UA | `GoogleService+Translate.swift` |
| Bing / 百度 | 同样是 Web 端点逆向 | `BingService+Translate.swift` 等 |

### 类型 B：系统能力（免费，稳定）
- 🍎 **Apple 词典**：直接查 macOS 词典.app（DCS 词典服务）
- 🍎 **Apple 翻译**：macOS 15 `Translation` framework 的 `TranslationSession`（官方 API，免费离线/系统在线）
- 🍎 **Apple OCR**：Vision 框架 + **自研段落合并算法**（`OCRMergeStrategy`、`IndentationInfo`、行高聚类，对标 Bob 的智能分段）
- 🍎 **Apple TTS**：AVSpeechSynthesizer

### 类型 C：内置 Key + 用户 Key
- **内置 Key 机制**（关键发现）：`EncryptedSecretKeys.plist`（AES 加密）打包进 App，运行时解密——作者把自己申请的 **OpenAI / Gemini / 彩云 / 小牛 / BuiltInAI 自建代理** 的 Key 内置给所有用户试用，源码注释原话："For convenience, we provide a default key for users to try out. **Please do not abuse it, otherwise it may be revoked.**"
- 用户自填 Key：火山/腾讯/阿里/百度/牛递/DeepL 官方 API、OpenAI 兼容端点、Ollama 本地
- 有趣的细节：还有 `ClaudeCodeService` / `CodexCLIService` —— 把本地 coding agent CLI 当翻译引擎用

## 2. 核心竞争力总结

1. **开箱即用的渠道广度**：20+ 服务全部默认可用，不需要用户申请任何东西（免 Key 逆向 + 系统 API + 内置 Key 三板斧）
2. **划词体验打磨深**：鼠标自动选词（AXUIElement 取 `kAXSelectedText`）+ 强制复制 fallback + 全屏/简易/沉浸多窗口模式
3. **LLM 集成深度**：流式输出、模型/温度/Prompt 可配、多 Provider 统一抽象
4. **OCR 段落重建算法**（和 Bob 同思路，开源可参考思想）
5. GPL-3.0 开源 + 单人长期维护的社区信任

## 3. 缺点 / 短板

| # | 短板 | 说明 |
|---|---|---|
| 1 | **仅 macOS** | Swift 绑死平台，14k star 的需求里 Windows/Linux 是空白 |
| 2 | **逆向端点极脆弱** | DeepL 端点已多次更换（代码注释自证）；有道动态密钥机制复杂，网页改版就要修；Google 反爬波动 |
| 3 | **内置 Key 的风险** | 全体用户共享作者配额：滥用即失效（批量故障）；且大概率违反各服务 ToS |
| 4 | **无插件系统** | 生态封闭，扩展要改源码发版（对比 Bob 的 50+ 社区插件） |
| 5 | **GPL-3.0** | 传染性协议——我们可以**参考思路但绝不能抄代码**，否则整个项目被迫 GPL |
| 6 | 技术债 | ObjC/Swift 混编、单人维护（README 自述"很忙，周末才处理 issue"） |
| 7 | 没有音标/拼音强化 | 词典结果依赖具体服务，无"永远有音标"的保障层，无拼音 |

## 4. 我们能做得更优的地方

### 保留的差异化（已实现）
- 词典层/翻译层分离 → 音标永远有 + 发音
- 中文自动拼音标注
- 跨平台架构（Rust/Tauri：Win/Linux 是 Easydict 完全没有的市场）
- Bob 插件兼容是远期目标（Easydict 完全没有）

### 值得借鉴的（按优先级排进 Roadmap）
| 借鉴点 | 做法 | 优先级 |
|---|---|---|
| **DeepL 免 Key 端点** | 参考公开社区方案（DLX 等）实现 `deepl_free` 服务——高价值，DeepL 质量顶级 | 高 |
| **有道动态密钥** | 我们现在用 jsonapi_s（更简单稳定）；若失效，借鉴其 getYoudaoKey 动态抓取方案 | 备用 |
| **Bing Web 端点** | 微软系相对稳定，免 Key，作为第三降级通道 | 中 |
| **Apple 翻译系统通道** | macOS 15+ Translation framework，免 Key 离线可用 | 中 |
| **多服务并发查询** | tokio `join!` 并发多服务，UI 多列展示（对齐 Bob 12 服务多开） | 高 |
| **OCR 段落合并算法** | 行高/缩进聚类思路，实现我们自己的 `merge.rs`（Apple Vision 原始输出带几何信息可做） | 中 |
| **划词多策略 fallback** | AX 取词优先 → 失败再模拟 Cmd+C（我们目前只有后者） | 高 |

### 明确不学的
- **内置共享 Key**：合规风险 + 运营负担 + 单点故障。我们走"免 Key 逆向端点（少量、精选）+ 用户自带 Key（全覆盖）"，干净且可持续
- GPL 代码零复制，只学思想
