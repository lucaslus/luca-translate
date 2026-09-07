//! 有道词典（web 端 jsonapi_s 接口）。
//!
//! 返回结构（实测）：
//! - `ec.word`: { usphone, ukphone, trs: [{pos, tran}], return-phrase, usspeech, ukspeech }
//! - `fanyi.tran`: 整句翻译时的译文字符串或段落数组
//!
//! 该接口即 Bob 第三方插件（如 Free 有道翻译）所用通道，无需密钥。

use serde_json::Value;

use super::{DictCard, DictService, ServiceError, TranslateService};

const API: &str = "https://dict.youdao.com/jsonapi_s?doctype=json&jsonversion=4";

pub struct YoudaoDict;

pub(super) struct DictionaryOutput {
    pub dict: Option<DictCard>,
    pub paragraphs: Vec<String>,
}

impl YoudaoDict {
    fn request_once(&self, text: &str) -> Result<Value, ServiceError> {
        let resp = ureq::post(API)
            .timeout(std::time::Duration::from_secs(15))
            // 浏览器 UA：降低被风控返回空结果的概率
            .set(
                "User-Agent",
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            )
            .set("Referer", "https://dict.youdao.com/")
            .send_form(&[("q", text), ("keyfrom", "webdict"), ("client", "web")])
            .map_err(ServiceError::from_http)?;
        resp.into_json::<Value>().map_err(ServiceError::from_body)
    }

    fn request(&self, text: &str) -> Result<Value, ServiceError> {
        // Empty/invalid responses are surfaced, never retried blindly.
        self.request_once(text)
    }

    pub(super) fn lookup_or_translate(&self, text: &str) -> Result<DictionaryOutput, ServiceError> {
        // Word-like phrases need not have a dictionary entry. Reuse the translation
        // in the same response instead of failing or issuing a second request.
        Self::parse_dictionary_output(&self.request(text)?, text)
    }

    fn parse_dictionary_output(data: &Value, text: &str) -> Result<DictionaryOutput, ServiceError> {
        if let Ok(dict) = Self::parse_dictionary(data, text) {
            return Ok(DictionaryOutput {
                paragraphs: dictionary_paragraphs(&dict),
                dict: Some(dict),
            });
        }
        translation_paragraphs(data)
            .map(|paragraphs| DictionaryOutput {
                dict: None,
                paragraphs,
            })
            .ok_or_else(missing_content)
    }
}

impl DictService for YoudaoDict {
    fn name(&self) -> &'static str {
        "YoudaoDict"
    }

    fn lookup(&self, text: &str) -> Result<DictCard, ServiceError> {
        Self::parse_dictionary(&self.request(text)?, text)
    }
}

impl YoudaoDict {
    fn parse_dictionary(data: &Value, text: &str) -> Result<DictCard, ServiceError> {
        let ec = data.pointer("/ec/word").ok_or_else(missing_content)?;

        let word = ec
            .get("return-phrase")
            .and_then(|v| v.get("l"))
            .and_then(|v| v.as_str())
            .or_else(|| ec.get("return-phrase").and_then(|v| v.as_str()))
            .unwrap_or(text)
            .to_string();

        let us_phonetic = ec
            .get("usphone")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| {
                // 兜底：simple.word[0]
                data.pointer("/simple/word/0/usphone")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            });
        let uk_phonetic = ec
            .get("ukphone")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| {
                data.pointer("/simple/word/0/ukphone")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            });

        let mut meanings = Vec::new();
        if let Some(trs) = ec.get("trs").and_then(|v| v.as_array()) {
            for tr in trs {
                let pos = tr
                    .get("pos")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let tran = tr
                    .get("tran")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if !tran.trim().is_empty() {
                    meanings.push((pos, tran));
                }
            }
        }

        if meanings.is_empty() {
            return Err(missing_content());
        }

        // 发音 URL（dictvoice 免费端点）
        let us_speech = us_phonetic
            .as_ref()
            .map(|_| crate::tts::youdao_voice_url(&word, false));
        let uk_speech = uk_phonetic
            .as_ref()
            .map(|_| crate::tts::youdao_voice_url(&word, true));

        Ok(DictCard {
            word,
            us_phonetic,
            uk_phonetic,
            meanings,
            uk_speech,
            us_speech,
            source: "youdao".into(),
        })
    }
}

