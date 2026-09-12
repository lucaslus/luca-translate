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
// This endpoint truncated 701/1200-character English inputs around character 600.
// A conservative UTF-8 byte budget also fits character/UTF-16 based limits.
const MAX_QUERY_BYTES: usize = 500;

pub struct YoudaoDict;

pub(super) struct DictionaryOutput {
    pub dict: Option<DictCard>,
    pub paragraphs: Vec<String>,
}

impl YoudaoDict {
    fn request_once(&self, text: &str) -> Result<Value, ServiceError> {
        let resp = crate::http::post(API)
            .timeout(std::time::Duration::from_secs(15))
            // 浏览器 UA：降低被风控返回空结果的概率
            .set(
                "User-Agent",
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            )
            .set("Referer", "https://dict.youdao.com/")
            .send_form(&[("q", text), ("keyfrom", "webdict"), ("client", "web")])
            ?;
        resp.into_json::<Value>()
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
        translation_paragraphs(data, text)?
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
        translate_with_request(text, to, |chunk| self.request(chunk))
    }
}

// Keep requests sequential: the HTTP layer retains the calling thread's
// cancellation token, and a failed/limited chunk stops all subsequent requests.
fn translate_with_request(
    text: &str,
    to: &str,
    mut request: impl FnMut(&str) -> Result<Value, ServiceError>,
) -> Result<Vec<String>, ServiceError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(missing_content());
    }
    let chunked = text.len() > MAX_QUERY_BYTES;
    let mut translated = String::new();
    let mut newlines = 0;
    for raw in translation_chunks(text) {
        let chunk = raw.trim();
        if chunk.is_empty() {
            newlines += raw.matches('\n').count();
            continue;
        }
        let data = request(chunk)?;
        // For split translations, every chunk must have a verifiable source.
        if chunked && translated_input(&data).is_none() {
            return Err(incomplete_translation());
        }
        let paragraphs = match translation_paragraphs(&data, chunk)? {
            Some(paragraphs) => paragraphs,
            // Preserve the existing word fallback for a single short query.
            None if !chunked => {
                return YoudaoDict::parse_dictionary(&data, chunk)
                    .map(|dict| dictionary_paragraphs(&dict));
            }
            None => return Err(missing_content()),
        };
        if !chunked {
            return Ok(paragraphs);
        }
        newlines += raw[..raw.len() - raw.trim_start().len()]
            .matches('\n')
            .count();
        if !translated.is_empty() {
            if newlines > 0 {
                translated.extend(std::iter::repeat_n('\n', newlines));
            } else if to == "en" {
                translated.push(' ');
            }
        }
        translated.push_str(paragraphs.join("\n").trim());
        newlines = raw[raw.trim_end().len()..].matches('\n').count();
    }
    Ok(vec![translated])
}

