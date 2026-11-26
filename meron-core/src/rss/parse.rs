use super::{ParsedItem, stable_hash};
use crate::store::{RssItemExtra, RssMedia};

pub(super) fn parse_items(feed: &feed_rs::model::Feed, fetched_at: i64) -> Vec<ParsedItem> {
    feed.entries
        .iter()
        .filter_map(|entry| parse_item(entry, fetched_at))
        .collect()
}

pub(super) fn parse_item(entry: &feed_rs::model::Entry, fetched_at: i64) -> Option<ParsedItem> {
    let mut title = text_content(&entry.title).trim().to_string();
    if title.is_empty() {
        title = "(untitled)".to_string();
    }
    let author = entry
        .authors
        .first()
        .map(|p| p.name.trim().to_string())
        .unwrap_or_default();
    let summary_html = text_content(&entry.summary);
    let content_html = entry
        .content
        .as_ref()
        .and_then(|c| c.body.clone())
        .unwrap_or_default();
    let summary = clean_feed_text(&summary_html);
    let content = clean_feed_text(&content_html);
    // Lift inline <img> URLs out of the HTML (stripped by clean_feed_text) so the
    // UI can show them as images. Union content + summary, content first. Keys are
    // filled in later by the image cacher (sync time) for image-enabled accounts.
    let mut image_urls = extract_image_urls(&content_html);
    for url in extract_image_urls(&summary_html) {
        if !image_urls.iter().any(|existing| existing == &url) {
            image_urls.push(url);
        }
    }
    let images = image_urls
        .into_iter()
        .map(|url| RssMedia { url, key: None })
        .collect();
    // Lift inline <video>/<source> URLs out of the HTML the same way. These are
    // rendered straight from their remote URL (no local caching), so they only
    // ever carry a `url`.
    let mut video_urls = extract_video_urls(&content_html);
    for url in extract_video_urls(&summary_html) {
        if !video_urls.iter().any(|existing| existing == &url) {
            video_urls.push(url);
        }
    }
    let videos = video_urls
        .into_iter()
        .map(|url| RssMedia { url, key: None })
        .collect();
    let link = entry
        .links
        .first()
        .map(|l| l.href.trim().to_string())
        .unwrap_or_default();
    let published_at = entry.published.map(|d| d.timestamp()).unwrap_or(0);
    let updated_at = entry.updated.map(|d| d.timestamp()).unwrap_or(0);

    // Keep the raw HTML (content preferred, summary as fallback) for HTML mode.
    let content_html = if !content_html.trim().is_empty() {
        Some(content_html)
    } else if !summary_html.trim().is_empty() {
        Some(summary_html)
    } else {
        None
    };

    let key_source = if !entry.id.trim().is_empty() {
        entry.id.trim().to_string()
    } else if !link.is_empty() {
        link.clone()
    } else {
        format!("{title}|{published_at}|{updated_at}")
    };

    Some(ParsedItem {
        item_key: stable_hash(&key_source),
        title,
        unread: true,
        content_html,
        extra: RssItemExtra {
            author,
            link,
            summary,
            content,
            images,
            videos,
            published_at,
            updated_at,
            fetched_at,
        },
    })
}

pub(super) fn text_content(text: &Option<feed_rs::model::Text>) -> String {
    text.as_ref().map(|t| t.content.clone()).unwrap_or_default()
}

pub(super) fn clean_feed_text(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return String::new();
    }
    // Replace block-level HTML tags with newlines to preserve structural breaks.
    let normalized = value
        .replace("<p>", "\n")
        .replace("</p>", "\n")
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</div>", "\n")
        .replace("</li>", "\n")
        .replace("</td>", "\n")
        .replace("</tr>", "\n")
        .replace("</h1>", "\n")
        .replace("</h2>", "\n")
        .replace("</h3>", "\n")
        .replace("</h4>", "\n")
        .replace("</h5>", "\n")
        .replace("</h6>", "\n")
        .replace("<blockquote>", "\n")
        .replace("</blockquote>", "\n");

    let fragment = scraper::Html::parse_fragment(&normalized);
    // Concatenate text nodes without separators (like goquery's .Text()) so we
    // don't insert spaces before punctuation; normalize spacing and newlines after.
    let text: String = fragment.root_element().text().collect();
    normalize_newlines_and_spaces(&text)
}

fn normalize_newlines_and_spaces(text: &str) -> String {
    let mut result = String::new();
    let mut consecutive_newlines = 0;

    for line in text.lines() {
        let trimmed_line = line.trim();
        if trimmed_line.is_empty() {
            consecutive_newlines += 1;
        } else {
            if !result.is_empty() {
                if consecutive_newlines > 1 {
                    result.push_str("\n\n");
                } else {
                    result.push('\n');
                }
            }
            consecutive_newlines = 0;

            // Collapse multiple spaces within the line
            let collapsed_line = trimmed_line
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            result.push_str(&collapsed_line);
        }
    }
    result.trim().to_string()
}

