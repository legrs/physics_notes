//! Image extraction from `answer` markdown (§3, §8.2).
//! Extracts `![alt](src)` and `<img ...>` after stripping code fences / inline code.

use regex::Regex;
use std::sync::OnceLock;

fn fence_ranges(s: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    // ```...``` (dot matches newline via (?s)) and `...`
    let re = Regex::new(r"(?s)```.*?```|`[^`]*`").unwrap();
    for m in re.find_iter(s) {
        ranges.push((m.start(), m.end()));
    }
    ranges
}

fn in_fence(pos: usize, ranges: &[(usize, usize)]) -> bool {
    ranges.iter().any(|(s, e)| pos >= *s && pos < *e)
}

fn md_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"!\[([^\]]*)\]\(\s*([^\s)]+)(?:\s+"[^"]*")?\s*\)"#).unwrap())
}

fn html_img_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)<img\b[^>]*>").unwrap())
}

fn html_src_alt(tag: &str) -> (String, String) {
    // alt: try double-quoted, then single-quoted
    let alt = {
        let re_double = Regex::new(r#"(?i)alt\s*=\s*"([^"]*)""#).unwrap();
        if let Some(c) = re_double.captures(tag) {
            c.get(1).map(|m| m.as_str().to_string()).unwrap_or_default()
        } else {
            let re_single = Regex::new(r"(?i)alt\s*=\s*'([^']*)'").unwrap();
            re_single
                .captures(tag)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
                .unwrap_or_default()
        }
    };
    // src: quoted double, then single, then unquoted
    let re_src_double = Regex::new(r#"(?i)\ssrc\s*=\s*"([^"]*)""#).unwrap();
    if let Some(c) = re_src_double.captures(tag) {
        let src = c.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
        return (alt, src);
    }
    let re_src_single = Regex::new(r"(?i)\ssrc\s*=\s*'([^']*)'").unwrap();
    if let Some(c) = re_src_single.captures(tag) {
        let src = c.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
        return (alt, src);
    }
    let src_unquoted = Regex::new(r"(?i)\ssrc\s*=\s*([^\s>]+)").unwrap();
    if let Some(c) = src_unquoted.captures(tag) {
        let src = c.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
        let src = src.trim_end_matches(['"', '\'', '>']).to_string();
        return (alt, src);
    }
    (alt, String::new())
}

/// Extract `(alt, src)` pairs from `answer` markdown.
/// - Code fences (```...```) and inline code (`...`) are ignored.
/// - Both markdown `![alt](src "title")` and HTML `<img alt="..." src="...">` are handled.
/// - `alt` may be empty, `src` may be `qa_images/...` relative or an absolute `https://` URL.
/// - `qa_images/` relative is the only form that gets UUID-normalized upstream; this function keeps it as-is.
pub fn extract_images(answer: &str) -> Vec<(String, String)> {
    if answer.is_empty() {
        return Vec::new();
    }
    let ranges = fence_ranges(answer);
    let mut out = Vec::new();

    // markdown
    for caps in md_regex().captures_iter(answer) {
        let m = caps.get(0).unwrap();
        if in_fence(m.start(), &ranges) {
            continue;
        }
        let alt = caps
            .get(1)
            .map(|x| x.as_str().to_string())
            .unwrap_or_default();
        let src = caps
            .get(2)
            .map(|x| x.as_str().to_string())
            .unwrap_or_default();
        if src.is_empty() {
            continue;
        }
        out.push((alt, src));
    }

    // html <img>
    for m in html_img_regex().find_iter(answer) {
        if in_fence(m.start(), &ranges) {
            continue;
        }
        let tag = m.as_str();
        let (alt, src) = html_src_alt(tag);
        if src.is_empty() {
            continue;
        }
        out.push((alt, src));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_markdown_images() {
        let ans = "Text ![alt text](qa_images/abc.jpg) more";
        let v = extract_images(ans);
        assert_eq!(
            v,
            vec![("alt text".to_string(), "qa_images/abc.jpg".to_string())]
        );
    }

    #[test]
    fn extracts_multiple_and_external() {
        let ans = "![a](qa_images/a.jpg)\n![b](https://example.com/b.png)\n";
        let v = extract_images(ans);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].1, "qa_images/a.jpg");
        assert_eq!(v[1].1, "https://example.com/b.png");
    }

    #[test]
    fn ignores_code_fence() {
        let ans = "```\n![ignore](qa_images/ignore.jpg)\n```\n![real](qa_images/real.jpg)";
        let v = extract_images(ans);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].1, "qa_images/real.jpg");
    }

    #[test]
    fn ignores_inline_code() {
        let ans = "`![ignore](qa_images/x.jpg)` and ![real](qa_images/y.jpg)";
        let v = extract_images(ans);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].1, "qa_images/y.jpg");
    }

    #[test]
    fn extracts_html_img() {
        let ans = r#"<img alt="photo" src="qa_images/p.jpg">"#;
        let v = extract_images(ans);
        assert_eq!(
            v,
            vec![("photo".to_string(), "qa_images/p.jpg".to_string())]
        );
    }

    #[test]
    fn empty_alt_allowed() {
        let ans = "![](qa_images/empty.jpg)";
        let v = extract_images(ans);
        assert_eq!(v[0].0, "");
        assert_eq!(v[0].1, "qa_images/empty.jpg");
    }

    #[test]
    fn title_ignored() {
        let ans = r#"![alt](qa_images/a.jpg "title")"#;
        let v = extract_images(ans);
        assert_eq!(v[0].1, "qa_images/a.jpg");
    }
}
