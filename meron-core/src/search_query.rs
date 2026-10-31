//! Gmail-style search operators, parsed once and compiled per backend.
//!
//! A query is whitespace-separated terms ANDed together. A term of the form
//! `key:value` with a known key becomes a structured filter; everything else is
//! free text, matched as one substring the way a plain query always was.
//! Values may be double-quoted to hold spaces (`from:"Ann Lee"`). Unknown keys
//! and empty values stay free text, so `re:foo` or a URL still searches as
//! typed.
//!
//! `from:`, `to:` and `subject:` take alternatives the Gmail way — any of them
//! matches: `from:{a b}`, `from:(a OR b)`, `from:a,b`, or `from:a OR from:b`.
//! `OR` between other terms, negation and general grouping are not supported.
//!
//! A bare uppercase `AND`, as in Gmail, joins queries that must all match —
//! how a saved search is narrowed by another. Each side is read on its own, so
//! its free text stays its own substring and its dates and read state narrow
//! the other side's rather than replacing them.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// One `from:`/`to:`/`subject:` term's values: a message matches the term if
/// it matches any of them. Never empty.
pub type AnyOf = Vec<String>;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    /// Free text left after operators are taken out, one substring per side
    /// of an `AND`; each must match. Empty when the query is operators only.
    pub text: Vec<String>,
    /// `from:` terms, matched against the sender's name or address.
    pub from: Vec<AnyOf>,
    /// `to:` terms, matched against To or Cc.
    pub to: Vec<AnyOf>,
    /// `subject:` terms.
    pub subject: Vec<AnyOf>,
    /// `is:read`. Within a side the last read-state term wins; across sides
    /// both may be set, which nothing matches.
    pub read: bool,
    /// `is:unread`.
    pub unread: bool,
    /// `is:starred`.
    pub starred: bool,
    /// `has:attachment`.
    pub has_attachment: bool,
    /// `after:` — sent on or after this day.
    pub after: Option<NaiveDate>,
    /// `before:` — sent before this day.
    pub before: Option<NaiveDate>,
}

impl SearchQuery {
    pub fn parse(query: &str) -> Self {
        let query = query.trim();
        let mut parsed = SearchQuery::default();
        for side in sides(clauses(query)) {
            parsed.narrow(query, side);
        }
        parsed
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty() && !self.has_operators()
    }

    pub fn has_operators(&self) -> bool {
        !self.from.is_empty()
            || !self.to.is_empty()
            || !self.subject.is_empty()
            || self.read
            || self.unread
            || self.starred
            || self.has_attachment
            || self.after.is_some()
            || self.before.is_some()
    }

    /// Add one side of an `AND` of `query`: everything both sides ask for
    /// must hold.
    fn narrow(&mut self, query: &str, side: Vec<Clause<'_>>) {
        let (Some(first), Some(last)) = (side.first(), side.last()) else {
            return;
        };
        // A side with no operators keeps its exact spacing, so plain search
        // behaves as it always did.
        let plain = side.iter().all(|clause| clause.operator.is_none());
        let whole = span(query, first.raw, last.raw);
        let mut text = Vec::new();
        let mut seen = None;
        for clause in side {
            match clause.operator {
                Some(Operator::From(value)) => self.from.push(value),
                Some(Operator::To(value)) => self.to.push(value),
                Some(Operator::Subject(value)) => self.subject.push(value),
                Some(Operator::Seen(value)) => seen = Some(value),
                Some(Operator::Starred) => self.starred = true,
                Some(Operator::HasAttachment) => self.has_attachment = true,
                Some(Operator::After(date)) => self.after = self.after.max(Some(date)),
                Some(Operator::Before(date)) => {
                    self.before = Some(self.before.map_or(date, |before| before.min(date)))
                }
                None => text.push(clause.raw),
            }
        }
        let text = if plain {
            whole.to_string()
        } else {
            text.join(" ")
        };
        if !text.is_empty() {
            self.text.push(text);
        }
        match seen {
            Some(true) => self.read = true,
            Some(false) => self.unread = true,
            None => {}
        }
    }
}

/// A bare `AND`, joining two sides of a query. Gmail's is uppercase, as `OR`.
fn is_and(clause: &Clause<'_>) -> bool {
    clause.operator.is_none() && clause.raw == "AND"
}

