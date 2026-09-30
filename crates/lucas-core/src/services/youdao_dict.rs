//! 有道词典（web 端 jsonapi_s 接口）。
//!
//! 返回结构（实测）：
//! - `ec.word`: { usphone, ukphone, trs: [{pos, tran}], return-phrase, usspeech, ukspeech }
//! - `fanyi.tran`: 整句翻译时的译文字符串或段落数组
//!
//! 该接口即 Bob 第三方插件（如 Free 有道翻译）所用通道，无需密钥。

use md5::{Digest, Md5};
use serde_json::Value;

use super::{
    DictCard, DictService, DictionaryHelp, ServiceError, TranslateService, TranslationOutput,
};

const API: &str = "https://dict.youdao.com/jsonapi_s?doctype=json&jsonversion=4";
// Public web client signing key, observed in Youdao's web_dict 3.1.0 client
// (76bdd54.js) on 2026-09-17. Unsigned requests can return unrelated words.
const WEB_SIGN_KEY: &str = "Mk6hqtUp33DGGtoS63tTJbMUYjRrG1Lu";
// This endpoint truncated 701/1200-character English inputs around character 600.
// A conservative UTF-8 byte budget also fits character/UTF-16 based limits.
const MAX_QUERY_BYTES: usize = 500;

pub struct YoudaoDict;

pub(super) struct DictionaryOutput {
    pub dict: Option<DictCard>,
    pub paragraphs: Vec<String>,
    pub help: Option<DictionaryHelp>,
    pub detected_from: Option<String>,
}

impl YoudaoDict {
    fn request_once(&self, text: &str, language: &str) -> Result<Value, ServiceError> {
        let (t, sign) = request_signature(text);
        let resp = crate::http::post(API)
            .timeout(std::time::Duration::from_secs(15))
            // 浏览器 UA：降低被风控返回空结果的概率
            .set(
                "User-Agent",
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            )
            .set("Referer", "https://dict.youdao.com/")
            .send_form(&[
                ("q", text),
                ("le", language),
                ("t", &t),
                ("client", "web"),
                ("sign", &sign),
                ("keyfrom", "webdict"),
            ])
            ?;
        resp.into_json::<Value>()
    }

    fn request(&self, text: &str, language: &str) -> Result<Value, ServiceError> {
        // Empty/invalid responses are surfaced, never retried blindly.
        self.request_once(text, language)
    }

    pub(super) fn lookup_or_translate(
        &self,
        text: &str,
        from: &str,
    ) -> Result<DictionaryOutput, ServiceError> {
        lookup_with_fallback(
            text,
            self.request(text, "en"),
            || super::youdao_translate::translate(text, from, "zh-Hans"),
            |lemma| self.lookup(lemma),
        )
    }

    fn parse_dictionary_output(data: &Value, text: &str) -> Result<DictionaryOutput, ServiceError> {
        if let Ok(dict) = Self::parse_dictionary(data, text) {
            return Ok(DictionaryOutput {
                paragraphs: dictionary_paragraphs(&dict),
                dict: Some(dict),
                help: None,
                detected_from: detected_source(data, text).map(str::to_string),
            });
        }
        translation_paragraphs(data, text)?
            .map(|paragraphs| DictionaryOutput {
                dict: None,
                paragraphs,
                help: None,
                detected_from: detected_source(data, text).map(str::to_string),
            })
            .ok_or_else(missing_content)
    }
}

fn has_phonetics(dict: &DictCard) -> bool {
    [&dict.uk_phonetic, &dict.us_phonetic]
        .into_iter()
        .any(|p| p.as_ref().is_some_and(|s| !s.trim().is_empty()))
}

