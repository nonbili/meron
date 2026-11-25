//! Turn bare URLs in sanitized email HTML into links, the way webmail does: a
//! sender whose client never wrapped `https://…` in an `<a>` (Meron's own quick
//! reply among them) otherwise shows it as dead text in the HTML frames.
//!
//! Runs on ammonia's output, which html5ever serialised: every `<` in text is
//! escaped, attribute values are double-quoted, comments are gone and `<style>`
//! is the only raw-text element left. That makes a plain scan exact here — it
//! is not a general HTML tokenizer and must not be fed unsanitized markup.

/// The attribute on a link whose text is a URL, which the core's stylesheet
/// lets break anywhere (see `prepare_message_html`). The sanitizer drops every
/// `data-*` attribute a sender writes, so a link carrying it was marked here.
pub const URL_LINK_ATTR: &str = "data-meron-url";

/// A link in a plain body: a bare URL, or a Markdown `[label](url)` (what
/// [`crate::parse::html_to_text`] writes for an HTML link). `start` and `end`
/// are UTF-16 offsets into the body, the unit JavaScript and Kotlin index
/// strings by, so clients draw links without matching URLs themselves. A
/// `label` replaces the source range `start..end` when drawn. `href` is left
/// out when it is the linked text itself, which keeps a link-heavy page from
/// sending every URL twice.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BodyLink {
    pub start: usize,
    pub end: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Longest Markdown link target taken as one, which bounds the scan for a
/// `](` that never closes.
const MAX_MARKDOWN_URL_BYTES: usize = 4096;

/// The links in a plain body, in order and not overlapping.
pub fn plain_body_links(body: &str) -> Vec<BodyLink> {
    let mut links = Vec::new();
    // UTF-16 offset of `pos`, advanced with it.
    let mut utf16 = 0;
    let mut pos = 0;
    while pos < body.len() {
        let found = if body.as_bytes()[pos] == b'[' {
            markdown_link_at(body, pos).map(|(len, label, href)| (len, href, Some(label)))
        } else {
            url_at(body, pos, Text::Plain).map(|len| (len, bare_href(&body[pos..pos + len]), None))
        };
        if let Some((len, href, label)) = found {
            let text = &body[pos..pos + len];
            let end = utf16 + utf16_len(text);
            links.push(BodyLink {
                start: utf16,
                end,
                href: (href != text).then_some(href),
                label,
            });
            utf16 = end;
            pos += len;
            continue;
        }
        let c = body[pos..].chars().next().unwrap_or('\0');
        utf16 += c.len_utf16();
        pos += c.len_utf8().max(1);
    }
    links
}

fn utf16_len(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}

/// The href a bare URL opens: `www.` gets a scheme.
fn bare_href(url: &str) -> String {
    if url.len() >= 4 && url[..4].eq_ignore_ascii_case("www.") {
        format!("https://{url}")
    } else {
        url.to_string()
    }
}

/// A Markdown link `[label](url)` or `[label](url "title")` starting at `pos`:
/// its byte length, label and href. Only web, `mailto:` and `tel:` targets, or
/// a schemeless `host.tld/…`, count; anything else stays text. The label can't
/// hold `[` or `]`, so its scan stops at the next `[`; the URL may hold
/// balanced parentheses and brackets (`[::1]`, `a[]=1`), and its scan is
/// capped at [`MAX_MARKDOWN_URL_BYTES`].
fn markdown_link_at(body: &str, pos: usize) -> Option<(usize, String, String)> {
    let rest = &body[pos + 1..];
    let label_len = rest.find(['[', ']'])?;
    let label = &rest[..label_len];
    if label.trim().is_empty() || !rest[label_len..].starts_with("](") {
        return None;
    }
    let target = &rest[label_len + 2..];
    let mut depth = 0usize;
    let mut url_len = None;
    for (i, c) in target.char_indices() {
        if i > MAX_MARKDOWN_URL_BYTES {
            return None;
        }
        match c {
            '(' => depth += 1,
            ')' if depth == 0 => {
                url_len = Some(i);
                break;
            }
            ')' => depth -= 1,
            c if c.is_whitespace() => {
                url_len = Some(i);
                break;
            }
            _ => {}
        }
    }
    let url_len = url_len?;
    let url = &target[..url_len];
    // An optional quoted title, then the closing parenthesis.
    let after = &target[url_len..];
    let close = if after.starts_with(')') {
        0
    } else {
        let title = after.trim_start_matches([' ', '\t']);
        let quoted = title.strip_prefix('"')?;
        let end = quoted.find('"')?;
        if !quoted[end + 1..].starts_with(')') {
            return None;
        }
        after.len() - title.len() + 1 + end + 1
    };
    let href = markdown_href(url)?;
    let len = 1 + label_len + 2 + url_len + close + 1;
    Some((len, label.to_string(), href))
}