/// Collect `<img src>` URLs from a feed item's HTML, in document order, deduped.
/// Skips `data:` URIs (inline base64 would bloat the cache) and empty srcs.
pub(super) fn extract_image_urls(html: &str) -> Vec<String> {
    let html = html.trim();
    if html.is_empty() {
        return Vec::new();
    }
    let fragment = scraper::Html::parse_fragment(html);
    let Ok(selector) = scraper::Selector::parse("img") else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for el in fragment.select(&selector) {
        let Some(src) = el.value().attr("src") else {
            continue;
        };
        let src = src.trim();
        if src.is_empty() || src.starts_with("data:") {
            continue;
        }

        let mut is_tracker = false;

        // 1. Check width and height attributes
        if let (Some(w), Some(h)) = (el.value().attr("width"), el.value().attr("height")) {
            let w = w.trim();
            let h = h.trim();
            if (w == "1" || w == "0" || w == "2") && (h == "1" || h == "0" || h == "2") {
                is_tracker = true;
            }
        }

        // 2. Check inline style attributes
        if let Some(style) = el.value().attr("style") {
            let s = style.to_lowercase();
            if s.contains("display:none")
                || s.contains("display: none")
                || s.contains("visibility:hidden")
                || s.contains("visibility: hidden")
            {
                is_tracker = true;
            }
            let has_tiny_w = s.contains("width:0px")
                || s.contains("width: 0px")
                || s.contains("width:1px")
                || s.contains("width: 1px")
                || s.contains("width:2px")
                || s.contains("width: 2px");
            let has_tiny_h = s.contains("height:0px")
                || s.contains("height: 0px")
                || s.contains("height:1px")
                || s.contains("height: 1px")
                || s.contains("height:2px")
                || s.contains("height: 2px");
            if has_tiny_w && has_tiny_h {
                is_tracker = true;
            }
        }

        // 3. Check URL pattern heuristics
        let lower_src = src.to_lowercase();
        if lower_src.contains("/open/")
            || lower_src.contains("/track")
            || lower_src.contains("/pixel")
            || lower_src.contains("pixel.gif")
            || lower_src.contains("cleardot.gif")
            || lower_src.contains("spacer.gif")
            || lower_src.contains("/wf/open")
            || lower_src.contains("/open.php")
            || lower_src.contains("utm_")
            || lower_src.contains("bounce")
        {
            is_tracker = true;
        }

        if is_tracker {
            continue;
        }

        if !out.iter().any(|existing| existing == src) {
            out.push(src.to_string());
        }
    }
    out
}

/// Collect inline video URLs from a feed item's HTML, in document order, deduped.
/// Handles both `<video src>` and `<video><source src></video>`. Skips `data:`
/// URIs and empty srcs.
pub(super) fn extract_video_urls(html: &str) -> Vec<String> {
    let html = html.trim();
    if html.is_empty() {
        return Vec::new();
    }
    let fragment = scraper::Html::parse_fragment(html);
    let Ok(selector) = scraper::Selector::parse("video, video source") else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for el in fragment.select(&selector) {
        let Some(src) = el.value().attr("src") else {
            continue;
        };
        let src = src.trim();
        if src.is_empty() || src.starts_with("data:") {
            continue;
        }
        if !out.iter().any(|existing| existing == src) {
            out.push(src.to_string());
        }
    }
    out
}

/// The trailing path segment of an image URL, used as its display filename.
pub(super) fn image_filename(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| {
            u.path_segments()
                .and_then(|mut segs| segs.next_back().map(str::to_string))
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "image".to_string())
}

/// Best-effort MIME guess from an image URL's extension (display only).
pub(super) fn image_mime(url: &str) -> &'static str {
    let lower = url.split(['?', '#']).next().unwrap_or(url).to_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".svg") {
        "image/svg+xml"
    } else {
        "image/jpeg"
    }
}

/// Best-effort MIME guess from a video URL's extension (display only); defaults
/// to `video/mp4` when the URL has no recognizable extension.
pub(super) fn video_mime(url: &str) -> &'static str {
    let lower = url.split(['?', '#']).next().unwrap_or(url).to_lowercase();
    if lower.ends_with(".webm") {
        "video/webm"
    } else if lower.ends_with(".ogv") || lower.ends_with(".ogg") {
        "video/ogg"
    } else if lower.ends_with(".mov") {
        "video/quicktime"
    } else {
        "video/mp4"
    }
}
