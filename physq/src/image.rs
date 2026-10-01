//! Image / video extraction from `answer` markdown (§3, §8.2).
//! Extracts `![alt](src)`, `<img ...>` and `<video ...>` after stripping code
//! fences / inline code. Videos use the same `![label](qa_images/clip.mp4)`
//! syntax as images and are told apart by file extension ([`is_video_src`]),
//! exactly like the web renderer (`search.html` `isVideoSrc`).

use regex::Regex;
use std::sync::OnceLock;

/// Video file extensions (lowercase). Keep in sync with `search.html` /
/// `qa_editor.html` `isVideoSrc`, `scripts/build.js` `MEDIA_EXT_RE` and
/// `scripts/normalize-images.js` `VIDEO_EXT_RE`.
pub const VIDEO_EXTS: [&str; 5] = ["mp4", "m4v", "webm", "ogv", "mov"];

/// Whether `src` points at a video, judged by extension like the web does
/// (query strings and `#t=…` fragments ignored, case-insensitive).
pub fn is_video_src(src: &str) -> bool {
    let path = src.split(['?', '#']).next().unwrap_or("");
    let file = path.rsplit('/').next().unwrap_or(path);
    match file.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => {
            VIDEO_EXTS.iter().any(|v| v.eq_ignore_ascii_case(ext))
        }
        _ => false,
    }
}

/// One image or video reference found in an `answer`, with its byte span in
/// that answer so a renderer can split the surrounding text around it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRef {
    pub start: usize,
    pub end: usize,
    /// alt text (images) / `aria-label` or `title` (`<video>`) / the
    /// `![label](…)` text (markdown videos).
    pub alt: String,
    pub src: String,
}

impl ImageRef {
    pub fn is_video(&self) -> bool {
        is_video_src(&self.src)
    }
}

fn fence_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // ```...``` (dot matches newline via (?s)) and `...`
    RE.get_or_init(|| Regex::new(r"(?s)```.*?```|`[^`]*`").unwrap())
}

fn fence_ranges(s: &str) -> Vec<(usize, usize)> {
    fence_regex()
        .find_iter(s)
        .map(|m| (m.start(), m.end()))
        .collect()
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

fn html_video_open_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)<video\b[^>]*>").unwrap())
}

fn html_video_close_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)</video\s*>").unwrap())
}

/// `(start, end, opening tag, body)` of every `<video>` element outside code.
/// An element runs to its `</video>` — but only if that closing tag comes
/// before the next `<video` and before the next code span, so an unclosed tag
/// can't swallow later content (refs must not overlap: the TUI lays out the
/// answer around them). Unclosed → just the opening tag.
fn video_elements<'a>(
    answer: &'a str,
    fences: &[(usize, usize)],
) -> Vec<(usize, usize, &'a str, &'a str)> {
    let opens: Vec<_> = html_video_open_regex()
        .find_iter(answer)
        .filter(|m| !in_fence(m.start(), fences))
        .collect();
    let mut out = Vec::new();
    for (k, m) in opens.iter().enumerate() {
        let limit = [
            opens.get(k + 1).map(|n| n.start()),
            fences
                .iter()
                .map(|&(s, _)| s)
                .filter(|&s| s >= m.end())
                .min(),
        ]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(answer.len());
        let window = &answer[m.end()..limit];
        match html_video_close_regex().find(window) {
            Some(c) => out.push((
                m.start(),
                m.end() + c.end(),
                m.as_str(),
                &window[..c.start()],
            )),
            None => out.push((m.start(), m.end(), m.as_str(), "")),
        }
    }
    out
}

fn source_src_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)<source\b[^>]*>").unwrap())
}

