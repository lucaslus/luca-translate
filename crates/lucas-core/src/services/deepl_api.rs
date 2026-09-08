//! Official DeepL API, separate from the anonymous web channel.
use super::{ServiceError, TranslateService, TranslationOutput};
use serde_json::{json, Value};

pub struct DeepLApi {
    pub api_key: String,
    pub pro: bool,
}
impl DeepLApi {
    fn endpoint(&self) -> &'static str {
        if self.pro {
            "https://api.deepl.com"
        } else {
            "https://api-free.deepl.com"
        }
    }
    /// Validate saved credentials without submitting user text or consuming translation quota.
    pub fn test_connection(&self) -> Result<(), ServiceError> {
        let data: Value = crate::http::get(&format!("{}/v2/usage", self.endpoint()))
            .timeout(std::time::Duration::from_secs(10))
            .set("Authorization", &format!("DeepL-Auth-Key {}", self.api_key))
            .call()?
            .into_json()?;
        if data.get("character_count").is_some() || data.get("products").is_some() {
            Ok(())
        } else {
            Err(ServiceError::Parse(String::new()))
        }
    }
}
fn code(language: &str, target: bool) -> Result<&str, ServiceError> {
    match language {
        "zh-Hans" => Ok(if target { "ZH-HANS" } else { "ZH" }),
        "zh-Hant" => Ok(if target { "ZH-HANT" } else { "ZH" }),
        "en" => Ok(if target { "EN-US" } else { "EN" }),
        "ja" => Ok("JA"),
        "ko" => Ok("KO"),
        "fr" => Ok("FR"),
        "de" => Ok("DE"),
        "ru" => Ok("RU"),
        "es" => Ok("ES"),
        _ => Err(ServiceError::Unsupported("DeepL API 不支持此语向".into())),
    }
}
fn parse(data: Value) -> Result<TranslationOutput, ServiceError> {
    let text = data
        .pointer("/translations/0/text")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| ServiceError::Parse(String::new()))?;
    Ok(TranslationOutput {
        paragraphs: text.lines().map(str::to_string).collect(),
        detected_from: data
            .pointer("/translations/0/detected_source_language")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}
impl TranslateService for DeepLApi {
    fn name(&self) -> &'static str {
        "DeepLApi"
    }
    fn cache_identity(&self) -> u64 {
        super::private_identity(&(self.name(), &self.api_key, self.pro))
    }
    fn translate(&self, text: &str, from: &str, to: &str) -> Result<Vec<String>, ServiceError> {
        self.translate_with_detection(text, from, to)
            .map(|r| r.paragraphs)
    }
    fn translate_with_detection(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<TranslationOutput, ServiceError> {
        if self.api_key.trim().is_empty() {
            return Err(ServiceError::Configuration("请配置 DeepL API Key".into()));
        }
        let mut body = json!({"text":[text], "target_lang":code(to, true)?});
        if from != "auto" {
            body["source_lang"] = json!(code(from, false)?);
        }
        let data = crate::http::post(&format!("{}/v2/translate", self.endpoint()))
            .timeout(std::time::Duration::from_secs(20))
            .set("Authorization", &format!("DeepL-Auth-Key {}", self.api_key))
            .send_json(&body)?
            .into_json()?;
        parse(data)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn official_api_languages_and_detection() {
        assert_eq!(code("zh-Hant", true).unwrap(), "ZH-HANT");
        assert_eq!(code("en", false).unwrap(), "EN");
        assert!(code("auto", true).is_err());
        let output =
            parse(json!({"translations":[{"text":"你好", "detected_source_language":"EN"}]}))
                .unwrap();
        assert_eq!(output.paragraphs, ["你好"]);
        assert_eq!(output.detected_from.as_deref(), Some("EN"));
        assert!(parse(json!({"translations":[{"text":" "}]})).is_err());
    }
    #[test]
    fn credentials_and_plan_partition_cache() {
        let first = DeepLApi {
            api_key: "one".into(),
            pro: false,
        };
        let second = DeepLApi {
            api_key: "two".into(),
            pro: false,
        };
        assert_ne!(first.cache_identity(), second.cache_identity());
        assert_eq!(first.endpoint(), "https://api-free.deepl.com");
    }
}
