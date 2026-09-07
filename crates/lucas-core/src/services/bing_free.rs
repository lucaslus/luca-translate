//! Bing 翻译免费通道（ttranslatev3 网页协议，国内可直连）。
//!
//! 流程（逆向 bing-translate-api 公开实现并实测验证）：
//! 1. GET www.bing.com/translator → 提取 IG / IID / key / token + Set-Cookie
//! 2. POST www.bing.com/ttranslatev3?isVertical=1&&IG=&IID=&SFX=1
//!    form: fromLang=auto-detect, text, to, token, key
//! 注意 fromLang 必须是 "auto-detect"（"auto" 会返回 400）。

use serde_json::Value;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::{ServiceError, TranslateService, TranslationOutput};

const PAGE_URL: &str = "https://www.bing.com/translator";
const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const SESSION_TTL: Duration = Duration::from_secs(1800);

pub struct BingFree;

struct BingSession {
    ig: String,
    iid: String,
    key: String,
    token: String,
    cookie: String,
    ts: Instant,
}

static SESSION: Mutex<Option<BingSession>> = Mutex::new(None);

/// 从页面 HTML 提取参数的小工具（避免引入 regex）
fn between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    s.split(start)
        .nth(1)
        .and_then(|rest| rest.split(end).next())
}

fn fetch_session() -> Result<BingSession, ServiceError> {
    let resp = ureq::get(PAGE_URL)
        .timeout(Duration::from_secs(15))
        .set("User-Agent", UA)
        .call()
        .map_err(ServiceError::from_http)?;

    // 收集 cookie（Set-Cookie 的 name=value 部分）
    let cookie = resp
        .all("set-cookie")
        .iter()
        .filter_map(|c| c.split(';').next())
        .collect::<Vec<_>>()
        .join("; ");

    let html = resp.into_string().map_err(ServiceError::from_body)?;

    let ig = between(&html, "IG:\"", "\"")
        .ok_or_else(|| ServiceError::Parse("Bing 页面缺少 IG".into()))?
        .to_string();
    let iid = between(&html, "data-iid=\"", "\"")
        .ok_or_else(|| ServiceError::Parse("Bing 页面缺少 IID".into()))?
        .to_string();
    let helper = between(&html, "params_AbusePreventionHelper", "]")
        .ok_or_else(|| ServiceError::Parse("Bing 页面缺少防滥用参数".into()))?
        .to_string();
    // helper 形如 " = [1788342980510,\"token...\",3600000"
    let arr = &helper[helper
        .find('[')
        .ok_or_else(|| ServiceError::Parse("Bing 防滥用参数格式异常".into()))?
        + 1..];
    let key = arr.split(',').next().unwrap_or("").trim().to_string();
    let token = between(arr, ",\"", "\"")
        .ok_or_else(|| ServiceError::Parse("Bing 页面缺少 token".into()))?
        .to_string();

    if key.is_empty() || token.is_empty() {
        return Err(ServiceError::Parse("Bing 参数提取失败".into()));
    }
    Ok(BingSession {
        ig,
        iid,
        key,
        token,
        cookie,
        ts: Instant::now(),
    })
}

fn get_session() -> Result<BingSession, ServiceError> {
    if let Some(s) = SESSION.lock().unwrap().as_ref() {
        if s.ts.elapsed() < SESSION_TTL {
            return Ok(BingSession {
                ig: s.ig.clone(),
                iid: s.iid.clone(),
                key: s.key.clone(),
                token: s.token.clone(),
                cookie: s.cookie.clone(),
                ts: s.ts,
            });
        }
    }
    let s = fetch_session()?;
    *SESSION.lock().unwrap() = Some(BingSession {
        ig: s.ig.clone(),
        iid: s.iid.clone(),
        key: s.key.clone(),
        token: s.token.clone(),
        cookie: s.cookie.clone(),
        ts: Instant::now(),
    });
    Ok(s)
}

fn invalidate_session() {
    *SESSION.lock().unwrap() = None;
}

/// Bob 语言代码 → Bing 代码
fn bing_lang(code: &str) -> Option<String> {
    match code {
        "zh-Hans" => Some("zh-Hans".into()),
        "zh-Hant" => Some("zh-Hant".into()),
        "en" => Some("en".into()),
        "ja" => Some("ja".into()),
        "ko" => Some("ko".into()),
        "fr" => Some("fr".into()),
        "de" => Some("de".into()),
        "ru" => Some("ru".into()),
        "es" => Some("es".into()),
        _ => None,
    }
}

fn parse_translation(data: &Value) -> Result<TranslationOutput, ServiceError> {
    let text_out = data
        .get(0)
        .and_then(|r| r.get("translations"))
        .and_then(|t| t.get(0))
        .and_then(|t| t.get("text"))
        .and_then(|v| v.as_str());
    match text_out {
        Some(t) if !t.trim().is_empty() => Ok(TranslationOutput {
            paragraphs: t.split('\n').map(str::to_string).collect(),
            detected_from: data
                .get(0)
                .and_then(|r| r.pointer("/detectedLanguage/language"))
                .and_then(|v| v.as_str())
                .map(str::to_string),
        }),
        _ => Err(ServiceError::Parse("Bing 返回异常".into())),
    }
}

fn do_translate(text: &str, from: &str, to: &str) -> Result<TranslationOutput, ServiceError> {
    let session = get_session()?;
    let target = bing_lang(to)
        .ok_or_else(|| ServiceError::Unsupported(format!("Bing 不支持目标语言 {to}")))?;
    let source = if from == "auto" {
        "auto-detect".to_string()
    } else {
        bing_lang(from)
            .ok_or_else(|| ServiceError::Unsupported(format!("Bing 不支持源语言 {from}")))?
    };

    let url = format!(
        "https://www.bing.com/ttranslatev3?isVertical=1&&IG={}&IID={}&SFX=1",
        session.ig, session.iid
    );
    let resp = ureq::post(&url)
        .timeout(Duration::from_secs(15))
        .set("User-Agent", UA)
        .set("Referer", PAGE_URL)
        .set("Cookie", &session.cookie)
        .send_form(&[
            ("fromLang", source.as_str()),
            ("text", text),
            ("to", target.as_str()),
            ("token", session.token.as_str()),
            ("key", session.key.as_str()),
            ("tryFetchingGenderDebiasedTranslations", "true"),
        ])
        .map_err(ServiceError::from_http)?;
    let data: Value = resp.into_json().map_err(ServiceError::from_body)?;

    match parse_translation(&data) {
        Ok(output) => Ok(output),
        Err(error) => {
            // 会话过期等场景：使缓存失效并提示上层重试
            invalidate_session();
            Err(error)
        }
    }
}

impl TranslateService for BingFree {
    fn name(&self) -> &'static str {
        "Bing"
    }

    fn translate(&self, text: &str, from: &str, to: &str) -> Result<Vec<String>, ServiceError> {
        // Do not retry 429/5xx/network errors immediately. Recovery belongs to
        // the coordinator, which isolates this provider and observes cooldowns.
        do_translate(text, from, to).map(|output| output.paragraphs)
    }

    fn translate_with_detection(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<TranslationOutput, ServiceError> {
        do_translate(text, from, to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_includes_bing_detected_language() {
        let data = serde_json::json!([{
            "detectedLanguage": {"language": "en", "score": 1.0},
            "translations": [{"text": "自主的", "to": "zh-Hans"}]
        }]);
        let output = parse_translation(&data).unwrap();
        assert_eq!(output.paragraphs, ["自主的"]);
        assert_eq!(output.detected_from.as_deref(), Some("en"));
    }
}