impl TranslateService for YoudaoDict {
    fn name(&self) -> &'static str {
        "YoudaoDict"
    }

    fn translate(&self, text: &str, from: &str, to: &str) -> Result<Vec<String>, ServiceError> {
        // This anonymous endpoint cannot select arbitrary language pairs.
        let source = crate::lang::source_hint(text, from);
        if !matches!((source, to), ("en", "zh-Hans") | ("zh-Hans", "en")) {
            return Err(ServiceError::Unsupported(
                "有道免费通道仅支持中英互译，请使用其他渠道".into(),
            ));
        }
        let data = self.request(text)?;
        if let Some(paragraphs) = translation_paragraphs(&data) {
            return Ok(paragraphs);
        }
        // 单词兜底：拼词典释义
        Self::parse_dictionary(&data, text).map(|dict| dictionary_paragraphs(&dict))
    }
}

fn missing_content() -> ServiceError {
    ServiceError::Parse("有道响应中没有可用的词条或译文".into())
}

fn dictionary_paragraphs(dict: &DictCard) -> Vec<String> {
    dict.meanings
        .iter()
        .map(|(pos, meaning)| format!("{pos} {meaning}").trim().to_string())
        .collect()
}

fn translation_paragraphs(data: &Value) -> Option<Vec<String>> {
    let paragraphs: Vec<String> = match data.pointer("/fanyi/tran")? {
        Value::String(s) if !s.trim().is_empty() => vec![s.clone()],
        Value::Array(values) => values
            .iter()
            .filter_map(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string)
            .collect(),
        _ => return None,
    };
    (!paragraphs.is_empty()).then_some(paragraphs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unlisted_phrase_uses_translation_from_same_response() {
        let text = "Redemption code recharge";
        assert!(crate::lang::dictionary_eligible(text, "en", "zh-Hans"));
        // Response shape observed for the reported failure: fanyi, but no ec.
        let data = json!({"fanyi": {
            "input": text, "type": "en2zh-CHS", "tran": "兑换码充值"
        }});
        let output = YoudaoDict::parse_dictionary_output(&data, text).unwrap();
        assert_eq!(output.paragraphs, ["兑换码充值"]);
        assert!(output.dict.is_none());
    }

    #[test]
    fn useful_dictionary_keeps_priority_and_phonetics() {
        let data = json!({
            "ec": {"word": {
                "return-phrase": {"l": "hello"},
                "usphone": "həˈloʊ", "ukphone": "həˈləʊ",
                "trs": [{"pos": "int.", "tran": "你好"}, {"tran": "喂"}]
            }},
            "fanyi": {"tran": "您好"}
        });
        let output = YoudaoDict::parse_dictionary_output(&data, "hello").unwrap();
        assert_eq!(output.paragraphs, ["int. 你好", "喂"]);
        let dict = output.dict.unwrap();
        assert_eq!(dict.word, "hello");
        assert_eq!(dict.us_phonetic.as_deref(), Some("həˈloʊ"));
        assert_eq!(dict.uk_phonetic.as_deref(), Some("həˈləʊ"));
        assert!(dict.us_speech.is_some());
        assert!(dict.uk_speech.is_some());
    }

    #[test]
    fn empty_or_malformed_dictionary_does_not_hide_translation() {
        for word in [
            Value::Null,
            json!({}),
            json!({"trs": []}),
            json!({"trs": [{"tran": " \n"}, {"tran": 42}, {}]}),
            json!({"trs": "invalid"}),
        ] {
            let data = json!({"ec": {"word": word}, "fanyi": {"tran": "兑换码充值"}});
            let output =
                YoudaoDict::parse_dictionary_output(&data, "Redemption code recharge").unwrap();
            assert_eq!(output.paragraphs, ["兑换码充值"]);
            assert!(output.dict.is_none());
        }
    }

    #[test]
    fn translation_array_ignores_blank_and_non_text_entries() {
        let data = json!({"fanyi": {"tran": ["", " \n", null, 42, "第一段", "第二段"]}});
        let output = YoudaoDict::parse_dictionary_output(&data, "phrase").unwrap();
        assert_eq!(output.paragraphs, ["第一段", "第二段"]);
        assert!(output.dict.is_none());
    }

    #[test]
    fn missing_or_invalid_content_remains_an_error() {
        for data in [
            Value::Null,
            json!({}),
            json!({"errorCode": 50}),
            json!({"fanyi": {"tran": " \n"}}),
            json!({"fanyi": {"tran": [null, 42, " "]}}),
            json!({"ec": {"word": {"trs": []}}, "fanyi": {"tran": {}}}),
        ] {
            assert!(matches!(
                YoudaoDict::parse_dictionary_output(&data, "phrase"),
                Err(ServiceError::Parse(_))
            ));
        }
    }
}
