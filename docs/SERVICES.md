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
| 有道词典 | 英语查词+多语种句子翻译 | `dict.youdao.com/jsonapi_s`（POST, keyfrom=webdict，附网页签名） | 英语单词返回英/美音标；支持简体中文与英、西、法、德、日、韩、俄语的句子互译 |
| **DeepL 免费** | 翻译 | `oneshot-free.www.deepl.com/v1/translate` | 免费网页通道，可能限流或变更，不承诺可用性 |
| **DeepL 官方 API** | 翻译 | `api-free.deepl.com/v2/translate` / `api.deepl.com/v2/translate` | 可选，默认关闭；在设置中配置 Free/Pro 账户和密钥 |
| Bing | 翻译 | 网页翻译接口 | 免费通道，动态获取会话信息 |
| OpenAI 兼容 / Ollama | 翻译 | 用户自填 endpoint + model | 已支持；密钥使用系统凭据存储 |
| Google 免费端点 | 翻译 | `translate.googleapis.com/translate_a/single?client=gtx` | 降级通道；注意反爬（Sorry 页） |
| 有道发音 | TTS | `dict.youdao.com/dictvoice?type=1/2` | 英式 type=1，美式 type=2，直接可播 |

有道请求需按[官方网页客户端](https://shared.ydstatic.com/market/souti/web_dict/online/3.1.0/dist/client/76bdd54.js)生成 `t` / `sign`（2026-09-17 核对）；`t` 使用 JavaScript 的 UTF-16 字符串长度。缺少签名时，`farewell` 和 `preps` 实测收到 HTTP 200，却返回其他单词，导致解析失败。现在词典和整句翻译均使用签名，并校验响应回显的原文，拒绝错配结果。两词均有真实接口回归测试；修复需重新构建、安装并重启应用后生效。公开网页签名参数可能随上游改版而变化。

同日排查西班牙语句子时，发现原实现将非中英语向在本地直接拦截，并固定 `le=en`。实测 `le` 指定与中文互译的外语：西译中和中译西均使用 `le=es`，其余已验证语种同理；固定英语参数会产生夹杂原文的错误翻译。现在按源/目标语言选择 `le`，并要求响应 `fanyi.type` 与请求语向一致。14 个句子翻译方向以及用户报告的西班牙语句子均有真实接口测试。当前尚不支持外语之间直接互译、繁体中文语向，以及非英语专用词典结构；不将英语释义当作其他语种的结果。

**桌面端并行返回**：只请求已启用的渠道，失败仅影响该卡片，不自动重复发送或切换付费服务。共享 HTTP 连接池，取消会终止网络等待；成功结果按文本、语向、服务和配置隔离，内存缓存有效期 5 分钟，上限 128 条 / 约 2 MiB 载荷，不缓存失败。429 等可重试错误采用按渠道冷却，避免重试风暴。

2026-09-30 实测修复：Bing 的 `www.bing.com/translator` 在本机网络重定向到 `cn.bing.com`，继续向 `www` 提交翻译会在重定向后收到 HTTP 200 空正文；现在缓存页面最终的 HTTPS Bing 域名，与会话参数一起用于翻译请求和 Referer，并保留 JSON 错误中的状态码以触发现有冷却逻辑；同类行为可交叉参考 [Readest 的地区域名修复](https://github.com/readest/readest/pull/5826)及 [bing-translate-api 的域名处理](https://github.com/plainheart/bing-translate-api/blob/master/src/index.js)。有道源语言采用原文校验通过后的 `meta.guessLanguage`（`eng` 归一化为 `en`），不使用表示词典选择的 `le` / `lang`；词典缺词后使用自动文本翻译时，从完整 SSE 的实际语向提取源语言，同时校验结束事件的语向、目标语言和请求 ID；查词、句子与分段翻译会将有效检测结果传给界面，避免成功后仍显示“待确认”，手动源语言保持用户选择；检测信息缺失或分段检测不完整时仍不宣称已经确认。上述变更需重新构建并重启应用后生效。

DeepL 官方连接测试仅调用 [`GET /v2/usage`](https://developers.deepl.com/api-reference/usage-and-quota/check-usage-and-limits)，不发送用户文字。官方 API 的额度与 SLA 由服务商和账户方案决定，并非无限免费或绝对可靠。

### 第二梯队（规划，免密钥/自建）
| 服务 | 说明 |
|---|---|
| DeepLX | 社区 DeepL 免费方案，可自建 endpoint |
| macOS 系统翻译 | Translation framework，免密钥 |

### 第三梯队（规划，用户自申密钥）
火山 / 腾讯 / 百度 / 阿里 / 有道开放平台 等，设计上通过统一 `TranslateService` trait 接入，UI 展示顺序可拖拽排序（对齐 Bob）。

## 拼音数据兼容

- 依赖 `rust-pinyin`（pinyin crate 0.10），字级转换带声调（`nǐ hǎo`）；
- 触发条件：原文或译文包含汉字（统一简繁处理）；
- 结果数据保留拼音字段，Omarchy、macOS、Windows 与普通 Linux 的界面均不展示拼音。

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