// Slices cover the entire input without losing punctuation, whitespace or UTF-8
// characters. Prefer a line break, then a sentence end, then a word boundary.
fn translation_chunks(mut text: &str) -> Vec<&str> {
    let mut chunks = Vec::new();
    while text.len() > MAX_QUERY_BYTES {
        let (mut line, mut sentence, mut word, mut limit) = (0, 0, 0, 0);
        let mut chars = text.char_indices().peekable();
        while let Some((index, c)) = chars.next() {
            let end = index + c.len_utf8();
            if end > MAX_QUERY_BYTES {
                break;
            }
            limit = end;
            if c == '\n' {
                line = end;
            }
            if c.is_whitespace() {
                word = end;
            }
            if matches!(c, '。' | '！' | '？' | '；')
                || (matches!(c, '.' | '!' | '?' | ';')
                    && chars.peek().is_none_or(|(_, next)| next.is_whitespace()))
            {
                sentence = end;
            }
        }
        let end = [line, sentence, word, limit]
            .into_iter()
            .find(|&end| end > 0)
            .expect("the byte budget fits every UTF-8 character");
        let (chunk, rest) = text.split_at(end);
        chunks.push(chunk);
        text = rest;
    }
    if !text.is_empty() {
        chunks.push(text);
    }
    chunks
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

fn incomplete_translation() -> ServiceError {
    ServiceError::Parse("有道未返回完整原文对应的译文".into())
}

fn translated_input(data: &Value) -> Option<&str> {
    ["/fanyi/input", "/input", "/meta/input"]
        .iter()
        .find_map(|path| data.pointer(path).and_then(Value::as_str))
}

fn translation_paragraphs(data: &Value, text: &str) -> Result<Option<Vec<String>>, ServiceError> {
    // The service can normalize spaces/newlines. All non-whitespace source
    // characters must still be present, in order; a nonempty tran is not enough.
    if let Some(input) = translated_input(data) {
        if !input
            .chars()
            .filter(|c| !c.is_whitespace())
            .eq(text.chars().filter(|c| !c.is_whitespace()))
        {
            return Err(incomplete_translation());
        }
    }
    let paragraphs: Vec<String> = match data.pointer("/fanyi/tran") {
        Some(Value::String(s)) if !s.trim().is_empty() => vec![s.clone()],
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string)
            .collect(),
        _ => return Ok(None),
    };
    Ok((!paragraphs.is_empty()).then_some(paragraphs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn echo(text: &str) -> Value {
        json!({"fanyi": {"input": text, "tran": text}})
    }

    fn without_whitespace(text: &str) -> String {
        text.chars().filter(|c| !c.is_whitespace()).collect()
    }

    #[test]
    fn chunks_cover_boundaries_and_unicode_without_losing_source() {
        for text in [
            String::new(),
            "a".repeat(499),
            "a".repeat(500),
            "a".repeat(501),
            "a".repeat(1200),
            "长句没有标点也不能丢失字符🦀café e\u{301}".repeat(70),
            "中文句子。English sentence!\r\n\n".repeat(80),
            " \n".repeat(600),
        ] {
            let chunks = translation_chunks(&text);
            assert_eq!(chunks.concat(), text);
            assert!(chunks
                .iter()
                .all(|s| !s.is_empty() && s.len() <= MAX_QUERY_BYTES));
        }
    }

    #[test]
    fn chunks_prefer_lines_then_sentences_then_words() {
        let prefix = "word ".repeat(70);
        for boundary in ["\n", ". ", "。"] {
            let text = format!("{prefix}end{boundary}{}", "next ".repeat(50));
            let chunks = translation_chunks(&text);
            assert_eq!(
                chunks[0].trim_end(),
                format!("{prefix}end{boundary}").trim_end()
            );
        }
        let text = "unbrokenwords ".repeat(100);
        let chunks = translation_chunks(&text);
        assert!(chunks[..chunks.len() - 1].iter().all(|s| s.ends_with(' ')));
        // A decimal point is not a sentence boundary.
        let text = format!("{}3.14 {}", "word ".repeat(70), "next ".repeat(50));
        assert!(translation_chunks(&text)[0].len() > 400);
    }

    #[test]
    fn long_translation_survives_a_service_that_silently_caps_inputs() {
        let text = format!(
            "{}The final marker must survive.",
            "A complete sentence. ".repeat(70)
        );
        let mut requests = Vec::new();
        let output = translate_with_request(&text, "en", |chunk| {
            requests.push(chunk.to_string());
            // Reproduce the old failure: a nonempty but silently shortened reply.
            Ok(echo(&chunk.chars().take(598).collect::<String>()))
        })
        .unwrap();
        assert!(requests.len() > 1);
        assert!(requests.iter().all(|s| s.len() <= MAX_QUERY_BYTES));
        assert_eq!(
            without_whitespace(&requests.concat()),
            without_whitespace(&text)
        );
        assert_eq!(output, [text]);
    }

    #[test]
    fn merged_chunks_keep_newlines_without_adding_paragraphs_or_chinese_spaces() {
        let english = format!(
            "{}\r\n\n{}",
            "A complete sentence. ".repeat(35),
            "Another sentence. ".repeat(40)
        );
        let result = translate_with_request(&english, "en", |s| Ok(echo(s))).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].split_whitespace().collect::<Vec<_>>(),
            english.split_whitespace().collect::<Vec<_>>()
        );
        assert_eq!(result[0].matches('\n').count(), 2);

        let chinese = "中文长句。".repeat(100);
        let result = translate_with_request(&chinese, "zh-Hans", |s| Ok(echo(s))).unwrap();
        assert_eq!(result, [chinese]);
    }

    #[test]
    fn truncated_or_wrong_source_is_rejected_even_with_a_nonempty_translation() {
        let text = "The complete source must be translated.";
        for input in ["The complete source", "An unrelated source", ""] {
            let data = json!({"input": text, "fanyi": {"input": input, "tran": "部分译文"}});
            assert!(matches!(
                translation_paragraphs(&data, text),
                Err(ServiceError::Parse(_))
            ));
        }
        // The short phrase/dictionary fallback must also reject truncated fanyi.
        assert!(YoudaoDict::parse_dictionary_output(
            &echo("Redemption"),
            "Redemption code recharge"
        )
        .is_err());
    }

    #[test]
    fn source_verification_accepts_whitespace_normalization_and_echo_locations() {
        let text = "First line.\r\n Second\tline.";
        for data in [
            json!({"fanyi": {"input": "First line. Second line.", "tran": "两行译文"}}),
            json!({"input": text, "fanyi": {"tran": "两行译文"}}),
            json!({"meta": {"input": text}, "fanyi": {"tran": "两行译文"}}),
        ] {
            assert_eq!(
                translation_paragraphs(&data, text).unwrap().unwrap(),
                ["两行译文"]
            );
        }
    }

    #[test]
    fn a_bad_later_chunk_discards_partial_results_and_stops_requests() {
        let text = "This sentence must be fully translated. ".repeat(50);
        for bad_response in [
            echo("This sentence"),
            json!({"fanyi": {"tran": "译文没有对应原文"}}),
            json!({"fanyi": {"tran": ""}}),
            json!({"ec": {"word": {"trs": [{"tran": "无关词义"}]}}}),
        ] {
            let mut calls = 0;
            let result = translate_with_request(&text, "en", |chunk| {
                calls += 1;
                Ok(if calls == 2 {
                    bad_response.clone()
                } else {
                    echo(chunk)
                })
            });
            assert!(matches!(result, Err(ServiceError::Parse(_))));
            assert_eq!(calls, 2);
        }
        for error in [
            ServiceError::Http {
                status: 429,
                retry_after_secs: Some(120),
            },
            ServiceError::Cancelled,
            ServiceError::Timeout,
        ] {
            let code = error.info().code;
            let mut error = Some(error);
            let mut calls = 0;
            let result = translate_with_request(&text, "en", |chunk| {
                calls += 1;
                if calls == 2 {
                    Err(error.take().unwrap())
                } else {
                    Ok(echo(chunk))
                }
            });
            assert_eq!(result.unwrap_err().info().code, code);
            assert_eq!(calls, 2);
        }
    }

    #[test]
    fn short_queries_keep_one_request_and_dictionary_fallback() {
        let mut calls = 0;
        let output = translate_with_request(" hello ", "zh-Hans", |text| {
            calls += 1;
            assert_eq!(text, "hello");
            Ok(json!({"ec": {"word": {"trs": [{"pos": "int.", "tran": "你好"}]}}}))
        })
        .unwrap();
        assert_eq!(output, ["int. 你好"]);
        assert_eq!(calls, 1);
        assert!(
            translate_with_request(" \n", "en", |_| panic!("empty input must not request"))
                .is_err()
        );
    }

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
