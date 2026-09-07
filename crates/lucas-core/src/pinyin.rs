//! 拼音生成（带声调）。
//!
//! 这是 lucas-translate 相对 Bob 的差异化功能之一：
//! 当原文或译文包含中文时，UI 会展示一行拼音标注，帮助学习者对照读音。

use pinyin::ToPinyin;

use crate::lang::contains_han;

fn is_han_char(c: char) -> bool {
    matches!(c as u32, 0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0x20000..=0x2A6DF)
}

/// 为文本中的汉字生成带声调拼音，音节以空格分隔，非汉字字符原样保留。
/// 若文本不含汉字，返回 None。
///
/// ```text
/// "你好，world" -> "nǐ hǎo，world"
/// ```
pub fn annotate(s: &str) -> Option<String> {
    if !contains_han(s) {
        return None;
    }
    let mut out = String::new();
    for ch in s.chars() {
        if is_han_char(ch) {
            match ch.to_pinyin() {
                Some(py) => out.push_str(py.with_tone()),
                None => out.push(ch),
            }
        } else {
            out.push(ch);
        }
    }
    let t = out.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// 仅提取汉字的拼音（忽略所有非汉字字符），适合做整行拼音标注。
/// 不含汉字返回 None。
pub fn to_pinyin_tone(s: &str) -> Option<String> {
    if !contains_han(s) {
        return None;
    }
    let mut out: Vec<&str> = Vec::new();
    for ch in s.chars() {
        if is_han_char(ch) {
            if let Some(py) = ch.to_pinyin() {
                out.push(py.with_tone());
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pinyin_basic() {
        assert_eq!(to_pinyin_tone("翻译").as_deref(), Some("fān yì"));
        assert_eq!(to_pinyin_tone("hello"), None);
        assert!(to_pinyin_tone("你好世界").unwrap().contains("nǐ"));
    }

    #[test]
    fn test_annotate() {
        let r = annotate("你好，world").unwrap();
        assert!(r.contains("nǐ"));
        assert!(r.contains("world"));
    }
}