/// The clauses of each side of a bare `AND`; an empty side is dropped.
fn sides(clauses: Vec<Clause<'_>>) -> Vec<Vec<Clause<'_>>> {
    let mut sides = vec![Vec::new()];
    for clause in clauses {
        if is_and(&clause) {
            sides.push(Vec::new());
        } else if let Some(side) = sides.last_mut() {
            side.push(clause);
        }
    }
    sides.retain(|side| !side.is_empty());
    sides
}

/// One recognised `key:value` term.
enum Operator {
    From(AnyOf),
    To(AnyOf),
    Subject(AnyOf),
    Seen(bool),
    Starred,
    HasAttachment,
    After(NaiveDate),
    Before(NaiveDate),
}

impl Operator {
    fn parse(term: &str) -> Option<Self> {
        let (key, raw) = term.split_once(':')?;
        let value = unquote(raw);
        if value.is_empty() {
            return None;
        }
        let any = || Some(alternatives(raw)).filter(|any| !any.is_empty());
        Some(match key.to_ascii_lowercase().as_str() {
            "from" => Operator::From(any()?),
            "to" => Operator::To(any()?),
            "subject" => Operator::Subject(any()?),
            "is" => match value.to_ascii_lowercase().as_str() {
                "unread" => Operator::Seen(false),
                "read" => Operator::Seen(true),
                "starred" => Operator::Starred,
                _ => return None,
            },
            "has" if value.eq_ignore_ascii_case("attachment") => Operator::HasAttachment,
            "after" => Operator::After(parse_date(value)?),
            "before" => Operator::Before(parse_date(value)?),
            _ => return None,
        })
    }

    /// `self OR other`, for two terms of the same key that take alternatives.
    fn or(self, other: Operator) -> Result<Operator, (Operator, Operator)> {
        match (self, other) {
            (Operator::From(mut a), Operator::From(b)) => Ok(Operator::From({
                a.extend(b);
                a
            })),
            (Operator::To(mut a), Operator::To(b)) => Ok(Operator::To({
                a.extend(b);
                a
            })),
            (Operator::Subject(mut a), Operator::Subject(b)) => Ok(Operator::Subject({
                a.extend(b);
                a
            })),
            pair => Err(pair),
        }
    }
}

/// A term of the query as written, and the operator it is, if any. Terms
/// joined by `OR` that [`Operator::or`] can merge are one clause.
struct Clause<'a> {
    raw: &'a str,
    operator: Option<Operator>,
}

fn clauses(query: &str) -> Vec<Clause<'_>> {
    let terms: Vec<&str> = terms(query).collect();
    let mut clauses: Vec<Clause<'_>> = Vec::new();
    let mut i = 0;
    while i < terms.len() {
        let term = terms[i];
        i += 1;
        // Gmail's `OR` is uppercase; anything else is a word.
        if term == "OR"
            && let Some(next) = terms.get(i).and_then(|next| Operator::parse(next))
            && let Some(last) = clauses.last_mut()
            && let Some(operator) = last.operator.take()
        {
            match operator.or(next) {
                Ok(merged) => {
                    last.operator = Some(merged);
                    last.raw = span(query, last.raw, terms[i]);
                    i += 1;
                    continue;
                }
                Err((operator, _)) => last.operator = Some(operator),
            }
        }
        clauses.push(Clause {
            raw: term,
            operator: Operator::parse(term),
        });
    }
    clauses
}

/// The slice of `query` from the start of `first` to the end of `last`, both
/// slices of it.
fn span<'a>(query: &'a str, first: &str, last: &str) -> &'a str {
    let start = first.as_ptr() as usize - query.as_ptr() as usize;
    let end = last.as_ptr() as usize - query.as_ptr() as usize + last.len();
    &query[start..end]
}

/// The alternatives in an operator's value: `{a b}` (Gmail's any-of), `(a OR
/// b)` and `a,b` (`OR`, `|` or `,` between them, so a parenthesized `(Ann
/// Lee)` is one name), or one quoted or bare value.
fn alternatives(value: &str) -> AnyOf {
    let (inner, braces) = if let Some(inner) = value.strip_prefix('{') {
        (inner.strip_suffix('}').unwrap_or(inner), true)
    } else if let Some(inner) = value.strip_prefix('(') {
        (inner.strip_suffix(')').unwrap_or(inner), false)
    } else if value.starts_with('"')
        && value[1..]
            .find('"')
            .is_none_or(|end| end + 2 == value.len())
    {
        // One quoted value; `"Ann Lee",bob` is a list.
        return vec![unquote(value).to_string()];
    } else {
        (value, false)
    };
    split_alternatives(inner, braces)
}

