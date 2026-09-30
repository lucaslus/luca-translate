//! Bing 翻译免费通道（ttranslatev3 网页协议，国内可直连）。
//!
//! 流程（逆向 bing-translate-api 公开实现并实测验证）：
//! 1. GET www.bing.com/translator → 提取 IG / IID / key / token + Set-Cookie
//! 2. POST 页面重定向后的 Bing 域名 /ttranslatev3?isVertical=1&IG=&IID=&SFX=1
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

#[derive(Clone)]
struct BingSession {
    origin: String,
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
    let resp = crate::http::get(PAGE_URL)
        .timeout(Duration::from_secs(15))
        .set("User-Agent", UA)
        .call()?;

    // A regional 302 on the translation POST drops its form body. Reuse the
    // origin that served the page, together with its cookies and auth material.
    let origin = session_origin(resp.url())?;

    // 收集 cookie（Set-Cookie 的 name=value 部分）
    let cookie = resp
        .all("set-cookie")
        .iter()
        .filter_map(|c| c.split(';').next())
        .collect::<Vec<_>>()
        .join("; ");

    let html = resp.into_string()?;

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
        origin,
        ig,
        iid,
        key,
        token,
        cookie,
        ts: Instant::now(),
    })
}

fn session_origin(url: &url::Url) -> Result<String, ServiceError> {
    let host = url.host_str().unwrap_or_default();
    if url.scheme() != "https"
        || !(host == "bing.com" || host.ends_with(".bing.com"))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return Err(ServiceError::Parse("Bing 页面地址异常".into()));
    }
    Ok(url.origin().ascii_serialization())
}

fn translation_url(session: &BingSession) -> String {
    let mut url = url::Url::parse(&format!("{}/ttranslatev3", session.origin))
        .expect("validated Bing origin");
    url.query_pairs_mut().extend_pairs([
        ("isVertical", "1"),
        ("IG", &session.ig),
        ("IID", &session.iid),
        ("SFX", "1"),
    ]);
    url.into()
}

fn get_session() -> Result<BingSession, ServiceError> {
    if let Some(s) = SESSION.lock().unwrap().as_ref() {
        if s.ts.elapsed() < SESSION_TTL {
            return Ok(s.clone());
        }
    }
    let s = fetch_session()?;
    *SESSION.lock().unwrap() = Some(s.clone());
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
    // Bing also sends failures in an HTTP 200 JSON body. Preserve the status so
    // the coordinator can apply its normal rate-limit and service cooldowns.
    if let Some(status @ 400..=599) = data.get("statusCode").and_then(Value::as_u64) {
        return Err(ServiceError::Http {
            status: status as u16,
            retry_after_secs: None,
        });
    }
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

    let url = translation_url(&session);
    let resp = crate::http::post(&url)
        .timeout(Duration::from_secs(15))
        .set("User-Agent", UA)
        .set("Referer", &format!("{}/translator", session.origin))
        .set("Cookie", &session.cookie)
        .send_form(&[
            ("fromLang", source.as_str()),
            ("text", text),
            ("to", target.as_str()),
            ("token", session.token.as_str()),
            ("key", session.key.as_str()),
            ("tryFetchingGenderDebiasedTranslations", "true"),
        ]);
    let output = resp
        .and_then(|response| response.into_json::<Value>())
        .and_then(|data| parse_translation(&data));
    match output {
        Ok(output) => Ok(output),
        Err(error) => {
            if matches!(
                error,
                ServiceError::Parse(_)
                    | ServiceError::Http {
                        status: 400..=403,
                        ..
                    }
            ) {
                invalidate_session();
            }
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
    fn translation_uses_the_resolved_page_origin() {
        for host in ["www.bing.com", "cn.bing.com"] {
            let session = BingSession {
                origin: session_origin(
                    &url::Url::parse(&format!("https://{host}/translator?mkt=zh-CN")).unwrap(),
                )
                .unwrap(),
                ig: "page-ig".into(),
                iid: "translator.5023".into(),
                key: "key".into(),
                token: "token".into(),
                cookie: "cookie".into(),
                ts: Instant::now(),
            };
            let url = url::Url::parse(&translation_url(&session.clone())).unwrap();
            assert_eq!(url.host_str(), Some(host));
            assert_eq!(url.path(), "/ttranslatev3");
            assert!(url
                .query_pairs()
                .any(|(key, value)| key == "IG" && value == "page-ig"));
        }
        for page in [
            "http://cn.bing.com/translator",
            "https://bing.com.example.org/translator",
            "https://notbing.com/translator",
            "https://user@cn.bing.com/translator",
            "https://cn.bing.com:8443/translator",
        ] {
            assert!(
                session_origin(&url::Url::parse(page).unwrap()).is_err(),
                "{page}"
            );
        }
    }

    #[test]
    fn json_errors_keep_their_status_without_exposing_the_body() {
        for status in [400, 401, 403, 429, 500, 503] {
            let error = parse_translation(&serde_json::json!({
                "statusCode": status, "message": "private response"
            }))
            .unwrap_err();
            assert_eq!(error.info().http_status, Some(status));
            assert!(!error.to_string().contains("private response"));
        }
        assert!(parse_translation(&serde_json::json!([])).is_err());
        assert!(parse_translation(&serde_json::json!([{"translations":[{"text":" "}]}])).is_err());
    }

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