fn lookup_with_fallback(
    text: &str,
    response: Result<Value, ServiceError>,
    translate: impl FnOnce() -> Result<TranslationOutput, ServiceError>,
    lookup_lemma: impl FnOnce(&str) -> Result<DictCard, ServiceError>,
) -> Result<DictionaryOutput, ServiceError> {
    let data = match response {
        Err(ServiceError::Cancelled) => return Err(ServiceError::Cancelled),
        Ok(data) if validate_response_input(&data, text).is_ok() => Some(data),
        _ => None,
    };
    let parsed = data
        .as_ref()
        .and_then(|d| YoudaoDict::parse_dictionary_output(d, text).ok());
    if parsed
        .as_ref()
        .and_then(|p| p.dict.as_ref())
        .is_some_and(has_phonetics)
    {
        return Ok(parsed.unwrap());
    }
    // This is a separate translation endpoint, not a retry of a failed lookup.
    let output = match translate() {
        Ok(output) if output.paragraphs.iter().any(|s| !s.trim().is_empty()) => output,
        Err(ServiceError::Cancelled) => return Err(ServiceError::Cancelled),
        result => return parsed.ok_or_else(|| result.err().unwrap_or_else(missing_content)),
    };
    let detected_from = output.detected_from.or_else(|| {
        data.as_ref()
            .and_then(|data| detected_source(data, text))
            .map(str::to_string)
    });
    let mut help = DictionaryHelp::default();
    if let Some(data) = data {
        if let Some(typos) = data.pointer("/typos/typo").and_then(Value::as_array) {
            for word in typos.iter().filter_map(|v| v["word"].as_str()) {
                if word != text
                    && word.len() <= 80
                    && !word.trim().is_empty()
                    && !help.suggestions.iter().any(|s| s == word)
                {
                    help.suggestions.push(word.into());
                }
                if help.suggestions.len() == 3 {
                    break;
                }
            }
        }
        // Only an explicit dictionary relationship permits a lemma lookup.
        // Never guess a lemma by stripping suffixes or choosing a typo suggestion.
        let lemma = data
            .pointer("/ec/word/prototype")
            .and_then(Value::as_str)
            .or_else(|| {
                parsed
                    .as_ref()
                    .and_then(|p| p.dict.as_ref())
                    .map(|d| d.word.as_str())
            })
            .filter(|word| {
                !word.eq_ignore_ascii_case(text) && !word.trim().is_empty() && word.len() <= 80
            });
        if let Some(lemma) = lemma {
            match lookup_lemma(lemma) {
                Ok(card) if card.word.eq_ignore_ascii_case(lemma) && has_phonetics(&card) => {
                    help.lemma = Some(card)
                }
                Err(ServiceError::Cancelled) => return Err(ServiceError::Cancelled),
                _ => {} // An optional pronunciation lookup must not discard translation.
            }
        }
    }
    Ok(DictionaryOutput {
        paragraphs: output.paragraphs,
        dict: None,
        help: (!help.suggestions.is_empty() || help.lemma.is_some()).then_some(help),
        detected_from,
    })
}

impl DictService for YoudaoDict {
    fn name(&self) -> &'static str {
        "YoudaoDict"
    }

    fn lookup(&self, text: &str) -> Result<DictCard, ServiceError> {
        Self::parse_dictionary(&self.request(text, "en")?, text)
    }
}

