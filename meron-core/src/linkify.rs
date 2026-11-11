//! Turn bare URLs in sanitized email HTML into links, the way webmail does: a
//! sender whose client never wrapped `https://…` in an `<a>` (Meron's own quick
//! reply among them) otherwise shows it as dead text in the HTML frames.
//!
//! Runs on ammonia's output, which html5ever serialised: every `<` in text is
//! escaped, attribute values are double-quoted, comments are gone and `<style>`
//! is the only raw-text element left. That makes a plain scan exact here — it
//! is not a general HTML tokenizer and must not be fed unsanitized markup.

/// Wrap each bare `http://`, `https://` or `www.` URL in the text of
/// `html` in an `<a>`, leaving text already inside a link (and CSS) alone.
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
        if let Some(len) = url_at(text, pos) {
            let url = &text[pos..pos + len];
            out.push_str(&text[copied..pos]);
            let scheme = if url.len() >= 4 && url[..4].eq_ignore_ascii_case("www.") {
                "http://"
            } else {
                ""
            };
            out.push_str("<a href=\"");
            out.push_str(scheme);
            out.push_str(url);
            out.push_str("\">");
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

/// The length of the URL starting at `pos` in escaped text, if one does.
fn url_at(text: &str, pos: usize) -> Option<usize> {
    let rest = &text[pos..];
    let prefix = ["https://", "http://", "www."].into_iter().find(|p| {
        rest.len() >= p.len() && rest.as_bytes()[..p.len()].eq_ignore_ascii_case(p.as_bytes())
    })?;
    // Only at the start of a word: not `xhttp://`, nor the `www.` inside an
    // address or a path. An entity before it (`&lt;https://…`) is a word break.
    if let Some(before) = text[..pos].chars().next_back()
        && (before.is_alphanumeric() || matches!(before, '.' | '/' | '@' | '-' | '_'))
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
        let name_len = host
            .find(|c: char| !(c.is_alphanumeric() || matches!(c, '-' | '.')))
            .unwrap_or(host.len());
        let name = &host[..name_len];
        starts_name(name)
            && name
                .match_indices('.')
                .any(|(i, _)| starts_name(&name[i + 1..]))
    } else {
        starts_name(host) || host.starts_with('[')
    };
    if !valid {
        return None;
    }

    let mut end = 0;
    while end < rest.len() {
        let c = rest[end..].chars().next()?;
        if c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\'' | '`') {
            break;
        }
        if c == '&' {
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
    if rest[..end].ends_with("&amp") {
        end -= "&amp".len();
    }

    // Trimming never reaches the host checked above; this only guards the slice.
    (end > prefix.len()).then_some(end)
}

#[cfg(test)]
mod tests {
    use super::linkify_html;

    #[test]
    fn links_a_bare_url_in_text() {
        assert_eq!(
            linkify_html(
                "<p>same as https://github.com/nonbili/meron/releases/tag/android%2Fv0.5.0</p>"
            ),
            "<p>same as <a href=\"https://github.com/nonbili/meron/releases/tag/android%2Fv0.5.0\">\
             https://github.com/nonbili/meron/releases/tag/android%2Fv0.5.0</a></p>"
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
            "<p>(see <a href=\"https://x.example/a\">https://x.example/a</a>.)</p>"
        );
        assert_eq!(
            linkify_html("<p>https://en.wikipedia.org/wiki/Foo_(bar), ok</p>"),
            "<p><a href=\"https://en.wikipedia.org/wiki/Foo_(bar)\">https://en.wikipedia.org/wiki/Foo_(bar)</a>, ok</p>"
        );
    }

    #[test]
    fn keeps_query_ampersands_and_stops_at_other_entities() {
        assert_eq!(
            linkify_html("<p>&lt;https://x.example/?a=1&amp;b=2&gt;&nbsp;x</p>"),
            "<p>&lt;<a href=\"https://x.example/?a=1&amp;b=2\">https://x.example/?a=1&amp;b=2</a>&gt;&nbsp;x</p>"
        );
    }

    #[test]
    fn links_www_with_a_scheme() {
        assert_eq!(
            linkify_html("<p>Visit www.example.com.</p>"),
            "<p>Visit <a href=\"http://www.example.com\">www.example.com</a>.</p>"
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
        assert!(out.starts_with("<p><a href=\"https://x.example/\">https://x.example/</a>))"));
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
            "<p><a href=\"https://x.example/?token=abc_\">https://x.example/?token=abc_</a> ok</p>"
        );
    }
}
