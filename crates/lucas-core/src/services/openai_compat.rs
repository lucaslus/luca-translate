//! OpenAI 兼容 AI 翻译服务。
//!
//! 用户在设置面板填写 base_url / api_key / model 即可接入：
//! DeepSeek、Kimi、智谱、通义、Ollama（本地）、OpenAI 等一切 OpenAI 兼容端点。
//! 非流式实现（简单可靠），流式后续迭代。

use serde_json::{json, Value};

use super::{ServiceError, TranslateService};

pub struct OpenAiCompat {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

impl OpenAiCompat {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            model: model.into(),
        }
    }
}

/// Bob 风格语言代码 -> 英文描述（用于 prompt）
fn lang_desc(code: &str) -> &'static str {
    match code {
        "auto" => "the detected language",
        "zh-Hans" => "Simplified Chinese",
        "zh-Hant" => "Traditional Chinese",
        "en" => "English",
        "ja" => "Japanese",
        "ko" => "Korean",
        "fr" => "French",
        "de" => "German",
        "ru" => "Russian",
        "es" => "Spanish",
        _ => "the target language",
    }
}

impl TranslateService for OpenAiCompat {
    fn name(&self) -> &'static str {
        "AI"
    }

    fn translate(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<Vec<String>, ServiceError> {
        if to == "auto" {
            return Err(ServiceError::Unsupported("目标语言不能是 auto".into()));
        }
        if self.api_key.trim().is_empty() && !self.base_url.contains("localhost") && !self.base_url.contains("127.0.0.1") {
            // Ollama 等本地端点可以无 Key，云端必须填
            return Err(ServiceError::Unsupported("未配置 API Key".into()));
        }

        let system_prompt = format!(
            "You are a professional translation engine. Translate the user's text from {} to {}. \
             Reply with ONLY the translation, preserving original formatting and line breaks. \
             Do not add explanations, quotes, or any extra content.",
            lang_desc(from),
            lang_desc(to),
        );

        let body = json!({
            "model": self.model,
            "temperature": 0.2,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": text}
            ]
        });

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let mut req = ureq::post(&url)
            .timeout(std::time::Duration::from_secs(60))
            .set("Content-Type", "application/json");
        if !self.api_key.trim().is_empty() {
            req = req.set("Authorization", &format!("Bearer {}", self.api_key.trim()));
        }

        let resp = req
            .send_json(&body)
            .map_err(|e| ServiceError::Network(e.to_string()))?;
        let data: Value = resp
            .into_json()
            .map_err(|e| ServiceError::Parse(e.to_string()))?;

        let content = data
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ServiceError::Parse(format!(
                    "AI 响应格式异常: {}",
                    serde_json::to_string(&data).unwrap_or_default()
                ))
            })?;

        let trimmed_content = content.trim().trim_matches(|c| c == '"' || c == '\u{201c}' || c == '\u{201d}');
        if trimmed_content.is_empty() {
            return Err(ServiceError::Parse("AI 译文为空".into()));
        }

        Ok(trimmed_content
            .split('\n')
            .map(|s| s.to_string())
            .collect())
    }
}