impl YoudaoDict {
    fn parse_dictionary(data: &Value, text: &str) -> Result<DictCard, ServiceError> {
        validate_response_input(data, text)?;
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

fn request_signature(text: &str) -> (String, String) {
    // Match JavaScript String.length, including surrogate pairs for emoji.
    let t = ((text.encode_utf16().count() + "webdict".len()) % 10).to_string();
    let query_hash = format!("{:x}", Md5::digest(format!("{text}webdict")));
    let sign = format!(
        "{:x}",
        Md5::digest(format!("web{text}{t}{WEB_SIGN_KEY}{query_hash}"))
    );
    (t, sign)
}

impl TranslateService for YoudaoDict {
    fn name(&self) -> &'static str {
        "YoudaoDict"
    }

    fn translate(&self, text: &str, from: &str, to: &str) -> Result<Vec<String>, ServiceError> {
        self.translate_with_detection(text, from, to)
            .map(|output| output.paragraphs)
    }

    fn translate_with_detection(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<TranslationOutput, ServiceError> {
        let source = crate::lang::source_hint(text, from);
        query_language(source, to)?;
        match translate_for_languages(text, from, to, |chunk, language| {
            self.request(chunk, language)
        }) {
            Ok(result) => Ok(result),
            Err(ServiceError::Cancelled) => Err(ServiceError::Cancelled),
            Err(_) => super::youdao_translate::translate(text, from, to),
        }
    }
}

// `le` selects the foreign side of a Chinese/foreign pair, in both directions.
// These pairs were checked against the live endpoint. Other pairs remain
// unsupported here rather than silently returning a different target language.
fn query_language<'a>(from: &'a str, to: &'a str) -> Result<&'a str, ServiceError> {
    let language = match (from, to) {
        ("zh-Hans", other) | (other, "zh-Hans") => other,
        _ => "",
    };
    if matches!(language, "en" | "es" | "fr" | "de" | "ja" | "ko" | "ru") {
        Ok(language)
    } else {
        Err(ServiceError::Unsupported(
            "有道词典通道支持简体中文与英、西、法、德、日、韩、俄语互译".into(),
        ))
    }
}

fn validate_translation_language(data: &Value, from: &str, to: &str) -> Result<(), ServiceError> {
    if !data.get("fanyi").is_none_or(Value::is_null) {
        let provider_code = |code| if code == "zh-Hans" { "zh-CHS" } else { code };
        let expected = format!("{}2{}", provider_code(from), provider_code(to));
        if data.pointer("/fanyi/type").and_then(Value::as_str) != Some(expected.as_str()) {
            return Err(ServiceError::Parse("有道返回的翻译语向与请求不一致".into()));
        }
    } else if (from, to) != ("en", "zh-Hans") {
        // Only English->Chinese can use the English dictionary fallback.
        return Err(missing_content());
    }
    Ok(())
}

fn translate_for_languages(
    text: &str,
    from: &str,
    to: &str,
    mut request: impl FnMut(&str, &str) -> Result<Value, ServiceError>,
) -> Result<TranslationOutput, ServiceError> {
    let source = crate::lang::source_hint(text, from);
    let language = query_language(source, to)?;
    let mut detected_from = None;
    let mut all_confirmed = true;
    let paragraphs = translate_with_request(text, to, |chunk| {
        let data = request(chunk, language)?;
        validate_translation_language(&data, source, to)?;
        let detected = detected_source(&data, chunk);
        if from == "auto" && detected.is_some_and(|detected| detected != source) {
            return Err(ServiceError::Parse("有道检测语言与翻译语向不一致".into()));
        }
        all_confirmed &=
            detected.is_some() && (detected_from.is_none() || detected_from == detected);
        detected_from = detected;
        Ok(data)
    })?;
    Ok(TranslationOutput {
        paragraphs,
        detected_from: if from == "auto" && all_confirmed {
            detected_from.map(str::to_string)
        } else {
            None
        },
    })
}

fn detected_source(data: &Value, text: &str) -> Option<&'static str> {
    // `le` and `lang` select the dictionary, even for Chinese input. Only
    // guessLanguage describes the source, and its echoed input must match.
    response_input(data)?;
    validate_response_input(data, text).ok()?;
    data.pointer("/meta/guessLanguage")
        .and_then(Value::as_str)
        .and_then(crate::lang::normalize_provider_language)
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
        if chunked && response_input(&data).is_none() {
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
            } else if !matches!(to, "zh-Hans" | "zh-Hant" | "ja") {
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

fn response_input(data: &Value) -> Option<&str> {
    ["/fanyi/input", "/input", "/meta/input"]
        .iter()
        .find_map(|path| data.pointer(path).and_then(Value::as_str))
}

fn validate_response_input(data: &Value, text: &str) -> Result<(), ServiceError> {
    // The service can normalize spaces/newlines. All non-whitespace source
    // characters must still be present, in order; a nonempty tran is not enough.
    if let Some(input) = response_input(data) {
        if !input
            .chars()
            .filter(|c| !c.is_whitespace())
            .eq(text.chars().filter(|c| !c.is_whitespace()))
        {
            return Err(incomplete_translation());
        }
    }
    Ok(())
}

fn translation_paragraphs(data: &Value, text: &str) -> Result<Option<Vec<String>>, ServiceError> {
    validate_response_input(data, text)?;
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

    #[test]
    fn detection_uses_verified_source_metadata_not_dictionary_selection() {
        let chinese =
            json!({"meta": {"input": "你好", "guessLanguage": "zh"}, "le": "en", "lang": "eng"});
        assert_eq!(detected_source(&chinese, "你好"), Some("zh-Hans"));
        assert_eq!(detected_source(&chinese, "再见"), None);
        for data in [
            json!({"input": "hello", "le": "en", "lang": "eng"}),
            json!({"meta": {"guessLanguage": "eng"}}),
            json!({"meta": {"input": "hello", "guessLanguage": "unknown"}}),
        ] {
            assert_eq!(detected_source(&data, "hello"), None);
        }
    }

    #[test]
    fn dictionary_and_translation_fallback_preserve_provider_detection() {
        let data = json!({"meta": {"input": "hello", "guessLanguage": "eng"}, "ec": {"word": {
            "return-phrase": "hello", "usphone": "həˈloʊ", "trs": [{"tran": "你好"}]
        }}});
        let output = lookup_with_fallback("hello", Ok(data), || panic!(), |_| panic!()).unwrap();
        assert_eq!(output.detected_from.as_deref(), Some("en"));
        assert!(output.dict.is_some());

        let output = lookup_with_fallback(
            "bonjour",
            Err(missing_content()),
            || {
                Ok(TranslationOutput {
                    paragraphs: vec!["你好".into()],
                    detected_from: Some("fr".into()),
                })
            },
            |_| panic!(),
        )
        .unwrap();
        assert_eq!(output.detected_from.as_deref(), Some("fr"));
        assert_eq!(output.paragraphs, ["你好"]);

        let output = lookup_with_fallback(
            "walked",
            Ok(json!({"meta": {"input": "walked", "guessLanguage": "eng"}})),
            || Ok(TranslationOutput::paragraphs(vec!["走了".into()])),
            |_| panic!(),
        )
        .unwrap();
        assert_eq!(output.detected_from.as_deref(), Some("en"));
    }

    #[test]
    fn sentence_detection_survives_chunking_without_guessing_missing_languages() {
        let text = "This sentence must be fully translated. ".repeat(30);
        for missing in [false, true] {
            let mut calls = 0;
            let output = translate_for_languages(&text, "auto", "zh-Hans", |chunk, _| {
                calls += 1;
                Ok(json!({
                    "meta": {"input": chunk, "guessLanguage": if missing && calls == 1 { None } else { Some("eng") }},
                    "fanyi": {"input": chunk, "type": "en2zh-CHS", "tran": "译文"}
                }))
            }).unwrap();
            assert!(calls > 1);
            assert_eq!(
                output.detected_from.as_deref(),
                if missing { None } else { Some("en") }
            );
        }
        let output = translate_for_languages("hello", "en", "zh-Hans", |chunk, _| {
            Ok(json!({
                "meta": {"input": chunk, "guessLanguage": "eng"},
                "fanyi": {"input": chunk, "type": "en2zh-CHS", "tran": "你好"}
            }))
        })
        .unwrap();
        assert_eq!(output.detected_from, None);
    }

    #[test]
    fn automatic_dictionary_direction_must_match_detected_language() {
        let output = translate_for_languages("bonjour", "auto", "zh-Hans", |chunk, _| {
            Ok(json!({
                "meta": {"input": chunk, "guessLanguage": "fr"},
                "fanyi": {"input": chunk, "type": "en2zh-CHS", "tran": "你好"}
            }))
        });
        assert!(
            output.is_err(),
            "a wrong local source hint must use automatic text translation"
        );
    }

    #[test]
    fn typo_suggestions_never_replace_input_or_trigger_lemma_lookup() {
        let output = lookup_with_fallback("abliterated", Ok(json!({
            "input": "abliterated", "typos": {"typo": [{"word": "obliterated"}, {"word": "obliterate"}]}
        })), || Ok(TranslationOutput::paragraphs(vec!["被抹除".into()])), |_| panic!("suggestions are not lemmas")).unwrap();
        assert_eq!(output.paragraphs, ["被抹除"]);
        assert!(output.dict.is_none());
        let help = output.help.unwrap();
        assert_eq!(help.suggestions, ["obliterated", "obliterate"]);
        assert!(help.lemma.is_none());
    }

    #[test]
    fn dictionary_failure_falls_back_but_cancellation_does_not() {
        for error in [
            ServiceError::Timeout,
            missing_content(),
            ServiceError::Network(String::new()),
        ] {
            let result = lookup_with_fallback(
                "word",
                Err(error),
                || Ok(TranslationOutput::paragraphs(vec!["译文".into()])),
                |_| panic!(),
            )
            .unwrap();
            assert_eq!(result.paragraphs, ["译文"]);
            assert!(result.help.is_none());
        }
        assert!(matches!(
            lookup_with_fallback(
                "word",
                Err(ServiceError::Cancelled),
                || panic!(),
                |_| panic!()
            ),
            Err(ServiceError::Cancelled)
        ));
    }

    #[test]
    fn missing_phonetics_can_use_an_explicit_lemma_without_replacing_translation() {
        let data = json!({"input":"walked", "ec":{"word":{
            "return-phrase":"walked", "prototype":"walk", "trs":[{"tran":"走过"}]
        }}});
        let result = lookup_with_fallback(
            "walked",
            Ok(data.clone()),
            || Ok(TranslationOutput::paragraphs(vec!["走了".into()])),
            |word| {
                assert_eq!(word, "walk");
                YoudaoDict::parse_dictionary(
                    &json!({"input":"walk", "ec":{"word":{
                        "return-phrase":"walk", "usphone":"wɔːk", "trs":[{"tran":"走"}]
                    }}}),
                    "walk",
                )
            },
        )
        .unwrap();
        assert_eq!(result.paragraphs, ["走了"]);
        assert!(result.dict.is_none());
        assert_eq!(result.help.unwrap().lemma.unwrap().word, "walk");
        let result = lookup_with_fallback(
            "walked",
            Ok(data),
            || Ok(TranslationOutput::paragraphs(vec!["走了".into()])),
            |_| Err(ServiceError::Timeout),
        )
        .unwrap();
        assert_eq!(result.paragraphs, ["走了"]);
    }

    #[test]
    fn dictionary_with_phonetics_does_not_need_translation() {
        let result = lookup_with_fallback(
            "hello",
            Ok(json!({"input":"hello", "ec":{"word":{
                "return-phrase":"hello", "usphone":"həˈloʊ", "trs":[{"tran":"你好"}]
            }}})),
            || panic!(),
            |_| panic!(),
        )
        .unwrap();
        assert!(result.dict.is_some());
    }

    fn echo(text: &str) -> Value {
        json!({"fanyi": {"input": text, "tran": text}})
    }

    fn without_whitespace(text: &str) -> String {
        text.chars().filter(|c| !c.is_whitespace()).collect()
    }

    #[test]
    fn request_signatures_match_the_web_client() {
        // Vectors checked against the web client's algorithm and live endpoint.
        for (text, t, sign) in [
            ("farewell", "5", "f61eede0a490830f097f812c4b7607d0"),
            ("hello", "2", "3c71569a04e3231adce6ef811c67148a"),
            (
                "Redemption code recharge",
                "1",
                "b63ae342e0fd0ea56028882bce002c5a",
            ),
            ("再见🦀", "1", "a6b3b77fe7f052e94cc358e053ca9adf"),
        ] {
            assert_eq!(request_signature(text), (t.into(), sign.into()), "{text}");
        }
    }

    #[test]
    fn chinese_foreign_pairs_select_the_foreign_language_in_both_directions() {
        for language in ["en", "es", "fr", "de", "ja", "ko", "ru"] {
            for (from, to, direction) in [
                (language, "zh-Hans", format!("{language}2zh-CHS")),
                ("zh-Hans", language, format!("zh-CHS2{language}")),
            ] {
                let mut calls = 0;
                let result = translate_for_languages("source", from, to, |text, le| {
                    calls += 1;
                    assert_eq!(le, language);
                    Ok(json!({"fanyi": {"input": text, "type": direction, "tran": "译文"}}))
                })
                .unwrap();
                assert_eq!(result.paragraphs, ["译文"]);
                assert_eq!(calls, 1);
            }
        }
        for (from, to) in [
            ("es", "en"),
            ("en", "es"),
            ("zh-Hans", "zh-Hant"),
            ("it", "zh-Hans"),
        ] {
            assert!(matches!(
                translate_for_languages("source", from, to, |_, _| panic!(
                    "unsupported pair must not request"
                )),
                Err(ServiceError::Unsupported(_))
            ));
        }
    }

    #[test]
    fn auto_spanish_uses_spanish_and_preserves_explicit_source() {
        let text =
            "Cierto, realize una prueba similar y el 3.8 me dió la misma UI que me dió la 3.6";
        for (from, language) in [("auto", "es"), ("es", "es"), ("en", "en")] {
            let result = translate_for_languages(text, from, "zh-Hans", |input, le| {
                assert_eq!(input, text);
                assert_eq!(le, language);
                Ok(json!({"fanyi": {
                    "input": input, "type": format!("{le}2zh-CHS"), "tran": "我做了类似的测试。"
                }}))
            })
            .unwrap();
            assert_eq!(result.paragraphs, ["我做了类似的测试。"]);
        }
    }

    #[test]
    fn wrong_or_missing_direction_cannot_return_a_successful_translation() {
        for kind in [
            json!("en2zh-CHS"),
            json!("zh-CHS2es"),
            json!("es2en"),
            json!(null),
            json!(42),
        ] {
            let result = translate_for_languages("prueba", "es", "zh-Hans", |text, _| {
                Ok(json!({"fanyi": {"input": text, "type": kind, "tran": "错误译文"}}))
            });
            assert!(matches!(result, Err(ServiceError::Parse(_))));
        }
        let result = translate_for_languages("prueba", "es", "zh-Hans", |text, _| {
            Ok(json!({"input": text, "ec": {"word": {"trs": [{"tran": "英语词典释义"}]}}}))
        });
        assert!(
            result.is_err(),
            "Spanish must not fall back to an English dictionary"
        );
    }

    #[test]
    fn later_chunk_with_wrong_direction_discards_partial_translation() {
        let text = "Hice una prueba similar y obtuve el mismo resultado. ".repeat(25);
        let mut calls = 0;
        let result = translate_for_languages(&text, "auto", "zh-Hans", |chunk, le| {
            calls += 1;
            assert_eq!(le, "es");
            Ok(json!({"fanyi": {
                "input": chunk, "type": if calls == 1 { "es2zh-CHS" } else { "en2zh-CHS" },
                "tran": "部分译文"
            }}))
        });
        assert!(result.is_err());
        assert_eq!(calls, 2);
    }

    #[test]
    fn unrelated_dictionary_is_rejected_even_with_usable_meanings() {
        for echo in [
            json!({"input": "undamaged"}),
            json!({"meta": {"input": "undamaged"}}),
        ] {
            let mut data = echo;
            data["ec"] = json!({"word": {
                "return-phrase": "undamaged",
                "usphone": "ʌnˈdæmɪdʒd",
                "trs": [{"pos": "adj.", "tran": "未损坏的"}]
            }});
            assert!(YoudaoDict::parse_dictionary(&data, "farewell").is_err());
            assert!(YoudaoDict::parse_dictionary_output(&data, "farewell").is_err());
            assert!(translate_with_request("farewell", "zh-Hans", |_| Ok(data.clone())).is_err());
        }
    }

    #[test]
    fn matching_query_can_return_a_dictionary_lemma() {
        let data = json!({
            "input": "farewells",
            "ec": {"word": {
                "return-phrase": "farewell",
                "trs": [{"pos": "n.", "tran": "告别"}]
            }}
        });
        let output = YoudaoDict::parse_dictionary_output(&data, "farewells").unwrap();
        assert_eq!(output.dict.unwrap().word, "farewell");
        assert_eq!(output.paragraphs, ["n. 告别"]);
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
        for target in ["en", "es", "fr", "de", "ko", "ru"] {
            let result = translate_with_request(&english, target, |s| Ok(echo(s))).unwrap();
            assert_eq!(result.len(), 1);
            assert_eq!(
                result[0].split_whitespace().collect::<Vec<_>>(),
                english.split_whitespace().collect::<Vec<_>>()
            );
            assert_eq!(result[0].matches('\n').count(), 2);
        }

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
