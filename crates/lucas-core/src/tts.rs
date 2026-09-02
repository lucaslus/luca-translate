//! TTS 发音辅助。
//!
//! 简单场景（单词发音）直接用有道词典的 dictvoice 免费端点；
//! 复杂场景（整句 TTS、多音色）未来接入各厂商语音合成服务。

/// 构造有道 dictvoice 发音 URL。
/// `uk=true` 英式(type=1)，否则美式(type=2)。
pub fn youdao_voice_url(word: &str, uk: bool) -> String {
    let t = if uk { "1" } else { "2" };
    format!(
        "https://dict.youdao.com/dictvoice?type={}&audio={}",
        t,
        urlencoded(word)
    )
}

fn urlencoded(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            other => out.push_str(&format!("%{:02X}", other)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voice_url() {
        assert_eq!(
            youdao_voice_url("excellent", true),
            "https://dict.youdao.com/dictvoice?type=1&audio=excellent"
        );
        assert!(youdao_voice_url("good morning", false).contains("good%20morning"));
    }
}
