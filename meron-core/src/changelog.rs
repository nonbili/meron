//! In-app changelog: fetch the GitHub releases atom feed and present each
//! release as a version + date + cleaned notes.
//!
//! Desktop and mobile ship from the same repo but tag releases differently —
//! desktop tags are `v*` (e.g. `v0.1.4`), mobile tags are `android/v*` (e.g.
//! `android/v0.1.3`). The caller passes a [`Variant`] so each platform only
//! sees its own releases.
//!
//! GitHub's auto-generated notes end with a "Full Changelog: <compare-url>"
//! line; we drop it so the in-app view shows only the human-written bullets.

use anyhow::{Context, Result};
use scraper::{Html, Selector};
use serde_json::{Value, json};
use std::time::Duration;

const RELEASES_ATOM_URL: &str = "https://github.com/nonbili/meron/releases.atom";
const ANDROID_TAG_PREFIX: &str = "android/";
const HTTP_TIMEOUT: Duration = Duration::from_secs(20);

/// Which platform's releases to surface. Tags that don't match are skipped.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Desktop,
    Android,
}

impl Variant {
    pub fn parse(value: &str) -> Variant {
        match value {
            "android" => Variant::Android,
            _ => Variant::Desktop,
        }
    }

    /// Whether a release tag belongs to this variant.
    fn matches(&self, tag: &str) -> bool {
        match self {
            Variant::Android => tag.starts_with(ANDROID_TAG_PREFIX),
            // Desktop tags are `v*` and never carry the mobile prefix.
            Variant::Desktop => tag.starts_with('v') && !tag.starts_with(ANDROID_TAG_PREFIX),
        }
    }
}

/// Fetch and parse the releases atom feed, returning the releases for `variant`
/// as `{ "releases": [ { version, tag, date, notes: [..] } ] }`. Newest first
/// (GitHub serves the feed in that order).
pub fn fetch(variant: Variant) -> Result<Value> {
    let body = http_get(RELEASES_ATOM_URL)?;
    let feed = feed_rs::parser::parse(&body[..]).context("parse releases atom feed")?;

    let releases: Vec<Value> = feed
        .entries
        .iter()
        .filter_map(|entry| {
            let tag = entry_tag(entry)?;
            if !variant.matches(&tag) {
                return None;
            }
            let version = tag
                .strip_prefix(ANDROID_TAG_PREFIX)
                .unwrap_or(&tag)
                .to_string();
            let date = entry
                .updated
                .or(entry.published)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default();
            let notes = entry
                .content
                .as_ref()
                .and_then(|content| content.body.as_deref())
                .map(extract_notes)
                .unwrap_or_default();
            Some(json!({
                "version": version,
                "tag": tag,
                "date": date,
                "notes": notes,
            }))
        })
        .collect();

    Ok(json!({ "releases": releases }))
}

/// The release tag for an atom entry. GitHub's entry links point at
/// `.../releases/tag/<tag>`; the tag itself may contain a slash
/// (`android/v0.1.3`), so we take everything after `/releases/tag/` rather than
/// the last path segment.
fn entry_tag(entry: &feed_rs::model::Entry) -> Option<String> {
    const MARKER: &str = "/releases/tag/";
    for link in &entry.links {
        if let Some(idx) = link.href.find(MARKER) {
            let raw = &link.href[idx + MARKER.len()..];
            let tag = raw.trim_end_matches('/').replace("%2F", "/");
            if !tag.is_empty() {
                return Some(tag);
            }
        }
    }
    // Fall back to the entry id: `tag:github.com,2008:Repository/<id>/<tag>`.
    let id = &entry.id;
    id.rfind("Repository/").map(|idx| {
        let rest = &id[idx + "Repository/".len()..];
        // Drop the numeric repo id, keep the (possibly slash-bearing) tag.
        rest.splitn(2, '/').nth(1).unwrap_or(rest).to_string()
    })
}

/// Turn the release-note HTML into a flat list of bullet strings, dropping the
/// auto-generated "Full Changelog" line. Prefers `<li>` items (the usual bullet
/// list); falls back to `<p>` paragraphs when there's no list.
fn extract_notes(html: &str) -> Vec<String> {
    let fragment = Html::parse_fragment(html);
    let li = Selector::parse("li").unwrap();
    let mut notes: Vec<String> = fragment
        .select(&li)
        .map(|el| normalize_ws(&el.text().collect::<String>()))
        .filter(|line| !line.is_empty() && !is_full_changelog(line))
        .collect();
    if notes.is_empty() {
        let p = Selector::parse("p").unwrap();
        notes = fragment
            .select(&p)
            .map(|el| normalize_ws(&el.text().collect::<String>()))
            .filter(|line| !line.is_empty() && !is_full_changelog(line))
            .collect();
    }
    notes
}

fn is_full_changelog(line: &str) -> bool {
    line.trim_start().starts_with("Full Changelog")
}

/// Collapse runs of whitespace (incl. newlines from the HTML) into single
/// spaces and trim, so each bullet renders as one clean line.
fn normalize_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn http_get(url: &str) -> Result<Vec<u8>> {
    let mut resp = ureq::get(url)
        .config()
        .timeout_global(Some(HTTP_TIMEOUT))
        .build()
        .call()
        .with_context(|| format!("HTTP GET {url}"))?;
    resp.body_mut()
        .read_to_vec()
        .with_context(|| format!("read body {url}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_matches_tags() {
        assert!(Variant::Desktop.matches("v0.1.4"));
        assert!(!Variant::Desktop.matches("android/v0.1.3"));
        assert!(Variant::Android.matches("android/v0.1.3"));
        assert!(!Variant::Android.matches("v0.1.4"));
    }

    #[test]
    fn extract_notes_drops_full_changelog() {
        let html = "<ul>\n<li>Improve add column dialog</li>\n<li>Fix Google OAuth sign-in</li>\n</ul>\n<p><strong>Full Changelog</strong>: <a href=\"x\">android/v0.1.2...android/v0.1.3</a></p>";
        let notes = extract_notes(html);
        assert_eq!(
            notes,
            vec![
                "Improve add column dialog".to_string(),
                "Fix Google OAuth sign-in".to_string(),
            ]
        );
    }

    #[test]
    fn extract_notes_falls_back_to_paragraphs() {
        let html = "<p>Initial release.</p><p>Full Changelog: foo</p>";
        let notes = extract_notes(html);
        assert_eq!(notes, vec!["Initial release.".to_string()]);
    }

    #[test]
    fn entry_tag_keeps_android_prefix() {
        let mut entry = feed_rs::model::Entry::default();
        entry.links = vec![feed_rs::model::Link {
            href: "https://github.com/nonbili/meron/releases/tag/android/v0.1.3".to_string(),
            rel: None,
            media_type: None,
            href_lang: None,
            title: None,
            length: None,
        }];
        assert_eq!(entry_tag(&entry).as_deref(), Some("android/v0.1.3"));
    }
}
