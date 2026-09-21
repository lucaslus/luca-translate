//! Youdao's text-translation web channel, independent of dictionary coverage.
//! Protocol checked against the public 1.0.7 web client on 2026-09-20.
use super::ServiceError;
use md5::{Digest, Md5};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const ROOT: &str = "https://dict-trans.youdao.com";
const KEY_ID: &str = "translate-webfanyi-webmain";
// Public web-client signing seed, not a user credential.
const KEY_SEED: &str = "kSy5gtKA4yRUxAVPJPrdYKZ0jBKyd3t1";

fn signed(
    mut extra: BTreeMap<String, String>,
    key: &str,
    key_id: &str,
    time: &str,
) -> BTreeMap<String, String> {
    let mut params: BTreeMap<String, String> = [
        ("product", "webfanyi"),
        ("appVersion", "1"),
        ("client", "webmain"),
        ("mid", "1"),
        ("vendor", "web"),
        ("screen", "1"),
        ("model", "1"),
        ("imei", "1"),
        ("network", "wifi"),
        ("keyfrom", "webfanyi.webmain"),
        ("keyid", key_id),
        ("mysticTime", time),
        ("yduuid", "abcdefg"),
        ("abtest", "0"),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v.into()))
    .collect();
    params.append(&mut extra);
    params.retain(|_, v| !v.is_empty());
    let input = params
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
        + "&key="
        + key;
    let points = params
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(",")
        + ",key";
    params.insert("sign".into(), format!("{:x}", Md5::digest(input)));
    params.insert("pointParam".into(), points);
    params
}
fn now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}
fn request(url: &str) -> crate::http::Request {
    crate::http::post(url)
        .set("Referer", "https://fanyi.youdao.com/")
        .set("Origin", "https://fanyi.youdao.com")
        .set("User-Agent", "Mozilla/5.0")
}
fn invalid() -> ServiceError {
    ServiceError::Parse("有道翻译响应不完整".into())
}
fn language(code: &str) -> &str {
    match code {
        "zh-Hans" => "zh-CHS",
        "zh-Hant" => "zh-CHT",
        other => other,
    }
}
// Match encodeURIComponent, before the form layer escapes the parameters again.
fn encode_input(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}
pub(super) fn translate(text: &str, from: &str, to: &str) -> Result<Vec<String>, ServiceError> {
    if text.trim().is_empty() || text.encode_utf16().count() > 5000 {
        return Err(ServiceError::Unsupported(
            "有道网页翻译单次支持 1–5000 字符".into(),
        ));
    }
    let params = signed(
        BTreeMap::from([("targetKeyid".into(), KEY_ID.into())]),
        KEY_SEED,
        "translate-webmain-key-getter",
        &now(),
    );
    let mut req = request(&format!("{ROOT}/translate/key")).timeout(Duration::from_secs(10));
    for (k, v) in &params {
        req = req.query(k, v);
    }
    let keys: Value = req.call()?.into_json()?;
    if keys["code"].as_i64() != Some(0) {
        return Err(invalid());
    }
    let key = keys
        .pointer("/data/secretKey")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(invalid)?;
    let token = keys
        .pointer("/data/token")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(invalid)?;
    let extra = [
        ("i", encode_input(text)),
        ("modelName", "llmLite".into()),
        ("useTerm", "false".into()),
        ("recTerms", "[]".into()),
        ("from", language(from).into()),
        ("to", language(to).into()),
        ("token", token.into()),
        ("source", "webmain".into()),
        ("signSecretKey", key.into()),
        ("keyId", KEY_ID.into()),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v))
    .collect();
    let params = signed(extra, key, KEY_ID, &now());
    let form: Vec<_> = params
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let body = request(&format!("{ROOT}/webtranslate/sse"))
        .timeout(Duration::from_secs(35))
        .send_form(&form)?
        .into_string()?;
    parse_stream(&body, &format!("{}2{}", language(from), language(to)))
}

fn parse_stream(body: &str, expected: &str) -> Result<Vec<String>, ServiceError> {
    let normalized = body.replace("\r\n", "\n");
    let mut begun = false;
    let mut ended = false;
    let mut result = String::new();
    let mut request_id = String::new();
    for block in normalized.split("\n\n") {
        let mut event = "";
        let mut data = Vec::new();
        for line in block.lines() {
            if let Some(s) = line.strip_prefix("event:") {
                event = s.trim();
            }
            if let Some(s) = line.strip_prefix("data:") {
                data.push(s.trim_start());
            }
        }
        if event.is_empty() && data.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(&data.join("\n")).map_err(|_| invalid())?;
        match event {
            "begin" if !begun && !ended => {
                if value["type"].as_str() != Some(expected) {
                    return Err(invalid());
                }
                request_id = value["requestId"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .ok_or_else(invalid)?
                    .into();
                begun = true;
            }
            "message" if begun && !ended => {
                result.push_str(value["transIncre"].as_str().ok_or_else(invalid)?)
            }
            "end" if begun && !ended => {
                if value["type"].as_str() != Some(expected)
                    || value["requestId"].as_str() != Some(&request_id)
                {
                    return Err(invalid());
                }
                ended = true;
            }
            _ => return Err(invalid()),
        }
    }
    if !ended || result.trim().is_empty() {
        return Err(invalid());
    }
    Ok(vec![result])
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stream() -> String {
        "event:begin\ndata:{\"requestId\":\"1\",\"type\":\"en2zh-CHS\"}\n\nevent:message\ndata:{\"transIncre\":\"被抹除\"}\n\nevent:end\ndata:{\"requestId\":\"1\",\"type\":\"en2zh-CHS\"}\n\n".into()
    }
    #[test]
    fn complete_stream_required() {
        let s = stream();
        assert_eq!(parse_stream(&s, "en2zh-CHS").unwrap(), ["被抹除"]);
        assert_eq!(
            parse_stream(&s.replace('\n', "\r\n"), "en2zh-CHS").unwrap(),
            ["被抹除"]
        );
        assert!(parse_stream(s.split("event:end").next().unwrap(), "en2zh-CHS").is_err());
        assert!(parse_stream(&s, "en2ja").is_err());
        assert!(parse_stream(&s.replace("event:end", "event:error"), "en2zh-CHS").is_err());
        assert!(parse_stream(&s.replace("被抹除", ""), "en2zh-CHS").is_err());
    }
    #[test]
    fn input_is_encoded_without_changing_the_original() {
        assert_eq!(encode_input("a b+中文"), "a%20b%2B%E4%B8%AD%E6%96%87");
    }
}