fn markdown_href(url: &str) -> Option<String> {
    let lower = url.to_ascii_lowercase();
    if ["https://", "http://", "mailto:", "tel:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme) && lower.len() > scheme.len())
    {
        return Some(url.to_string());
    }
    // `host.tld` or `host.tld/…`: letters, digits, `.`, `-`, `_` then a 2+
    // letter top-level domain.
    let host = url.split('/').next().unwrap_or("");
    let (name, tld) = host.rsplit_once('.')?;
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        && tld.len() >= 2
        && tld.chars().all(|c| c.is_ascii_alphabetic());
    valid.then(|| format!("https://{url}"))
}

/// Whether [`url_at`] scans plain text or the HTML-escaped text of a
/// sanitized document, where `&` only appears as an entity.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Text {
    Plain,
    Escaped,
}

/// Wrap each bare `http://`, `https://` or `www.` URL in the text of
/// `html` in an `<a>`, leaving text already inside a link (and CSS) alone.
/// Links whose text is a URL, new or the sender's, get [`URL_LINK_ATTR`].
pub fn linkify_html(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    if !lower.contains("http://") && !lower.contains("https://") && !lower.contains("www.") {
        return html.to_string();
    }
    let bytes = html.as_bytes();
    let mut out = String::with_capacity(html.len() + 64);
    let mut anchors = 0usize;
    let mut i = 0;
    while i < html.len() {
        if bytes[i] == b'<' {
            let end = tag_end(bytes, i);
            let (name, closing) = tag_name(&html[i..end]);
            if name.eq_ignore_ascii_case("style") && !closing {
                // Raw text: copy through to the closing tag untouched.
                let close = lower[end..].find("</style").map_or(html.len(), |p| end + p);
                out.push_str(&html[i..close]);
                i = close;
                continue;
            }
            if name.eq_ignore_ascii_case("a") {
                anchors = if closing {
                    anchors.saturating_sub(1)
                } else {
                    anchors + 1
                };
                if !closing && link_text_is_url(&html[end..]) && html[..end].ends_with('>') {
                    out.push_str(&html[i..end - 1]);
                    out.push(' ');
                    out.push_str(URL_LINK_ATTR);
                    out.push_str("=\"\">");
                    i = end;
                    continue;
                }
            }
            out.push_str(&html[i..end]);
            i = end;
            continue;
        }
        let next = html[i..].find('<').map_or(html.len(), |p| i + p);
        let text = &html[i..next];
        if anchors == 0 {
            linkify_text(text, &mut out);
        } else {
            out.push_str(text);
        }
        i = next;
    }
    out
}

/// Whether the link whose content starts `rest` holds nothing but a URL.
fn link_text_is_url(rest: &str) -> bool {
    let Some(close) = rest.find('<') else {
        return false;
    };
    let text = rest[..close].trim();
    rest[close..]
        .get(..3)
        .is_some_and(|tag| tag.eq_ignore_ascii_case("</a"))
        && !text.is_empty()
        && url_at(text, 0, Text::Escaped).is_some()
        && !text.contains(char::is_whitespace)
}

/// The index just past the `>` closing the tag that opens at `start`, skipping
/// any `>` inside a quoted attribute value.
fn tag_end(bytes: &[u8], start: usize) -> usize {
    let mut quoted = false;
    for (offset, &b) in bytes[start + 1..].iter().enumerate() {
        match b {
            b'"' => quoted = !quoted,
            b'>' if !quoted => return start + 1 + offset + 1,
            _ => {}
        }
    }
    bytes.len()
}

