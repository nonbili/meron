use super::*;

pub fn html_to_text(html: &str) -> String {
    let options = ConversionOptions::builder()
        .extract_metadata(false)
        .skip_images(false)
        // Render bare links as `[text](url)` rather than `<url>` autolinks — the
        // reader's link tokenizer handles the former, but leaves the angle
        // brackets of the latter as stray text.
        .autolinks(false)
        .strip_tags(vec!["style".to_string(), "script".to_string()])
        .build();
    let clean_html = flatten_tables(&strip_invisible_html(html));
    let mut markdown = convert(&clean_html, Some(options.clone()))
        .ok()
        .and_then(|result| result.content)
        .unwrap_or_default();
    if markdown.trim().is_empty()
        && let Some(body) = body_inner_html(&clean_html)
    {
        markdown = convert(&format!("<div>{body}</div>"), Some(options))
            .ok()
            .and_then(|result| result.content)
            .unwrap_or_default();
    }
    normalize_text(&compact_image_links(&strip_heading_markers(
        &rewrite_image_markdown(&markdown),
    )))
}

pub(super) fn is_whitespace_or_invisible(c: char) -> bool {
    c.is_whitespace()
        || c == '\u{200b}' // zero-width space
        || c == '\u{200c}' // zero-width non-joiner
        || c == '\u{200d}' // zero-width joiner
        || c == '\u{feff}' // zero-width no-break space
}

pub(super) fn normalize_text(text: &str) -> String {
    let mut final_lines = Vec::new();
    let mut consecutive_empty = 0;

    for line in text.lines() {
        let trimmed = line.trim_end_matches(is_whitespace_or_invisible);
        if trimmed.is_empty() {
            consecutive_empty += 1;
            if consecutive_empty <= 1 {
                final_lines.push("");
            }
        } else {
            consecutive_empty = 0;
            final_lines.push(trimmed);
        }
    }

    // Trim leading/trailing empty lines
    let mut start = 0;
    while start < final_lines.len() && final_lines[start].is_empty() {
        start += 1;
    }
    let mut end = final_lines.len();
    while end > start && final_lines[end - 1].is_empty() {
        end -= 1;
    }

    final_lines[start..end].join("\n").trim().to_string()
}