/// The alternatives in a group's inside: in braces each word is one, in
/// parentheses (and a form field) the words between separators are.
fn split_alternatives(inner: &str, braces: bool) -> AnyOf {
    let mut any = Vec::new();
    let mut words: Vec<&str> = Vec::new();
    let mut flush = |words: &mut Vec<&str>| {
        if !words.is_empty() {
            any.push(words.join(" "));
            words.clear();
        }
    };
    let pieces = pieces(inner);
    for (i, piece) in pieces.iter().enumerate() {
        // `OR` separates only between two values; anywhere else it is one.
        let piece = match piece {
            Piece::Or
                if !matches!(pieces.get(i.wrapping_sub(1)), Some(Piece::Word(_)))
                    || !matches!(pieces.get(i + 1), Some(Piece::Word(_))) =>
            {
                &Piece::Word("OR")
            }
            piece => piece,
        };
        match *piece {
            Piece::Separator | Piece::Or => flush(&mut words),
            Piece::Word(word) => {
                words.push(word);
                if braces {
                    flush(&mut words);
                }
            }
        }
    }
    flush(&mut words);
    any
}

enum Piece<'a> {
    Word(&'a str),
    Separator,
    /// A bare `OR`: a separator if it stands between two words.
    Or,
}

/// Split a group's inside into words (a quoted span is one, its quotes
/// dropped) and the `OR`, `|` and `,` separators between them.
fn pieces(inner: &str) -> Vec<Piece<'_>> {
    fn word<'a>(pieces: &mut Vec<Piece<'a>>, text: &'a str, quoted: bool) {
        match text.trim() {
            "" => {}
            "OR" if !quoted => pieces.push(Piece::Or),
            text => pieces.push(Piece::Word(text)),
        }
    }
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut chars = inner.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => {
                word(&mut pieces, &inner[start..i], false);
                let end = chars
                    .by_ref()
                    .find(|&(_, c)| c == '"')
                    .map_or(inner.len(), |(end, _)| end);
                word(&mut pieces, &inner[i + 1..end], true);
                start = end + 1;
            }
            ',' | '|' => {
                word(&mut pieces, &inner[start..i], false);
                pieces.push(Piece::Separator);
                start = i + 1;
            }
            c if c.is_whitespace() => {
                word(&mut pieces, &inner[start..i], false);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    word(&mut pieces, &inner[start.min(inner.len())..], false);
    pieces
}

/// The search options form: one field per operator, read from and written
/// back to query text so the text stays the single source of truth. A term the
/// form has no field for (a second `from:`, `is:read`) is kept verbatim in
/// `words`, so opening the form and searching again never drops part of a
/// query. From, To and Subject list a term's alternatives comma-separated, as
/// Gmail's form does.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchFields {
    pub from: String,
    pub to: String,
    pub subject: String,
    /// Free text, plus any terms the other fields can't hold.
    pub words: String,
    /// `yyyy-mm-dd`, or empty.
    pub after: String,
    /// `yyyy-mm-dd`, or empty.
    pub before: String,
    pub unread: bool,
    pub starred: bool,
    pub has_attachment: bool,
}

