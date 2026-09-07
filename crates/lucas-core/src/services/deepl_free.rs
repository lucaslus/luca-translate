//! DeepL 免费匿名端点（oneshot-free）。
//!
//! 参考 DLX 社区（OwO-Network/DLX）公开研究的 DeepL 交互式客户端协议，
//! 模拟 iOS 客户端的匿名一次性翻译请求，返回结构与官方 API 一致。
//! 无需密钥；仅供个人学习研究使用。

use serde::Serialize;
use serde_json::Value;

use super::{ServiceError, TranslateService, TranslationOutput};

const API: &str = "https://oneshot-free.www.deepl.com/v1/translate";
const USER_AGENT: &str = "DeepL/26.42 CFNetwork/3826.600.41 Darwin/25.0.0";
const OS_VERSION: &str = "26.0";
const APP_VERSION: &str = "26.42";
const APP_BUILD: &str = "5443737";

pub struct DeepLFree {
    instance_id: String,
    session_id: String,
}

impl Default for DeepLFree {
    fn default() -> Self {
        Self::new()
    }
}

impl DeepLFree {
    pub fn new() -> Self {
        Self {
            instance_id: uuid_v4_lower(),
            session_id: uuid_v4_lower(),
        }
    }

    fn request_translation(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<TranslationOutput, ServiceError> {
        if to == "auto" {
            return Err(ServiceError::Unsupported(
                "目标语言不能是 auto（应由路由层解析）".into(),
            ));
        }
        let source_lang = if from == "auto" {
            None
        } else {
            Some(oneshot_lang(from, false))
        };

        let body = OneshotRequest {
            text: vec![text],
            target_lang: oneshot_lang(to, true),
            source_lang,
            usage_type: "translate",
            app_information: AppInformation {
                os: "iOS",
                os_version: OS_VERSION,
                app_version: APP_VERSION,
                app_build: APP_BUILD,
                instance_id: &self.instance_id,
            },
        };

        let resp = ureq::post(API)
            .timeout(std::time::Duration::from_secs(20))
            .set("Content-Type", "application/json")
            .set("Authorization", "None")
            .set("User-Agent", USER_AGENT)
            .set("x-app-os-version", OS_VERSION)
            .set("x-app-instance-id", &self.instance_id)
            .set("x-app-session-id", &self.session_id)
            .send_json(&body)
            .map_err(ServiceError::from_http)?;

        let data: Value = resp.into_json().map_err(ServiceError::from_body)?;
        parse_translation(&data)
    }
}

/// 轻量级 UUID v4（避免引入 uuid 依赖链）
fn uuid_v4_lower() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64 + d.as_secs() * 1_000_000_000)
        .unwrap_or(0);
    let pid = std::process::id() as u64;
    let addr = &nanos as *const u64 as u64;

    let mut state = nanos ^ (pid << 32) ^ addr ^ 0x9E3779B97F4A7C15;
    let mut next = move || {
        // xorshift64*
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        state.wrapping_mul(0x2545F4914F6CDD1D)
    };

    let mut bytes = [0u8; 16];
    for chunk in bytes.chunks_mut(8) {
        let v = next().to_le_bytes();
        let n = chunk.len();
        chunk.copy_from_slice(&v[..n]);
    }
    bytes[6] = (bytes[6] & 0x0F) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3F) | 0x80; // variant

    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

#[derive(Serialize)]
struct OneshotRequest<'a> {
    text: Vec<&'a str>,
    target_lang: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_lang: Option<String>,
    usage_type: &'static str,
    app_information: AppInformation<'a>,
}

#[derive(Serialize)]
struct AppInformation<'a> {
    os: &'static str,
    os_version: &'static str,
    app_version: &'static str,
    app_build: &'static str,
    instance_id: &'a str,
}

/// Bob 语言代码 -> oneshot 端点的 BCP-47 风格代码
fn oneshot_lang(code: &str, is_target: bool) -> String {
    match code.to_lowercase().as_str() {
        "zh-hans" => "zh-Hans".into(),
        "zh-hant" => "zh-Hant".into(),
        "pt-pt" => "pt-PT".into(),
        "pt-br" => "pt-BR".into(),
        // 目标为英文时必须给具体变体
        "en" if is_target => "en-US".into(),
        other => other.into(),
    }
}

fn parse_translation(data: &Value) -> Result<TranslationOutput, ServiceError> {
    let translated = data
        .pointer("/translations/0/text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ServiceError::Parse("响应缺少 translations 字段".into()))?;
    if translated.trim().is_empty() {
        return Err(ServiceError::Parse("译文为空".into()));
    }
    Ok(TranslationOutput {
        paragraphs: translated.split('\n').map(str::to_string).collect(),
        detected_from: data
            .pointer("/translations/0/detected_source_language")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    })
}

impl TranslateService for DeepLFree {
    fn name(&self) -> &'static str {
        "DeepLFree"
    }

    fn translate(&self, text: &str, from: &str, to: &str) -> Result<Vec<String>, ServiceError> {
        self.request_translation(text, from, to)
            .map(|output| output.paragraphs)
    }

    fn translate_with_detection(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<TranslationOutput, ServiceError> {
        self.request_translation(text, from, to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oneshot_lang() {
        assert_eq!(oneshot_lang("zh-Hans", true), "zh-Hans");
        assert_eq!(oneshot_lang("en", true), "en-US");
        assert_eq!(oneshot_lang("en", false), "en");
        assert_eq!(oneshot_lang("ja", false), "ja");
    }

    #[test]
    fn response_includes_deepl_detected_language() {
        let data = serde_json::json!({
            "translations": [{"detected_source_language": "EN", "text": "自主的"}]
        });
        let output = parse_translation(&data).unwrap();
        assert_eq!(output.paragraphs, ["自主的"]);
        assert_eq!(output.detected_from.as_deref(), Some("EN"));
    }

    #[test]
    fn network_deepl_free_translate() {
        let svc = DeepLFree::new();
        let result = svc
            .translate(
                "The quick brown fox jumps over the lazy dog",
                "auto",
                "zh-Hans",
            )
            .expect("DeepL 免费端点请求失败");
        let joined = result.join("");
        assert!(!joined.is_empty());
        assert!(
            joined.contains("狐") || joined.contains("狗"),
            "DeepL 译文异常: {joined}"
        );
        println!("DeepL result: {joined}");
    }
}