pub(super) fn rewrite_image_markdown(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut rest = markdown;
    while let Some(start) = rest.find("![") {
        out.push_str(&rest[..start]);
        let after_marker = &rest[start + 2..];
        let Some(alt_end) = after_marker.find("](") else {
            out.push_str(&rest[start..]);
            return out;
        };
        let alt = &after_marker[..alt_end];
        let after_alt = &after_marker[alt_end + 2..];
        let Some(url_end) = after_alt.find(')') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let label = alt.trim();
        // Drop decorative/icon images (no alt text) entirely — e.g. GitHub's
        // issue-type icon or tracking pixels. Keeping them produced noise, and
        // when they sit inside an anchor (`[![](icon) text](url)`) they broke the
        // surrounding link. Named images become a plain `[image: alt]` marker
        // (Gmail's plain-text convention) rather than a link — we don't auto-load
        // remote images, and inline ones already show as thumbnails below.
        let tail = &after_alt[url_end + 1..];
        if label.is_empty() {
            if out.ends_with('[') && tail.starts_with("](") {
                // This is an empty image that is the sole content of a link: `[![](src)](url)`.
                // We want to drop the entire link since it has no readable text.
                out.pop(); // Pop the leading `[`
                if let Some(anchor_url_end) = tail[2..].find(')') {
                    rest = &tail[2 + anchor_url_end + 1..];
                } else {
                    rest = tail;
                }
            } else {
                // Normal decorative/icon image: just drop it.
                // Also swallow a single trailing space the icon left behind, so a
                // dropped leading icon doesn't open its link with `[ text`.
                rest = tail.strip_prefix(' ').unwrap_or(tail);
            }
        } else if out.ends_with('[') && tail.starts_with("](") {
            // Linked image (`[![alt](src)](url)`): the image is the sole content
            // of an anchor (e.g. a logo that links somewhere). Emit the marker as
            // the link's text — the surrounding `[` ... `](url)` already wrap it —
            // so the reader renders a clickable `[image: alt](url)` link instead
            // of dropping the destination.
            out.push_str(&format!("image: {label}"));
            rest = tail;
        } else {
            out.push_str(&format!("[image: {label}]"));
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

pub(super) fn strip_heading_markers(markdown: &str) -> String {
    markdown
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') {
                trimmed.trim_start_matches('#').trim_start()
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn compact_image_links(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut previous_image_link = false;
    for line in markdown.lines() {
        let trimmed = line.trim();
        let image_link = trimmed.starts_with("[image: ");
        if image_link && previous_image_link {
            if !out.ends_with(' ') {
                out.push(' ');
            }
            out.push_str(trimmed);
        } else {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(line);
        }
        previous_image_link = image_link;
    }
    out
}

pub(super) fn strip_invisible_html(html: &str) -> String {
    strip_html_block(&strip_html_block(html, "style"), "script")
}

/// Neutralize HTML table layout so the markdown converter doesn't render it as a
/// markdown table. Emails (Google, newsletters, Outlook) use tables purely for
/// layout; rendering them as tables crams whole paragraphs into single cells,
/// strips block structure (text runs together), and emits `| --- |` noise the
/// reader can't render. Cells/rows become `<div>` to keep block separation;
/// table containers are dropped. Tag-only rewrite — inner content is preserved.
pub(super) fn flatten_tables(html: &str) -> String {
    const DROP: &[&str] = &[
        "table", "thead", "tbody", "tfoot", "colgroup", "col", "caption",
    ];
    const BLOCK: &[&str] = &["tr", "td", "th"];

    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        let after = &rest[lt..];
        let Some(gt) = after.find('>') else {
            out.push_str(after);
            return out;
        };
        let inner = &after[1..gt];
        let closing = inner.trim_start().starts_with('/');
        let name: String = inner
            .trim_start()
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();

        if DROP.contains(&name.as_str()) {
            // drop the tag, keep content
        } else if BLOCK.contains(&name.as_str()) {
            out.push_str(if closing { "</div>" } else { "<div>" });
        } else {
            out.push_str(&after[..=gt]);
        }
        rest = &after[gt + 1..];
    }
    out.push_str(rest);
    out
}

pub(super) fn body_inner_html(html: &str) -> Option<&str> {
    let lower = html.to_ascii_lowercase();
    let body_start = lower.find("<body")?;
    let open_end = lower[body_start..].find('>')? + body_start + 1;
    let close_start = lower[open_end..].find("</body>")? + open_end;
    Some(&html[open_end..close_start])
}

pub(super) fn strip_html_block(html: &str, tag: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    let open_pattern = format!("<{tag}");
    let close_pattern = format!("</{tag}>");
    loop {
        let lower = rest.to_ascii_lowercase();
        let Some(start) = lower.find(&open_pattern) else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..start]);
        let Some(end_rel) = lower[start..].find(&close_pattern) else {
            return out;
        };
        let end = start + end_rel + close_pattern.len();
        rest = &rest[end..];
    }
}

#[derive(Default)]
pub(super) struct BodySources {
    pub(super) plain: Option<String>,
    pub(super) html: Option<String>,
}

/// Pick body sources, walking multipart trees. Multipart/alternative usually
/// carries both text/plain and text/html; the plain part is the source for the
/// conversation/plain reader, and HTML is kept for the HTML reader.
pub(super) fn body_sources(part: &mailparse::ParsedMail) -> BodySources {
    let mut plain = find_text_part(part, "text/plain");
    let html = find_text_part(part, "text/html");
    // Some senders put their HTML into the text/plain alternative as well.
    // Shown as text it is a wall of tags (and a notification snippet reading
    // `<div style=...`), so fall back to rendering the real HTML part. Only
    // then: that part is what the HTML tab shows, so nothing is lost, while a
    // plain-only message is always kept as the sender wrote it.
    if html.is_some() && plain.as_deref().is_some_and(plain_is_markup) {
        plain = None;
    }
    if plain.is_some() || html.is_some() {
        return BodySources { plain, html };
    }

    let ctype = part.ctype.mimetype.to_ascii_lowercase();
    if part.subparts.is_empty() && ctype.starts_with("text/") {
        let content = text_body(part).unwrap_or_default();
        if content.contains("<html")
            || content.contains("<body")
            || content.contains("<p>")
            || content.contains("</div>")
        {
            return BodySources {
                plain: None,
                html: Some(content),
            };
        }
        return BodySources {
            plain: Some(content),
            html: None,
        };
    }
    BodySources::default()
}

/// Whether a text/plain part is really an HTML document: it opens and closes
/// on a tag, and tags make up most of it. Prose that quotes tags as examples
/// (`<p>hello</p>` then an explanation) is mostly text and stays plain.
pub(super) fn plain_is_markup(plain: &str) -> bool {
    let trimmed = plain.trim();
    if !trimmed.starts_with('<') || !trimmed.ends_with('>') {
        return false;
    }
    let (mut markup, mut total) = (0usize, 0usize);
    let mut in_tag = false;
    for c in trimmed.chars() {
        if c == '<' {
            in_tag = true;
        }
        if !c.is_whitespace() {
            total += 1;
            if in_tag {
                markup += 1;
            }
        }
        if c == '>' {
            in_tag = false;
        }
    }
    markup * 2 >= total
}

pub(super) fn find_text_part(part: &ParsedMail, mime: &str) -> Option<String> {
    if part.ctype.mimetype.eq_ignore_ascii_case(mime) {
        return text_body(part);
    }
    for sub in &part.subparts {
        if let Some(body) = find_text_part(sub, mime) {
            return Some(body);
        }
    }
    None
}

/// Decoded text of a leaf part. A base64 body cut off mid-quantum (a truncated
/// message, a sender that drops the padding) fails to decode as a whole, which
/// would leave the reader with no body at all; decode the complete quanta
/// instead and show what did arrive.
pub(super) fn text_body(part: &ParsedMail) -> Option<String> {
    if let Ok(body) = part.get_body() {
        return Some(body);
    }
    let Body::Base64(encoded) = part.get_body_encoded() else {
        return None;
    };
    let mut cleaned: Vec<u8> = encoded
        .get_raw()
        .iter()
        .copied()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    cleaned.truncate(cleaned.len() / 4 * 4);
    // Nothing complete arrived: leave the part out so an HTML alternative can
    // stand in for it.
    if cleaned.is_empty() {
        return None;
    }
    // Re-parse as a part of its own so `mailparse` applies the charset.
    let mut raw = format!(
        "Content-Type: {}; charset={}\r\nContent-Transfer-Encoding: base64\r\n\r\n",
        part.ctype.mimetype, part.ctype.charset
    )
    .into_bytes();
    raw.extend_from_slice(&cleaned);
    parse_mail(&raw).ok()?.get_body().ok()
}

/// Drop the markup that makes a body readable in the reader but noisy in a
/// one-line snippet: image markers and their links, link syntax (the text is
/// worth showing, the URL isn't), emphasis/code markers, and line-leading
/// quote/list/heading markers. Applies to both bodies we converted from HTML
/// and text/plain parts, which carry the same conventions (Gmail writes
/// `[image: alt] <url>`).
pub(super) fn strip_preview_markup(body: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in body.lines() {
        let mut rest = line.trim();
        loop {
            let stripped = rest
                .strip_prefix("> ")
                .or_else(|| rest.strip_prefix('>'))
                .or_else(|| rest.strip_prefix("- "))
                .or_else(|| rest.strip_prefix("* "))
                .or_else(|| rest.strip_prefix("+ "))
                .or_else(|| {
                    rest.starts_with('#')
                        .then(|| rest.trim_start_matches('#').trim_start())
                });
            match stripped {
                Some(next) if next != rest => rest = next.trim_start(),
                _ => break,
            }
        }
        // Horizontal rules and other separator runs carry nothing in a snippet.
        if !rest.is_empty() && rest.chars().all(|c| matches!(c, '-' | '*' | '_' | '=')) {
            continue;
        }
        let stripped = strip_inline_markup(rest);
        if !stripped.trim().is_empty() {
            lines.push(stripped);
        }
    }
    lines.join(" ")
}

/// Inline half of [`strip_preview_markup`], run per line.
pub(super) fn strip_inline_markup(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find(['[', '*', '`', '<']) {
        out.push_str(&rest[..open]);
        let tail = &rest[open..];
        // A backslash-escaped delimiter is literal text, not markup.
        if is_escaped(&rest[..open]) {
            out.push_str(&tail[..1]);
            rest = &tail[1..];
            continue;
        }
        match tail.as_bytes()[0] {
            b'[' => {
                let Some(close) = tail.find(']') else {
                    out.push('[');
                    rest = &tail[1..];
                    continue;
                };
                let label = &tail[1..close];
                let after = &tail[close + 1..];
                // `[text](url)` / `[text]<url>` keeps the text and drops the
                // destination; a bare `[text]` keeps the brackets (it may be a
                // literal like `[Attachment: x]`).
                let (after, linked) = match strip_link_target(after) {
                    Some(next) => (next, true),
                    None => (after, false),
                };
                if let Some(alt) = label.strip_prefix("image: ") {
                    // The marker stands in for a picture; in a snippet even the
                    // alt text is usually chrome ("Company Logo"), so drop it.
                    let _ = alt;
                } else if linked {
                    out.push_str(label);
                } else {
                    out.push('[');
                    out.push_str(label);
                    out.push(']');
                }
                rest = after;
            }
            b'<' => {
                // Autolink: keep the URL, drop the brackets. Anything else stays
                // (`<escaped>` text, `a < b`).
                match tail.find('>') {
                    Some(close) if is_autolink(&tail[1..close]) => {
                        out.push_str(&tail[1..close]);
                        rest = &tail[close + 1..];
                    }
                    _ => {
                        out.push('<');
                        rest = &tail[1..];
                    }
                }
            }
            // Emphasis (`*bold*`, `**x**`) and code spans: strip the delimiters
            // only when they actually pair up. Bodies here are often text/plain,
            // where a lone `*` is literal (`2 * 3`, `*.rs`) and must survive.
            marker => {
                let run = tail.bytes().take_while(|byte| *byte == marker).count();
                let inner = &tail[run..];
                match closing_run(inner, marker, run) {
                    Some(close) if marker == b'`' => {
                        // Code spans are literal: keep the content untouched.
                        out.push_str(&inner[..close]);
                        rest = &inner[close + run..];
                    }
                    Some(close) => {
                        out.push_str(&strip_inline_markup(&inner[..close]));
                        rest = &inner[close + run..];
                    }
                    None => {
                        out.push_str(&tail[..run]);
                        rest = inner;
                    }
                }
            }
        }
    }
    out.push_str(rest);
    out
}

/// Whether the delimiter right after `before` is backslash-escaped, i.e. an odd
/// number of backslashes precedes it (`\\*` is a literal backslash then markup).
pub(super) fn is_escaped(before: &str) -> bool {
    before
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'\\')
        .count()
        % 2
        == 1
}

