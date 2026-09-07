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
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
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

    fn translate(&self, text: &str, from: &str, to: &str) -> Result<Vec<String>, ServiceError> {
        if to == "auto" {
            return Err(ServiceError::Unsupported("目标语言不能是 auto".into()));
        }
        let endpoint = url::Url::parse(&self.base_url)
            .map_err(|_| ServiceError::Configuration("AI 地址无效".into()))?;
        let local = endpoint.host_str().is_some_and(|h| {
            h == "localhost"
                || h.trim_matches(['[', ']'])
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        });
        if !(endpoint.scheme() == "https" || endpoint.scheme() == "http" && local)
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
        {
            return Err(ServiceError::Configuration(
                "云端 AI 地址须使用 HTTPS，且不能包含凭据或查询参数".into(),
            ));
        }
        if self.api_key.trim().is_empty() && !local {
            // Ollama 等本地端点可以无 Key，云端必须填
            return Err(ServiceError::Configuration("未配置 API Key".into()));
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

        let resp = req.send_json(&body).map_err(ServiceError::from_http)?;
        let data: Value = resp.into_json().map_err(ServiceError::from_body)?;

        let content = data
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ServiceError::Parse("AI 响应未包含文字译文".into()))?;

        if content.trim().is_empty() {
            return Err(ServiceError::Parse("AI 译文为空".into()));
        }

        Ok(content.split('\n').map(|s| s.to_string()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reject_remote_http_and_fake_localhost_before_request() {
        for url in [
            "http://localhost.evil/v1",
            "https://example.invalid/localhost/v1",
            "http://example.invalid/127.0.0.1",
        ] {
            assert!(matches!(
                OpenAiCompat::new(url, "", "test").translate("hello", "en", "ja"),
                Err(ServiceError::Configuration(_))
            ));
        }
    }
}
