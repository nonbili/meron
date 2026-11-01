//! What a tag is, and how it maps onto an IMAP keyword.
//!
//! A tag is short lowercase text, so `Work` and `work` are one tag, as they are
//! one keyword to IMAP. Keywords are ASCII atoms, so a tag that an atom cannot
//! spell travels hex-encoded behind [`ENCODED_PREFIX`]; other clients show that
//! one as noise, but it round-trips exactly.

/// Longest tag, in UTF-8 bytes. Hex-encoding doubles it, and that has to stay
/// within what servers accept as a keyword.
pub const MAX_TAG_BYTES: usize = 48;

const ENCODED_PREFIX: &str = "meron-x-";

/// Keywords that mail software sets for its own bookkeeping. Showing them as
/// tags would be noise; `$`-prefixed ones are ruled out the same way.
const SYSTEM_KEYWORDS: &[&str] = &[
    "forwarded",
    "junk",
    "junkrecorded",
    "mdnsent",
    "nonjunk",
    "notjunk",
    "phishing",
];

/// The canonical form of user input: no leading `#`, lowercase, whitespace as
/// `-`. `None` when nothing valid is left.
pub fn normalize_tag(input: &str) -> Option<String> {
    let trimmed = input.trim().trim_start_matches('#').trim();
    let tag = trimmed
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .to_lowercase();
    let valid = !tag.is_empty()
        && tag.len() <= MAX_TAG_BYTES
        && tag
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'));
    valid.then_some(tag)
}

fn is_atom_safe(tag: &str) -> bool {
    tag.is_ascii() && !tag.starts_with(ENCODED_PREFIX) && !SYSTEM_KEYWORDS.contains(&tag)
}

/// The IMAP keyword for a normalized tag.
pub fn tag_to_keyword(tag: &str) -> String {
    if is_atom_safe(tag) {
        return tag.to_string();
    }
    let hex: String = tag.bytes().map(|byte| format!("{byte:02x}")).collect();
    format!("{ENCODED_PREFIX}{hex}")
}

/// The tag a server keyword stands for, or `None` for a keyword that is not one
/// (system bookkeeping, or a spelling no tag has).
pub fn keyword_to_tag(keyword: &str) -> Option<String> {
    let keyword = keyword.to_ascii_lowercase();
    if keyword.starts_with('$') || SYSTEM_KEYWORDS.contains(&keyword.as_str()) {
        return None;
    }
    if let Some(hex) = keyword.strip_prefix(ENCODED_PREFIX) {
        let bytes = decode_hex(hex)?;
        let tag = String::from_utf8(bytes).ok()?;
        return (normalize_tag(&tag).as_deref() == Some(tag.as_str())).then_some(tag);
    }
    (normalize_tag(&keyword).as_deref() == Some(keyword.as_str())).then_some(keyword)
}

fn decode_hex(hex: &str) -> Option<Vec<u8>> {
    if hex.is_empty() || !hex.len().is_multiple_of(2) || !hex.is_ascii() {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).ok())
        .collect()
}

/// A request's list of tags, normalized, sorted and deduplicated. An invalid
/// one is an error: dropping it would report success for a tag never applied.
pub fn parse_tag_list(value: Option<&serde_json::Value>) -> Result<Vec<String>, String> {
    let mut tags = Vec::new();
    for item in value
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
    {
        let input = item.as_str().unwrap_or_default();
        tags.push(normalize_tag(input).ok_or_else(|| format!("invalid tag: {input}"))?);
    }
    tags.sort();
    tags.dedup();
    Ok(tags)
}

/// A request to rename a tag everywhere, or to delete it when `to` is left out or
/// null: the tag, and what it becomes. Any other `to` that spells no tag is an
/// error, never a delete, so clearing a rename field can't take a tag off every
/// message.
pub fn parse_tag_replacement(
    params: &serde_json::Value,
) -> Result<(String, Option<String>), String> {
    let spell = |input: &str| normalize_tag(input).ok_or_else(|| format!("invalid tag: {input}"));
    let tag = spell(
        params
            .get("tag")
            .and_then(|tag| tag.as_str())
            .unwrap_or_default(),
    )?;
    let to = match params.get("to") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(to)) => Some(spell(to)?),
        Some(other) => return Err(format!("invalid tag: {other}")),
    };
    Ok((tag, to))
}

/// The answer's `queries`: each saved search the request sent, in order, as it
/// reads after the replacement. Boards are kept by the app, so this is how their
/// columns follow a rename without a second reader of the search syntax.
pub fn renamed_queries(params: &serde_json::Value, tag: &str, to: Option<&str>) -> Vec<String> {
    params
        .get("queries")
        .and_then(|queries| queries.as_array())
        .into_iter()
        .flatten()
        .map(|query| {
            let query = query.as_str().unwrap_or_default();
            match to {
                Some(to) => crate::search_query::rename_tag_terms(query, tag, to),
                None => query.to_string(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_missing_name_deletes_a_tag() {
        let parse = |params| parse_tag_replacement(&params);
        assert_eq!(
            parse(serde_json::json!({ "tag": "#Old", "to": "New Name" })),
            Ok(("old".to_string(), Some("new-name".to_string())))
        );
        assert_eq!(
            parse(serde_json::json!({ "tag": "old" })),
            Ok(("old".to_string(), None))
        );
        assert_eq!(
            parse(serde_json::json!({ "tag": "old", "to": null })),
            Ok(("old".to_string(), None))
        );
        assert!(parse(serde_json::json!({ "tag": "old", "to": "" })).is_err());
        assert!(parse(serde_json::json!({ "tag": "old", "to": "  " })).is_err());
        assert!(parse(serde_json::json!({ "tag": "old", "to": 123 })).is_err());
    }

    #[test]
    fn saved_searches_come_back_in_order_and_follow_only_a_rename() {
        let params = serde_json::json!({ "queries": ["tag:old", "from:a"] });
        assert_eq!(
            renamed_queries(&params, "old", Some("new")),
            ["tag:new", "from:a"]
        );
        assert_eq!(renamed_queries(&params, "old", None), ["tag:old", "from:a"]);
    }

    #[test]
    fn input_normalizes_to_one_spelling() {
        assert_eq!(
            normalize_tag(" #Work  Stuff ").as_deref(),
            Some("work-stuff")
        );
        assert_eq!(normalize_tag("重要").as_deref(), Some("重要"));
        assert_eq!(normalize_tag("#"), None);
        assert_eq!(normalize_tag("a:b"), None);
        assert_eq!(normalize_tag(&"x".repeat(MAX_TAG_BYTES + 1)), None);
    }

    #[test]
    fn tags_round_trip_through_keywords() {
        for tag in ["work", "q3.plan_a-b", "重要", "junk", "meron-x-6869"] {
            let keyword = tag_to_keyword(tag);
            assert!(keyword.is_ascii(), "{keyword}");
            assert_eq!(keyword_to_tag(&keyword).as_deref(), Some(tag));
        }
        assert_eq!(tag_to_keyword("work"), "work");
        assert_eq!(tag_to_keyword("重要"), "meron-x-e9878de8a681");
    }

    #[test]
    fn foreign_keywords_are_tags_only_when_a_tag_could_spell_them() {
        assert_eq!(keyword_to_tag("Todo").as_deref(), Some("todo"));
        assert_eq!(keyword_to_tag("$label1"), None);
        assert_eq!(keyword_to_tag("NonJunk"), None);
        assert_eq!(keyword_to_tag("a/b"), None);
        assert_eq!(keyword_to_tag("meron-x-zz"), None);
        assert_eq!(keyword_to_tag("meron-x-41"), None, "not normalized");
    }
}