/// The element name of a tag, and whether it is a closing tag.
fn tag_name(tag: &str) -> (&str, bool) {
    let inner = tag.trim_start_matches('<');
    let (inner, closing) = match inner.strip_prefix('/') {
        Some(rest) => (rest, true),
        None => (inner, false),
    };
    let end = inner
        .find(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
        .unwrap_or(inner.len());
    (&inner[..end], closing)
}

/// Copy escaped text node content to `out`, wrapping each URL in it in an `<a>`.
fn linkify_text(text: &str, out: &mut String) {
    let mut copied = 0;
    let mut pos = 0;
    while pos < text.len() {
        if let Some(len) = url_at(text, pos, Text::Escaped) {
            let url = &text[pos..pos + len];
            out.push_str(&text[copied..pos]);
            out.push_str("<a href=\"");
            out.push_str(&bare_href(url));
            out.push_str("\" ");
            out.push_str(URL_LINK_ATTR);
            out.push_str("=\"\">");
            out.push_str(url);
            out.push_str("</a>");
            pos += len;
            copied = pos;
            continue;
        }
        pos += text[pos..].chars().next().map_or(1, char::len_utf8);
    }
    out.push_str(&text[copied..]);
}

/// Whether `c` is Chinese, Japanese or Korean script, written without spaces
/// between words.
fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{3040}'..='\u{30ff}' // Hiragana, Katakana
        | '\u{3400}'..='\u{4dbf}' // CJK Extension A
        | '\u{4e00}'..='\u{9fff}' // CJK Unified Ideographs
        | '\u{ac00}'..='\u{d7af}' // Hangul syllables
        | '\u{f900}'..='\u{faff}' // CJK Compatibility Ideographs
        | '\u{ff66}'..='\u{ff9f}' // Halfwidth Katakana
        | '\u{20000}'..='\u{2fa1f}' // CJK Extensions B–F, Compatibility Supplement
    )
}

/// Brackets CJK text sets a URL in: a closing one the URL didn't open ends it.
const CJK_OPEN_BRACKETS: &str = "「『【〈《〔";
const CJK_CLOSE_BRACKETS: &str = "」』】〉》〕";

/// CJK sentence punctuation, which ends a URL.
const CJK_PUNCTUATION: &str = "。，、！？：；";

/// The longest DNS name.
const MAX_HOST_BYTES: usize = 253;

