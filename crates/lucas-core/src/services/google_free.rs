//! Google 翻译免费 Web 端点（translate.googleapis.com，client=gtx）。
//!
//! 无需密钥，广泛用于开源项目（DeepLX、openai-translator 等同类思路）。
//! 响应为嵌套数组：data[0] = [[trans, orig, ...], ...]，data[2] = 检测语言。

use super::{ServiceError, TranslateService, TranslationOutput};

const API: &str = "https://translate.googleapis.com/translate_a/single";

pub struct GoogleFree;

fn parse_response(data: &serde_json::Value) -> Result<TranslationOutput, ServiceError> {
    let mut paras = Vec::new();
    if let Some(rows) = data.get(0).and_then(|v| v.as_array()) {
        for row in rows {
            if let Some(seg) = row.get(0).and_then(|v| v.as_str()) {
                paras.push(seg.to_string());
            }
        }
    }
    if paras.is_empty() {
        return Err(ServiceError::Parse("响应中没有翻译数据".into()));
    }
    Ok(TranslationOutput {
        // Google 会把长文本切成多段，同句拼接：段间无空行时直接相连。
        paragraphs: vec![paras.concat()],
        detected_from: data.get(2).and_then(|v| v.as_str()).map(str::to_string),
    })
}

fn request_translation(
    text: &str,
    from: &str,
    to: &str,
) -> Result<TranslationOutput, ServiceError> {
    use crate::lang::{auto_target, detect_source, map_lang};
    let table: &[(&str, &str)] = &[
        ("zh-Hans", "zh-CN"),
        ("zh-Hant", "zh-TW"),
        ("en", "en"),
        ("ja", "ja"),
        ("ko", "ko"),
        ("fr", "fr"),
        ("de", "de"),
        ("ru", "ru"),
        ("es", "es"),
    ];
    let sl = map_lang(from, table);
    let resolved_to = if to == "auto" {
        auto_target(detect_source(text)).to_string()
    } else {
        to.to_string()
    };
    let tl = map_lang(&resolved_to, table);

    let resp = crate::http::get(API)
        .timeout(std::time::Duration::from_secs(15))
        .query("client", "gtx")
        .query("sl", &sl)
        .query("tl", &tl)
        .query("dt", "t")
        .query("q", text)
        .call()?;
    let data: serde_json::Value = resp.into_json()?;
    parse_response(&data)
}

impl TranslateService for GoogleFree {
    fn name(&self) -> &'static str {
        "GoogleFree"
    }

    fn translate(&self, text: &str, from: &str, to: &str) -> Result<Vec<String>, ServiceError> {
        request_translation(text, from, to).map(|output| output.paragraphs)
    }

    fn translate_with_detection(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<TranslationOutput, ServiceError> {
        request_translation(text, from, to)
    }

    fn detect(&self, _text: &str) -> String {
        // detect 在 translate 中已知；此处用粗检测兜底
        crate::lang::detect_source(_text).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_includes_google_detected_language() {
        let data = serde_json::json!([[["自主的", "autonomous"]], null, "en"]);
        let output = parse_response(&data).unwrap();
        assert_eq!(output.paragraphs, ["自主的"]);
        assert_eq!(output.detected_from.as_deref(), Some("en"));
    }
}