impl SearchFields {
    pub fn parse(query: &str) -> Self {
        let query = query.trim();
        let mut fields = SearchFields::default();
        let mut words = Vec::new();
        let mut taken = false;
        // The fields hold the first side of an `AND`; the rest stays verbatim
        // in the words, where its read state keeps narrowing the fields'.
        let mut head = clauses(query);
        let tail = head.iter().position(is_and).map(|at| {
            let rest = head.split_off(at);
            span(query, rest[0].raw, rest[rest.len() - 1].raw)
        });
        // The last read-state term decides, as in `SearchQuery`. Only that one
        // survives: `is:unread` in its checkbox, `is:read` in the words, so the
        // rebuilt query can't put them back in an order that flips it.
        let seen = head.iter().rev().find_map(|clause| match clause.operator {
            Some(Operator::Seen(seen)) => Some(seen),
            _ => None,
        });
        for clause in head {
            let slot = match clause.operator {
                Some(Operator::From(any)) => fill(&mut fields.from, field_text(&any)),
                Some(Operator::To(any)) => fill(&mut fields.to, field_text(&any)),
                Some(Operator::Subject(any)) => fill(&mut fields.subject, field_text(&any)),
                Some(Operator::After(date)) => fill(&mut fields.after, iso(date)),
                Some(Operator::Before(date)) => fill(&mut fields.before, iso(date)),
                Some(Operator::Seen(false)) => {
                    fields.unread |= seen == Some(false);
                    true
                }
                Some(Operator::Seen(true)) => seen != Some(true),
                Some(Operator::Starred) => {
                    fields.starred = true;
                    true
                }
                Some(Operator::HasAttachment) => {
                    fields.has_attachment = true;
                    true
                }
                None => false,
            };
            if slot {
                taken = true;
            } else {
                words.push(clause.raw);
            }
        }
        words.extend(tail);
        // As in `SearchQuery::parse`, a plain query keeps its exact spacing.
        fields.words = if taken {
            words.join(" ")
        } else {
            query.to_string()
        };
        fields
    }

    /// The query text for these fields: operators, then the words, then
    /// `is:unread` — last, so the checkbox wins over an `is:read` kept in the
    /// words, as the last read-state term is the one that counts. Last on the
    /// first side, that is: words from an `AND` on follow it.
    pub fn to_query(&self) -> String {
        let mut terms = Vec::new();
        for (key, value) in [
            ("from", &self.from),
            ("to", &self.to),
            ("subject", &self.subject),
        ] {
            let any: Vec<String> = split_alternatives(value, false)
                .iter()
                .map(|value| {
                    let value = value.replace('"', "");
                    let plain = !value
                        .contains(|c: char| c.is_whitespace() || ",|{}()".contains(c))
                        && value != "OR";
                    if plain { value } else { format!("\"{value}\"") }
                })
                .filter(|value| !value.is_empty())
                .collect();
            match any.as_slice() {
                [] => {}
                [value] => terms.push(format!("{key}:{value}")),
                any => terms.push(format!("{key}:{{{}}}", any.join(" "))),
            }
        }
        if self.starred {
            terms.push("is:starred".to_string());
        }
        if self.has_attachment {
            terms.push("has:attachment".to_string());
        }
        for (key, value) in [("after", &self.after), ("before", &self.before)] {
            if let Some(date) = parse_date(value.trim()) {
                terms.push(format!("{key}:{}", date.format("%Y/%m/%d")));
            }
        }
        let words = self.words.trim();
        let (words, tail) = match clauses(words).iter().find(|clause| is_and(clause)) {
            Some(and) => words.split_at(and.raw.as_ptr() as usize - words.as_ptr() as usize),
            None => (words, ""),
        };
        let words = words.trim();
        if !words.is_empty() {
            terms.push(words.to_string());
        }
        if self.unread {
            terms.push("is:unread".to_string());
        }
        if !tail.is_empty() {
            terms.push(tail.to_string());
        }
        terms.join(" ")
    }
}

/// `search.parse` / `search.build`, the search options form's round trip,
/// shared by the desktop sidecar and the mobile bridge. `None` for any other
/// method.
pub fn handle_request(method: &str, params: &serde_json::Value) -> Option<serde_json::Value> {
    match method {
        "search.parse" => {
            let query = params
                .get("query")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            serde_json::to_value(SearchFields::parse(query)).ok()
        }
        "search.build" => {
            let fields: SearchFields =
                serde_json::from_value(params.get("fields").cloned().unwrap_or_default())
                    .unwrap_or_default();
            Some(serde_json::json!({ "query": fields.to_query() }))
        }
        _ => None,
    }
}