/// The length of the URL starting at `pos` in `text`, if one does.
fn url_at(text: &str, pos: usize, kind: Text) -> Option<usize> {
    let rest = &text[pos..];
    let prefix = ["https://", "http://", "www."].into_iter().find(|p| {
        rest.len() >= p.len() && rest.as_bytes()[..p.len()].eq_ignore_ascii_case(p.as_bytes())
    })?;
    // Only at the start of a word: not `xhttp://`, nor the `www.` inside an
    // address or a path. An entity before it (`&lt;https://…`) is a word break,
    // and so is CJK prose, which runs into a URL without a space
    // (`詳細はhttps://…`).
    if let Some(before) = text[..pos].chars().next_back()
        && ((before.is_alphanumeric() && !is_cjk(before))
            || matches!(before, '.' | '/' | '@' | '-' | '_'))
    {
        return None;
    }
    // The host must start right after the prefix, checked before the scan
    // below: a rejected start would otherwise scan the rest of the text, once
    // per `https://(` it holds. A `www.` needs a dotted name. The name's run of
    // letters, digits, `-` and `.` can't hold another word start, so these
    // checks scan each character once.
    let host = &rest[prefix.len()..];
    let starts_name = |s: &str| s.chars().next().is_some_and(char::is_alphanumeric);
    let valid = if prefix == "www." {
        // A dotted name: the scan stops at its first `.` + letter, and is
        // capped at the longest DNS name. CJK prose before a `www.` is a word
        // break, so one run of name characters can hold many candidates; an
        // unbounded scan from each would be quadratic.
        let mut after_dot = false;
        starts_name(host)
            && host
                .char_indices()
                .take_while(|&(i, c)| {
                    i < MAX_HOST_BYTES && (c.is_alphanumeric() || matches!(c, '-' | '.'))
                })
                .any(|(_, c)| {
                    let dotted = after_dot && c.is_alphanumeric();
                    after_dot = c == '.';
                    dotted
                })
    } else {
        starts_name(host) || host.starts_with('[')
    };
    if !valid {
        return None;
    }

    let mut end = 0;
    let mut cjk_depth = 0usize;
    while end < rest.len() {
        let c = rest[end..].chars().next()?;
        // CJK sentence punctuation is never written raw in a URL, and the
        // prose after it runs on without a space. U+FEFF is an invisible
        // character `trim()` strips in the clients but Rust's whitespace isn't.
        if c.is_whitespace()
            || matches!(c, '<' | '>' | '"' | '\'' | '`' | '\u{feff}')
            || CJK_PUNCTUATION.contains(c)
        {
            break;
        }
        if CJK_OPEN_BRACKETS.contains(c) {
            cjk_depth += 1;
        } else if CJK_CLOSE_BRACKETS.contains(c) {
            if cjk_depth == 0 {
                break;
            }
            cjk_depth -= 1;
        }
        if c == '&' && kind == Text::Escaped {
            // The serialiser escapes `&` as `&amp;`, which is part of a URL;
            // any other entity (`&lt;`, `&gt;`, `&nbsp;`) ends it.
            if rest[end..].starts_with("&amp;") {
                end += "&amp;".len();
                continue;
            }
            break;
        }
        end += c.len_utf8();
    }

    // A URL written inside `<…>` ends at the `>`, whatever it ends with.
    let (open, close) = match kind {
        Text::Plain => ("<", ">"),
        Text::Escaped => ("&lt;", "&gt;"),
    };
    if text[..pos].ends_with(open) && rest[end..].starts_with(close) {
        return (end > prefix.len()).then_some(end);
    }

    // Sentence punctuation after a URL is not part of it, nor is a closing
    // bracket that has no opening one inside the URL. The bracket balances are
    // counted once and kept up as the tail is trimmed, so a long run of `)`
    // stays linear.
    const PAIRS: [(char, char); 3] = [('(', ')'), ('[', ']'), ('{', '}')];
    let mut balance = [0isize; 3];
    for c in rest[..end].chars() {
        for (i, (open, close)) in PAIRS.iter().enumerate() {
            if c == *open {
                balance[i] += 1;
            } else if c == *close {
                balance[i] -= 1;
            }
        }
    }
    while let Some(last) = rest[..end].chars().next_back() {
        let trim = match last {
            '.' | ',' | ';' | ':' | '!' | '?' | '*' => true,
            _ => match PAIRS.iter().position(|(_, close)| *close == last) {
                Some(i) if balance[i] < 0 => {
                    balance[i] += 1;
                    true
                }
                _ => false,
            },
        };
        if !trim {
            break;
        }
        end -= last.len_utf8();
    }
    // A trailing `&amp;` would be cut mid-entity by the loop above only if `;`
    // went; put it back whole or drop it whole.
    if kind == Text::Escaped && rest[..end].ends_with("&amp") {
        end -= "&amp".len();
    }

    // Trimming never reaches the host checked above; this only guards the slice.
    (end > prefix.len()).then_some(end)
}

#[cfg(test)]
mod tests {
    use super::{BodyLink, linkify_html, plain_body_links};

    #[test]
    fn links_a_bare_url_in_text() {
        assert_eq!(
            linkify_html(
                "<p>same as https://github.com/nonbili/meron/releases/tag/android%2Fv0.5.0</p>"
            ),
            "<p>same as <a href=\"https://github.com/nonbili/meron/releases/tag/android%2Fv0.5.0\" data-meron-url=\"\">\
             https://github.com/nonbili/meron/releases/tag/android%2Fv0.5.0</a></p>"
        );
    }

