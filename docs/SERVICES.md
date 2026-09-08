# 翻译服务调研

## Bob 用了什么（逆向 + 官网文档结论）

### Bob 内置免费服务
| 服务 | 说明 |
|---|---|
| 系统翻译 | macOS Translation framework（12.3.1+），免费 |
| 金山词霸 | 查词/词典卡片，免费不保证稳定 |
| 简明英汉词典 | 离线内置 |
| 智谱 GLM-4-Flash | 内置免费 LLM 翻译 |
| 硅基流动 Qwen2.5-7B 等 | 内置免费 LLM 翻译 |
| 离线文本识别 | Apple Vision（macOS 11+） |
| 离线语音合成 | AVSpeechSynthesizer |

### Bob 需要用户自申密钥的服务
火山 / 腾讯 / 阿里 / 百度 / 有道 / 彩云 / 小牛 / Google / Microsoft / Amazon / DeepL / OpenAI / Azure OpenAI / Gemini / Groq / Ollama / DeepSeek / 通义 / 文心 / 豆包 / 混元 / 零一 / Kimi（翻译）；对应的 OCR 与 TTS 厂商各若干。

### Bob 音标差的根因
- Bob 翻译窗口的词典卡片（`toDict`）只有在**词典类服务**（金山词霸、简明英汉词典）下才带音标；
- 用户日常选择的翻译服务（DeepL/OpenAI/火山等）返回的只有译文，没有音标；
- 结果：同一个单词，换一个服务音标就没了，体验割裂。

## lucas-translate 的选择

### 第一梯队（免密钥，默认启用）
| 服务 | 类型 | 通道 | 说明 |
|---|---|---|---|
| 有道词典 | 词典+翻译 | `dict.youdao.com/jsonapi_s`（POST, keyfrom=webdict） | 单词返回英/美音标；整句也能翻 |
| **DeepL 免费** | 翻译 | `oneshot-free.www.deepl.com/v1/translate` | 免费网页通道，可能限流或变更，不承诺可用性 |
| **DeepL 官方 API** | 翻译 | `api-free.deepl.com/v2/translate` / `api.deepl.com/v2/translate` | 可选，默认关闭；在设置中配置 Free/Pro 账户和密钥 |
| Bing | 翻译 | 网页翻译接口 | 免费通道，动态获取会话信息 |
| OpenAI 兼容 / Ollama | 翻译 | 用户自填 endpoint + model | 已支持；密钥使用系统凭据存储 |
| Google 免费端点 | 翻译 | `translate.googleapis.com/translate_a/single?client=gtx` | 降级通道；注意反爬（Sorry 页） |
| 有道发音 | TTS | `dict.youdao.com/dictvoice?type=1/2` | 英式 type=1，美式 type=2，直接可播 |

**桌面端并行返回**：只请求已启用的渠道，失败仅影响该卡片，不自动重复发送或切换付费服务。共享 HTTP 连接池，取消会终止网络等待；成功结果按文本、语向、服务和配置隔离，内存缓存有效期 5 分钟，上限 128 条 / 约 2 MiB 载荷，不缓存失败。429 等可重试错误采用按渠道冷却，避免重试风暴。

DeepL 官方连接测试仅调用 [`GET /v2/usage`](https://developers.deepl.com/api-reference/usage-and-quota/check-usage-and-limits)，不发送用户文字。官方 API 的额度与 SLA 由服务商和账户方案决定，并非无限免费或绝对可靠。

### 第二梯队（规划，免密钥/自建）
| 服务 | 说明 |
|---|---|
| DeepLX | 社区 DeepL 免费方案，可自建 endpoint |
| macOS 系统翻译 | Translation framework，免密钥 |

### 第三梯队（规划，用户自申密钥）
火山 / 腾讯 / 百度 / 阿里 / 有道开放平台 等，设计上通过统一 `TranslateService` trait 接入，UI 展示顺序可拖拽排序（对齐 Bob）。

## 拼音方案

- 依赖 `rust-pinyin`（pinyin crate 0.10），字级转换带声调（`nǐ hǎo`）；
- 触发条件：原文或译文包含汉字（统一简繁处理）；
- 展示：翻译窗口中的高亮拼音行；
- 规划增强：多音字词组校正（rust-pinyin 支持词组拼音 `to_pinyin_multi`）。

## 划词策略（参考 Easydict + 增强）

1. **AX 取词优先**：系统级 `AXSelectedText` → 焦点元素（`AXFocusedUIElement` 属性）→ `AXSelectedText`
   - 不污染剪贴板、速度快；⚠️ macOS 26 SDK 已移除 `AXUIElementCopyFocusedUIElement` 函数，必须用属性方式
   - HIServices 子框架需在 build.rs 加 framework 搜索路径
2. **Cmd+C 模拟兑底**：清剪贴板 → 模拟按键 → 读剪贴板 → 延迟恢复原剪贴板

## 合规提醒

免费 web 端点（有道 jsonapi_s、Google gtx）是非官方接口，仅供个人学习研究；开源发布时应：
1. 在 README 明确说明数据来源与用途；
2. 不内置任何破解/逆向的付费 API；
3. 鼓励用户配置自己的密钥或本地模型。