/// A term's alternatives as a form field lists them, comma-separated. One the
/// field would split is quoted, so submitting the form unchanged searches for
/// it whole.
fn field_text(any: &AnyOf) -> String {
    any.iter()
        .map(|value| {
            let words: Vec<&str> = value.split_whitespace().collect();
            // The field would split it, or re-space it: its words are rejoined
            // with single spaces.
            let splits =
                value.contains([',', '|']) || words.contains(&"OR") || words.join(" ") != *value;
            if splits {
                format!("\"{}\"", value.replace('"', ""))
            } else {
                value.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Put `value` in an empty form field; a full one leaves it to `words`.
fn fill(field: &mut String, value: String) -> bool {
    if !field.is_empty() {
        return false;
    }
    *field = value;
    true
}

fn iso(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

/// Epoch seconds of local midnight starting `date`, which is what a user means
/// by `after:2026/01/02`. Falls back to UTC midnight for a day that has no
/// local midnight (a DST gap at 00:00).
pub fn local_midnight(date: NaiveDate) -> i64 {
    let midnight = date.and_time(chrono::NaiveTime::MIN);
    midnight
        .and_local_timezone(chrono::Local)
        .earliest()
        .map_or_else(|| midnight.and_utc().timestamp(), |dt| dt.timestamp())
}

/// Split on whitespace, except inside a double-quoted span or a group opened
/// right after a key's colon, so `from:"Ann Lee"` and `from:{a b}` each stay
/// one term. An unclosed quote or group runs to the end, or to a bare `AND`:
/// the query joined on there is not part of its value. A group also ends at a
/// bare `AND`, which is never one of its alternatives, and a newline ends any
/// term, so queries joined on separate lines can't run into each other.
fn terms(query: &str) -> impl Iterator<Item = &str> {
    let mut rest = query;
    std::iter::from_fn(move || {
        rest = rest.trim_start();
        if rest.is_empty() {
            return None;
        }
        let (end, quoted, close) = scan_term(rest);
        // Still open, at the end or a newline.
        let end = if quoted || close.is_some() {
            before_bare_and(&rest[..end]).unwrap_or(end)
        } else {
            end
        };
        let (term, tail) = rest.split_at(end);
        rest = tail;
        Some(term)
    })
}

/// Read a term from the start of `text`, as [`terms`] does: where it ends, and
/// whether a quote and which group's closer are still open there.
fn scan_term(text: &str) -> (usize, bool, Option<char>) {
    let mut quoted = false;
    let mut close = None;
    let mut prev = ' ';
    let end = text
        .char_indices()
        .find(|&(i, c)| {
            if c == '\n'
                || (c.is_whitespace()
                    && close.is_some()
                    && !quoted
                    && before_bare_and(&text[i..]) == Some(0))
            {
                return true;
            }
            let after = std::mem::replace(&mut prev, c);
            if c == '"' {
                quoted = !quoted;
            } else if !quoted {
                match close {
                    Some(end) if c == end => close = None,
                    None if after == ':' => {
                        close = match c {
                            '{' => Some('}'),
                            '(' => Some(')'),
                            _ => None,
                        }
                    }
                    _ => {}
                }
            }
            c.is_whitespace() && !quoted && close.is_none()
        })
        .map_or(text.len(), |(i, _)| i);
    (end, quoted, close)
}

/// `query`'s terms, each with the quote and group it leaves open closed, for a
/// server that reads the query itself: text joined on after it then can't fall
/// inside.
pub(crate) fn close_open_terms(query: &str) -> String {
    terms(query)
        .map(|term| {
            let (_, quoted, close) = scan_term(term);
            let mut term = term.to_string();
            if quoted {
                term.push('"');
            }
            term.extend(close);
            term
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The end of `text` before its first bare `AND` word, if it has one.
fn before_bare_and(text: &str) -> Option<usize> {
    text.match_indices("AND")
        .find(|&(i, _)| {
            text[..i].ends_with(char::is_whitespace)
                && text[i + 3..].chars().next().is_none_or(char::is_whitespace)
        })
        .map(|(i, _)| text[..i].trim_end().len())
}

fn unquote(value: &str) -> &str {
    let value = value.strip_prefix('"').unwrap_or(value);
    value.strip_suffix('"').unwrap_or(value).trim()
}

/// Gmail's `yyyy/mm/dd`, plus ISO `yyyy-mm-dd`.
fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y/%m/%d")
        .or_else(|_| NaiveDate::parse_from_str(value, "%Y-%m-%d"))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_query_is_unchanged_free_text() {
        let q = SearchQuery::parse("  quarterly  report ");
        assert_eq!(q.text, ["quarterly  report"]);
        assert!(!q.has_operators());
    }

    #[test]
    fn operators_are_taken_out_of_the_text() {
        let q = SearchQuery::parse("from:a@b.c to:d@e.f budget is:unread");
        assert_eq!(q.from, [["a@b.c"]]);
        assert_eq!(q.to, [["d@e.f"]]);
        assert_eq!(q.text, ["budget"]);
        assert!(q.unread && !q.read);
    }

    #[test]
    fn quoted_values_keep_spaces() {
        let q = SearchQuery::parse(r#"From:"Ann Lee" subject:"Q3 plan" notes"#);
        assert_eq!(q.from, [["Ann Lee"]]);
        assert_eq!(q.subject, [["Q3 plan"]]);
        assert_eq!(q.text, ["notes"]);
    }

    #[test]
    fn unknown_or_empty_operators_stay_text() {
        let q = SearchQuery::parse("re:foo https://x.y from: is:muted after:soon");
        assert!(!q.has_operators());
        assert_eq!(q.text, ["re:foo https://x.y from: is:muted after:soon"]);
    }

    #[test]
    fn alternatives_in_gmail_syntax() {
        let any = |query: &str| SearchQuery::parse(query).from;
        assert_eq!(any("from:{a@b.c x@y.z}"), [["a@b.c", "x@y.z"]]);
        assert_eq!(any(r#"from:{"Ann Lee", bob}"#), [["Ann Lee", "bob"]]);
        assert_eq!(any("from:(a OR b | c)"), [["a", "b", "c"]]);
        // Parentheses without separators hold one value.
        assert_eq!(any("from:(Ann Lee)"), [["Ann Lee"]]);
        assert_eq!(any("from:a,b"), [["a", "b"]]);
        assert_eq!(any("from:a OR from:b OR from:c"), [["a", "b", "c"]]);
        assert_eq!(any(r#"from:"Ann Lee",bob"#), [["Ann Lee", "bob"]]);
        assert_eq!(any(r#"from:"Ann Lee"#), [["Ann Lee"]]);
        // A quoted comma is part of the name.
        assert_eq!(any(r#"from:"Lee, Ann""#), [["Lee, Ann"]]);
        // Each term is still ANDed with the others.
        assert_eq!(any("from:{a b} from:c"), [vec!["a", "b"], vec!["c"]]);

        // `OR` that can't merge stays a word: lowercase, across keys, or
        // between words.
        let q = SearchQuery::parse("from:a or from:b");
        assert_eq!(
            (q.from.len(), q.text.as_slice()),
            (2, &["or".to_string()][..])
        );
        let q = SearchQuery::parse("from:a OR to:b");
        assert_eq!(
            (q.from.len(), q.to.len(), q.text.as_slice()),
            (1, 1, &["OR".to_string()][..])
        );
        assert_eq!(SearchQuery::parse("cats OR dogs").text, ["cats OR dogs"]);
        // A group opens only right after a key's colon; an empty one is text.
        assert_eq!(SearchQuery::parse("(a b) from:{}").text, ["(a b) from:{}"]);
        assert_eq!(any("subject:x from:{a b"), [["a", "b"]]);
    }

    #[test]
    fn fields_list_alternatives_comma_separated() {
        let fields = SearchFields::parse("from:a OR from:b to:{c \"D E\"} plan");
        assert_eq!(fields.from, "a, b");
        assert_eq!(fields.to, "c, D E");
        assert_eq!(fields.words, "plan");
        assert_eq!(fields.to_query(), r#"from:{a b} to:{c "D E"} plan"#);
        assert_eq!(SearchFields::parse(&fields.to_query()), fields);
        // Submitting the form unchanged keeps a value it would otherwise split.
        for query in [
            r#"subject:"Q3  plan""#,
            r#"from:"Lee, Ann""#,
            r#"subject:"A|B""#,
            r#"subject:"OR""#,
            r#"subject:"this OR that""#,
        ] {
            let fields = SearchFields::parse(query);
            assert_eq!(
                SearchQuery::parse(&fields.to_query()),
                SearchQuery::parse(query),
                "{query}"
            );
        }
        assert_eq!(
            SearchFields::parse(r#"from:"Lee, Ann" OR from:bob"#).from,
            r#""Lee, Ann", bob"#
        );
        // A lone `OR` is a value, not a separator.
        for (field, query) in [
            ("OR", "subject:\"OR\""),
            ("a OR", "subject:\"a OR\""),
            ("OR, b", "subject:{\"OR\" b}"),
        ] {
            let fields = SearchFields {
                subject: field.into(),
                ..Default::default()
            };
            assert_eq!(fields.to_query(), query, "{field}");
        }
        assert_eq!(SearchQuery::parse("from:{OR b}").from, [["OR", "b"]]);
        // A picked suggestion leaves a trailing comma.
        let fields = SearchFields {
            from: "a@b.c, ".into(),
            ..Default::default()
        };
        assert_eq!(fields.to_query(), "from:a@b.c");
    }

    #[test]
    fn flags_and_dates() {
        let q = SearchQuery::parse("is:starred is:read after:2026/01/02 before:2026-02-03");
        assert!(q.starred);
        assert!(q.read && !q.unread);
        assert_eq!(q.after, NaiveDate::from_ymd_opt(2026, 1, 2));
        assert_eq!(q.before, NaiveDate::from_ymd_opt(2026, 2, 3));
        assert!(q.text.is_empty());
        assert!(!q.is_empty());
    }

    #[test]
    fn has_attachment_is_an_operator() {
        let q = SearchQuery::parse("HAS:Attachment budget");
        assert!(q.has_attachment);
        assert_eq!(q.text, ["budget"]);
        assert!(SearchQuery::parse("has:attachment").has_operators());
        assert!(SearchQuery::parse("x AND has:attachment").has_attachment);
        // Gmail's other `has:` values stay text, as any unknown operator.
        let q = SearchQuery::parse("has:drive");
        assert!(!q.has_operators());
        assert_eq!(q.text, ["has:drive"]);
    }

    #[test]
    fn and_joins_queries_that_must_all_match() {
        let q = SearchQuery::parse("invoice  due AND paid");
        assert_eq!(q.text, ["invoice  due", "paid"]);
        let q = SearchQuery::parse("from:boss budget AND from:ann notes");
        assert_eq!(q.from, [["boss"], ["ann"]]);
        assert_eq!(q.text, ["budget", "notes"]);
        // Each side's dates and read state narrow the other's.
        let q = SearchQuery::parse(
            "after:2026/10/01 before:2026/12/01 AND after:2026/01/01 before:2026/11/01",
        );
        assert_eq!(q.after, NaiveDate::from_ymd_opt(2026, 10, 1));
        assert_eq!(q.before, NaiveDate::from_ymd_opt(2026, 11, 1));
        let q = SearchQuery::parse("is:unread AND is:read");
        assert!(q.read && q.unread);
        // Within a side the last read state still wins.
        let q = SearchQuery::parse("is:unread is:read AND x");
        assert!(q.read && !q.unread);
        // Lowercase or quoted `and` is a word; an empty side is nothing.
        assert_eq!(
            SearchQuery::parse("salt and pepper").text,
            ["salt and pepper"]
        );
        assert_eq!(
            SearchQuery::parse(r#"subject:"Q AND A""#).subject,
            [["Q AND A"]]
        );
        assert_eq!(SearchQuery::parse("AND budget AND").text, ["budget"]);
        // An unclosed group or quote ends at the `AND`, not swallowing the rest.
        let q = SearchQuery::parse("from:{boss ann AND budget");
        assert_eq!(q.from, [["boss", "ann"]]);
        assert_eq!(q.text, ["budget"]);
        let q = SearchQuery::parse(r#"subject:"Q3 plan AND budget"#);
        assert_eq!(q.subject, [["Q3 plan"]]);
        assert_eq!(q.text, ["budget"]);
        // A closed one keeps its `AND`.
        // A group never takes in a bare `AND`, closed or not.
        let q = SearchQuery::parse("from:{boss ann AND from:{c d}");
        assert_eq!(q.from, [vec!["boss", "ann"], vec!["c", "d"]]);
        // Closing what a saved query leaves open, as narrowing it does, reads
        // the same, so the narrowed search can be flattened onto one line.
        assert_eq!(
            SearchQuery::parse(r#"x subject:"Q3"#),
            SearchQuery::parse(r#"x subject:"Q3""#)
        );
        assert_eq!(
            SearchQuery::parse("x from:{a b"),
            SearchQuery::parse("x from:{a b}")
        );
        assert_eq!(
            SearchQuery::parse("subject:\"Q3\"\nAND subject:\"x y\""),
            SearchQuery::parse(r#"subject:"Q3" AND subject:"x y""#)
        );
        // A quote may hold one, but doesn't run past a line, where a line left
        // open reads as it would alone.
        let q = SearchQuery::parse("subject:\"Q3 plan\nAND subject:\"x\"");
        assert_eq!(q.subject, [["Q3 plan"], ["x"]]);
        let alone = SearchQuery::parse(r#"subject:"Q3 AND budget"#);
        let mut narrowed = SearchQuery::parse("subject:\"Q3 AND budget\nAND paid");
        assert_eq!(narrowed.text.pop().as_deref(), Some("paid"));
        assert_eq!(narrowed, alone);
        let q = SearchQuery::parse("27\" monitor\nAND 24\" monitor");
        assert_eq!(q.text, ["27\" monitor", "24\" monitor"]);
    }

    #[test]
    fn fields_hold_the_first_side_of_an_and() {
        let fields = SearchFields::parse("from:boss is:read AND from:ann is:unread budget");
        assert_eq!(fields.from, "boss");
        assert!(!fields.unread);
        assert_eq!(fields.words, "is:read AND from:ann is:unread budget");
        let ticked = SearchFields {
            unread: true,
            ..SearchFields::parse("from:boss AND is:read")
        };
        assert_eq!(ticked.to_query(), "from:boss is:unread AND is:read");
        let q = SearchQuery::parse(&ticked.to_query());
        assert!(q.read && q.unread);
        for query in [
            "from:boss is:read AND from:ann is:unread budget",
            "a  b AND c",
        ] {
            let fields = SearchFields::parse(query);
            assert_eq!(
                SearchQuery::parse(&fields.to_query()),
                SearchQuery::parse(query),
                "{query}"
            );
        }
    }

    #[test]
    fn fields_round_trip_through_query_text() {
        let fields = SearchFields::parse(
            r#"budget from:"Ann Lee" to:d@e.f is:unread is:starred has:attachment after:2026/01/02 before:2026-02-03"#,
        );
        assert_eq!(
            fields,
            SearchFields {
                from: "Ann Lee".into(),
                to: "d@e.f".into(),
                subject: String::new(),
                words: "budget".into(),
                after: "2026-01-02".into(),
                before: "2026-02-03".into(),
                unread: true,
                starred: true,
                has_attachment: true,
            }
        );
        assert_eq!(
            fields.to_query(),
            r#"from:"Ann Lee" to:d@e.f is:starred has:attachment after:2026/01/02 before:2026/02/03 budget is:unread"#
        );
        assert_eq!(SearchFields::parse(&fields.to_query()), fields);
    }

    #[test]
    fn fields_keep_terms_they_cannot_hold_in_words() {
        let fields = SearchFields::parse("from:a@b.c from:x@y.z is:read re:foo");
        assert_eq!(fields.from, "a@b.c");
        assert_eq!(fields.words, "from:x@y.z is:read re:foo");
        assert_eq!(fields.to_query(), "from:a@b.c from:x@y.z is:read re:foo");
        // A plain query is all words, spacing intact.
        assert_eq!(SearchFields::parse(" a  b ").words, "a  b");
    }

    #[test]
    fn fields_keep_the_last_read_state() {
        let unread = SearchFields::parse("is:read budget is:unread");
        assert!(unread.unread);
        assert_eq!(unread.words, "budget");
        assert!(SearchQuery::parse(&unread.to_query()).unread);

        let read = SearchFields::parse("is:unread is:read budget is:read");
        assert!(!read.unread);
        assert_eq!(read.words, "is:read budget is:read");
        assert!(SearchQuery::parse(&read.to_query()).read);

        // Ticking Unread on a read search wins over the kept `is:read`.
        let ticked = SearchFields {
            unread: true,
            ..SearchFields::parse("is:read budget")
        };
        assert!(!SearchQuery::parse(&ticked.to_query()).read);
    }

    #[test]
    fn fields_drop_unusable_values() {
        let fields = SearchFields {
            from: r#" "x" "#.into(),
            subject: "  ".into(),
            after: "soon".into(),
            ..Default::default()
        };
        assert_eq!(fields.to_query(), "from:x");
    }
}