    #[test]
    fn marks_a_senders_link_whose_text_is_a_url() {
        assert_eq!(
            linkify_html(
                "<a href=\"https://a.example/x\"> https://a.example/x </a><a href=\"https://a.example/\">docs</a>"
            ),
            "<a href=\"https://a.example/x\" data-meron-url=\"\"> https://a.example/x </a><a href=\"https://a.example/\">docs</a>"
        );
    }

    #[test]
    fn leaves_existing_links_and_attributes_alone() {
        let html = "<p><a href=\"https://a.example/\">see <b>https://a.example/</b></a> \
                    <img src=\"https://img.example/x.png\" alt=\"a > https://b.example\"></p>";
        assert_eq!(linkify_html(html), html);
    }

    #[test]
    fn leaves_css_alone() {
        let html = "<style>p { background: url(https://x.example/a.png) }</style><p>hi</p>";
        assert_eq!(linkify_html(html), html);
    }

    #[test]
    fn trims_punctuation_and_unbalanced_brackets() {
        assert_eq!(
            linkify_html("<p>(see https://x.example/a.)</p>"),
            "<p>(see <a href=\"https://x.example/a\" data-meron-url=\"\">https://x.example/a</a>.)</p>"
        );
        assert_eq!(
            linkify_html("<p>https://en.wikipedia.org/wiki/Foo_(bar), ok</p>"),
            "<p><a href=\"https://en.wikipedia.org/wiki/Foo_(bar)\" data-meron-url=\"\">https://en.wikipedia.org/wiki/Foo_(bar)</a>, ok</p>"
        );
    }

    #[test]
    fn keeps_query_ampersands_and_stops_at_other_entities() {
        assert_eq!(
            linkify_html("<p>&lt;https://x.example/?a=1&amp;b=2&gt;&nbsp;x</p>"),
            "<p>&lt;<a href=\"https://x.example/?a=1&amp;b=2\" data-meron-url=\"\">https://x.example/?a=1&amp;b=2</a>&gt;&nbsp;x</p>"
        );
    }

    #[test]
    fn links_www_with_a_scheme() {
        assert_eq!(
            linkify_html("<p>Visit www.example.com.</p>"),
            "<p>Visit <a href=\"https://www.example.com\" data-meron-url=\"\">www.example.com</a>.</p>"
        );
    }

    #[test]
    fn ignores_non_urls() {
        for html in [
            "<p>https:// alone</p>",
            "<p>www. alone and www.nodot</p>",
            "<p>mail me at me@www.example.com</p>",
            "<p>xhttps://x.example</p>",
        ] {
            assert_eq!(linkify_html(html), html);
        }
    }

    #[test]
    fn trims_a_long_bracket_run_quickly() {
        let html = format!("<p>https://x.example/{}</p>", ")".repeat(200_000));
        let start = std::time::Instant::now();
        let out = linkify_html(&html);
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
        assert!(out.starts_with(
            "<p><a href=\"https://x.example/\" data-meron-url=\"\">https://x.example/</a>))"
        ));
    }

    #[test]
    fn rejects_a_long_run_of_bad_starts_quickly() {
        let html = format!("<p>{}</p>", "https://(".repeat(50_000));
        let start = std::time::Instant::now();
        assert_eq!(linkify_html(&html), html);
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
    }

    #[test]
    fn keeps_a_trailing_underscore() {
        assert_eq!(
            linkify_html("<p>https://x.example/?token=abc_ ok</p>"),
            "<p><a href=\"https://x.example/?token=abc_\" data-meron-url=\"\">https://x.example/?token=abc_</a> ok</p>"
        );
    }

    fn link(start: usize, end: usize, href: &str, label: Option<&str>) -> BodyLink {
        BodyLink {
            start,
            end,
            href: Some(href.to_string()),
            label: label.map(str::to_string),
        }
    }

    fn bare(start: usize, end: usize) -> BodyLink {
        BodyLink {
            start,
            end,
            href: None,
            label: None,
        }
    }