/// Byte offset of the delimiter run that closes an emphasis/code span opened by
/// `run` copies of `marker`, or `None` when the opener is unpaired. Follows the
/// CommonMark shape loosely: the span may not start or end on whitespace, and
/// the closing run must be the same length (so `**bold**` doesn't close on the
/// first `*`).
pub(super) fn closing_run(inner: &str, marker: u8, run: usize) -> Option<usize> {
    let bytes = inner.as_bytes();
    if bytes.first().is_none_or(|byte| byte.is_ascii_whitespace()) {
        return None;
    }
    let mut idx = 1;
    while idx < bytes.len() {
        if bytes[idx] != marker {
            idx += 1;
            continue;
        }
        let len = bytes[idx..]
            .iter()
            .take_while(|byte| **byte == marker)
            .count();
        if len == run && !bytes[idx - 1].is_ascii_whitespace() && !is_escaped(&inner[..idx]) {
            return Some(idx);
        }
        idx += len;
    }
    None
}

/// Consume a link destination that follows a `]`, i.e. `(url)`, `<url>`, or
/// ` <url>`. Returns the remainder when one was there.
pub(super) fn strip_link_target(after: &str) -> Option<&str> {
    if let Some(paren) = after.strip_prefix('(') {
        // Destinations may nest parens (`.../Foo_(bar)`) or escape them, so match
        // the closer by depth rather than taking the first `)`.
        let bytes = paren.as_bytes();
        let mut depth = 1usize;
        let mut idx = 0;
        while idx < bytes.len() {
            match bytes[idx] {
                b'\\' => idx += 1,
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(&paren[idx + 1..]);
                    }
                }
                _ => {}
            }
            idx += 1;
        }
        return None;
    }
    let angle = after
        .strip_prefix(" <")
        .or_else(|| after.strip_prefix('<'))?;
    let end = angle.find('>')?;
    is_autolink(&angle[..end]).then(|| &angle[end + 1..])
}

pub(super) fn is_autolink(inner: &str) -> bool {
    !inner.contains(char::is_whitespace)
        && (inner.contains("://") || inner.starts_with("mailto:") || inner.contains('@'))
}

pub(crate) fn preview_of(body: &str) -> String {
    let stripped = strip_preview_markup(body);
    let collapsed = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut preview: String = collapsed.chars().take(200).collect();
    if collapsed.chars().count() > 200 {
        preview.push('…');
    }
    preview
}