fn label_regexes() -> &'static [Regex; 4] {
    static RE: OnceLock<[Regex; 4]> = OnceLock::new();
    RE.get_or_init(|| {
        [
            Regex::new(r#"(?i)\saria-label\s*=\s*"([^"]*)""#).unwrap(),
            Regex::new(r"(?i)\saria-label\s*=\s*'([^']*)'").unwrap(),
            Regex::new(r#"(?i)\stitle\s*=\s*"([^"]*)""#).unwrap(),
            Regex::new(r"(?i)\stitle\s*=\s*'([^']*)'").unwrap(),
        ]
    })
}

/// `alt` then `src` attribute patterns, each tried double-quoted → single-quoted
/// (→ unquoted for `src`). `\b`/`\s` anchors keep `data-alt=` / `data-src=`
/// from matching.
fn attr_regexes() -> &'static [Regex; 5] {
    static RE: OnceLock<[Regex; 5]> = OnceLock::new();
    RE.get_or_init(|| {
        [
            Regex::new(r#"(?i)[\s<]alt\s*=\s*"([^"]*)""#).unwrap(),
            Regex::new(r"(?i)[\s<]alt\s*=\s*'([^']*)'").unwrap(),
            Regex::new(r#"(?i)\ssrc\s*=\s*"([^"]*)""#).unwrap(),
            Regex::new(r"(?i)\ssrc\s*=\s*'([^']*)'").unwrap(),
            Regex::new(r"(?i)\ssrc\s*=\s*([^\s>]+)").unwrap(),
        ]
    })
}

fn first_capture(res: &[Regex], tag: &str) -> Option<String> {
    res.iter()
        .find_map(|re| re.captures(tag).and_then(|c| c.get(1)))
        .map(|m| m.as_str().to_string())
}

fn tag_src(tag: &str) -> String {
    first_capture(&attr_regexes()[2..5], tag)
        .map(|s| s.trim_end_matches(['"', '\'', '>']).to_string())
        .unwrap_or_default()
}

fn html_src_alt(tag: &str) -> (String, String) {
    let alt = first_capture(&attr_regexes()[0..2], tag).unwrap_or_default();
    (alt, tag_src(tag))
}

/// Every image and video in `answer`, in **document order**, with byte spans.
/// - Code fences (```...```) and inline code (`...`) are ignored — fence
///   detection runs over the whole answer, so a multi-line fence hides the
///   images inside it no matter how the caller later splits lines.
/// - Markdown `![alt](src "title")` (image or video by extension), HTML
///   `<img alt="..." src="...">`, and HTML `<video src>` / `<video><source
///   src></video>` (one entry per `<video>` element) are handled.
pub fn image_refs(answer: &str) -> Vec<ImageRef> {
    if answer.is_empty() {
        return Vec::new();
    }
    let ranges = fence_ranges(answer);
    let mut out = Vec::new();

    for caps in md_regex().captures_iter(answer) {
        let m = caps.get(0).unwrap();
        if in_fence(m.start(), &ranges) {
            continue;
        }

        let src = caps.get(2).map_or("", |x| x.as_str());
        if src.is_empty() {
            continue;
        }
        out.push(ImageRef {
            start: m.start(),
            end: m.end(),
            alt: caps.get(1).map_or("", |x| x.as_str()).to_string(),
            src: src.to_string(),
        });
    }

    let videos = video_elements(answer, &ranges);
    for &(start, end, open, body) in &videos {
        let mut src = tag_src(open);
        if src.is_empty() {
            src = source_src_regex()
                .find_iter(body)
                .map(|t| tag_src(t.as_str()))
                .find(|s| !s.is_empty())
                .unwrap_or_default();
        }
        if src.is_empty() {
            continue;
        }
        out.push(ImageRef {
            start,
            end,
            alt: first_capture(label_regexes(), open).unwrap_or_default(),
            src,
        });
    }

    for m in html_img_regex().find_iter(answer) {
        if in_fence(m.start(), &ranges) {
            continue;
        }

        let (alt, src) = html_src_alt(m.as_str());
        if src.is_empty() {
            continue;
        }
        out.push(ImageRef {
            start: m.start(),
            end: m.end(),
            alt,
            src,
        });
    }

    // Images inside a <video> element are its fallback content, not
    // separate entries (and would overlap the video's span).
    out.retain(|r| {
        !videos
            .iter()
            .any(|&(s, e, _, _)| r.start > s && r.start < e)
    });
    // Selection index i (TUI) must mean the i-th image *as displayed*, so
    // markdown and HTML images are interleaved by position.
    out.sort_by_key(|r| r.start);
    out
}

/// Extract `(alt, src)` pairs from `answer` markdown, in document order.
/// - `alt` may be empty, `src` may be `qa_images/...` relative or an absolute `https://` URL.
/// - `qa_images/` relative is the only form that gets UUID-normalized upstream; this function keeps it as-is.
pub fn extract_images(answer: &str) -> Vec<(String, String)> {
    image_refs(answer)
        .into_iter()
        .map(|r| (r.alt, r.src))
        .collect()
}

/// Resolve an image `src` from the corpus into something a browser / OS
/// opener can load: absolute `http(s):` and `data:` URLs pass through,
/// protocol-relative `//host/…` gets `https:` (an OS opener has no page
/// scheme to inherit), and anything else is a repo-relative path served from
/// the data host (`file_url`).
pub fn resolve_image_url(src: &str, file_url: impl Fn(&str) -> String) -> String {
    let lower = src.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("data:") {
        src.to_string()
    } else if let Some(rest) = src.strip_prefix("//") {
        format!("https://{rest}")
    } else {
        file_url(src.trim_start_matches("./"))
    }
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

    #[test]
    fn markdown_and_html_images_come_back_in_document_order() {
        // Regression: markdown images used to be returned before every HTML
        // one, so TUI selection #0 (the first image on screen, an <img>)
        // opened the markdown image further down.
        let ans = "<img src=\"qa_images/first.jpg\" alt=\"1\">\ntext\n![2](qa_images/second.jpg)";
        let v = extract_images(ans);
        assert_eq!(v[0].1, "qa_images/first.jpg");
        assert_eq!(v[1].1, "qa_images/second.jpg");
    }

    #[test]
    fn spans_point_at_the_image_syntax() {
        let ans = "see ![a](qa_images/a.jpg) done";
        let r = &image_refs(ans)[0];
        assert_eq!(&ans[r.start..r.end], "![a](qa_images/a.jpg)");
    }

    #[test]
    fn data_attributes_are_not_mistaken_for_alt_or_src() {
        let ans = r#"<img data-alt="no" alt="yes" data-src="nope.jpg" src="qa_images/ok.jpg">"#;
        assert_eq!(
            extract_images(ans),
            vec![("yes".to_string(), "qa_images/ok.jpg".to_string())]
        );
    }

    #[test]
    fn video_detection_by_extension() {
        for v in [
            "qa_images/a.mp4",
            "qa_images/a.MP4",
            "qa_images/a.webm",
            "qa_images/a.m4v",
            "qa_images/a.mov",
            "qa_images/a.ogv",
            "https://ex.com/v.mp4?x=1",
            "qa_images/a.mp4#t=5",
        ] {
            assert!(is_video_src(v), "{v}");
        }
        for n in [
            "qa_images/a.jpg",
            "qa_images/mp4",
            "qa_images/a.mp4.jpg",
            "qa_images/.mp4",
            "",
            "https://ex.com/mp4/",
        ] {
            assert!(!is_video_src(n), "{n}");
        }
    }

    #[test]
    fn markdown_and_html_videos_are_extracted_in_order() {
        let ans = "![振り子](qa_images/a.mp4)\n\
                   <video controls title=\"斜面\" poster=\"qa_images/p.jpg\">\n\
                   <source src=\"qa_images/b.webm\" type=\"video/webm\">\n\
                   <img src=\"qa_images/fallback.jpg\" alt=\"fb\">\n\
                   </video>\n\
                   <video src='qa_images/c.mov' aria-label='台車'>\n\
                   ![img](qa_images/d.png)\n\
                   ```\n<video src=\"qa_images/no.mp4\"></video>\n```";
        let refs = image_refs(ans);
        let got: Vec<(&str, &str, bool)> = refs
            .iter()
            .map(|r| (r.alt.as_str(), r.src.as_str(), r.is_video()))
            .collect();
        assert_eq!(
            got,
            vec![
                ("振り子", "qa_images/a.mp4", true),
                ("斜面", "qa_images/b.webm", true),
                ("台車", "qa_images/c.mov", true),
                ("img", "qa_images/d.png", false),
            ]
        );
        // the whole <video>…</video> element is one span (Detail renders one row)
        assert!(ans[refs[1].start..refs[1].end].ends_with("</video>"));
        // the unclosed <video> is just its opening tag — it must not run on to
        // the `</video>` inside the later code fence and swallow d.png
        assert!(ans[refs[2].start..refs[2].end].starts_with("<video"));
        assert!(ans[refs[2].start..refs[2].end].ends_with('>'));
        assert!(refs[2].end <= refs[3].start);
        // spans never overlap (the TUI's answer_pieces relies on it)
        assert!(refs.windows(2).all(|w| w[0].end <= w[1].start));
    }

    #[test]
    fn resolve_image_url_handles_every_src_shape() {
        let base = |p: &str| format!("https://host/repo/{p}");
        assert_eq!(
            resolve_image_url("qa_images/a.jpg", base),
            "https://host/repo/qa_images/a.jpg"
        );
        assert_eq!(
            resolve_image_url("./qa_images/a.jpg", base),
            "https://host/repo/qa_images/a.jpg"
        );
        assert_eq!(
            resolve_image_url("https://ex.com/a.png", base),
            "https://ex.com/a.png"
        );
        assert_eq!(
            resolve_image_url("HTTPS://ex.com/a.png", base),
            "HTTPS://ex.com/a.png"
        );
        // protocol-relative: an OS opener has no page scheme to inherit
        assert_eq!(
            resolve_image_url("//cdn.ex.com/a.png", base),
            "https://cdn.ex.com/a.png"
        );
    }
}
