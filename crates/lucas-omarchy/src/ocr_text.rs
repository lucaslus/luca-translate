//! Conservative OCR line joining; paragraph, list and indented code boundaries survive.
fn han(c: char) -> bool {
    matches!(c as u32, 0x3400..=0x9fff)
}
fn structured(line: &str) -> bool {
    line.starts_with([' ', '\t'])
        || ["- ", "* ", "• ", "```"]
            .iter()
            .any(|prefix| line.starts_with(prefix))
        || line.contains('\t')
        || line.contains('|')
        || line.split_once('.').is_some_and(|(prefix, tail)| {
            !prefix.is_empty()
                && prefix.chars().all(|c| c.is_ascii_digit())
                && tail.starts_with(' ')
        })
}
pub fn clean(text: &str) -> String {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = String::new();
    let mut previous_structured = false;
    for line in normalized.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            previous_structured = false;
            continue;
        }
        let block = structured(line);
        if !out.is_empty() && !out.ends_with('\n') {
            let last = out.chars().last().unwrap();
            let first = trimmed.chars().next().unwrap();
            if block || previous_structured || ".!?。！？:：;；".contains(last) {
                out.push('\n');
            } else if last == '-'
                && out
                    .chars()
                    .rev()
                    .nth(1)
                    .is_some_and(|c| c.is_ascii_alphabetic())
                && first.is_ascii_lowercase()
            {
                out.pop();
            } else if !(han(last) && han(first)) {
                out.push(' ');
            }
        }
        out.push_str(if block { line.trim_end() } else { trimmed });
        previous_structured = block;
    }
    out.trim_end().to_string()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn joins_wrapped_prose_and_preserves_structure() {
        assert_eq!(
            clean("A trans-\nlation example\ncontinues here."),
            "A translation example continues here."
        );
        assert_eq!(
            clean("中文内容\n继续显示\n\n第二段。\n第三段"),
            "中文内容继续显示\n\n第二段。\n第三段"
        );
        assert_eq!(
            clean("- first\n- second\n  code()\n  next()"),
            "- first\n- second\n  code()\n  next()"
        );
        assert_eq!(clean(""), "");
    }
}