    #[test]
    fn finds_bare_urls_in_a_plain_body_without_sentence_punctuation() {
        assert_eq!(
            plain_body_links(
                "Join https://beta.example/join/AbCd1234, then (see https://x.example/a)."
            ),
            vec![bare(5, 39), bare(51, 70),]
        );
    }

    #[test]
    fn keeps_plain_ampersands_and_gives_www_a_scheme() {
        assert_eq!(
            plain_body_links("https://x.example/?a=1&b=2 www.example.com"),
            vec![bare(0, 26), link(27, 42, "https://www.example.com", None),]
        );
    }

    #[test]
    fn keeps_a_url_in_angle_brackets_whole() {
        assert_eq!(
            plain_body_links("<https://x.example/Wow!>"),
            vec![bare(1, 23)]
        );
        assert_eq!(
            linkify_html("<p>&lt;https://x.example/Wow!&gt;</p>"),
            "<p>&lt;<a href=\"https://x.example/Wow!\" data-meron-url=\"\">https://x.example/Wow!</a>&gt;</p>"
        );
    }

    #[test]
    fn keeps_brackets_inside_a_url() {
        assert_eq!(
            plain_body_links("https://x.example/?q=[a](b) ok"),
            vec![bare(0, 27)]
        );
    }

    #[test]
    fn stops_at_cjk_brackets_and_punctuation() {
        assert_eq!(
            plain_body_links("「https://x.example/日本」です https://x.example/a。"),
            vec![bare(1, 21), bare(25, 44),]
        );
    }

    #[test]
    fn finds_markdown_links_with_labels() {
        assert_eq!(
            plain_body_links(
                "See [docs](example.com/docs), [wiki](https://en.example.org/wiki/Foo_(bar) \"t\") and [x](javascript:alert(1))"
            ),
            vec![
                link(4, 28, "https://example.com/docs", Some("docs")),
                link(
                    30,
                    79,
                    "https://en.example.org/wiki/Foo_(bar)",
                    Some("wiki")
                ),
            ]
        );
    }

    #[test]
    fn counts_offsets_in_utf16() {
        assert_eq!(
            plain_body_links("😀 é https://x.example"),
            vec![bare(5, 22)]
        );
    }

    #[test]
    fn scans_unclosed_markdown_quickly() {
        let body = "[a".repeat(100_000) + &"](".repeat(100_000);
        let start = std::time::Instant::now();
        assert!(plain_body_links(&body).is_empty());
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
    }

    #[test]
    fn links_a_url_run_into_cjk_prose() {
        assert_eq!(
            plain_body_links("詳細はhttps://x.example を参照、またはxhttps://y.example"),
            vec![bare(3, 20)]
        );
    }

    #[test]
    fn keeps_brackets_in_a_markdown_target() {
        assert_eq!(
            plain_body_links("[local](http://[::1]:8080/) [list](https://x.example/?f[]=1)"),
            vec![
                link(0, 27, "http://[::1]:8080/", Some("local")),
                link(28, 60, "https://x.example/?f[]=1", Some("list")),
            ]
        );
    }

    #[test]
    fn serializes_an_href_only_when_it_differs_from_the_text() {
        assert_eq!(
            serde_json::to_string(&plain_body_links("https://x.example www.x.example")).unwrap(),
            r#"[{"start":0,"end":17},{"start":18,"end":31,"href":"https://www.x.example"}]"#
        );
    }

    #[test]
    fn keeps_balanced_cjk_brackets_in_a_url() {
        assert_eq!(
            plain_body_links("「https://x.example/wiki/「日本」」です"),
            vec![bare(1, 28)]
        );
    }

    #[test]
    fn rejects_a_run_of_www_candidates_quickly() {
        let body = "日www.-".repeat(50_000);
        let start = std::time::Instant::now();
        assert!(plain_body_links(&body).is_empty());
        let body = "日www.".repeat(50_000);
        plain_body_links(&body);
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
    }

    #[test]
    fn stops_at_cjk_sentence_punctuation_and_bom() {
        assert_eq!(
            plain_body_links("詳細はhttps://x.example/help。次の手順 https://x.example/b\u{feff}"),
            vec![bare(3, 25), bare(31, 50)]
        );
    }
}
