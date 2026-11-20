use super::{Account, Engine, account_client, accounts, lock, sync_book};
use anyhow::{Context, Result, anyhow, ensure};
use calcard::vcard::{VCard, VCardEntry, VCardProperty as Prop, VCardValue as Val, VCardVersion};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

pub(super) fn list(db: &Connection, book: &str, p: &Value) -> Result<Value> {
    let query = p["query"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let offset = p["offset"].as_u64().unwrap_or_default();
    let mut stmt = db.prepare(
        "SELECT card.url, card.etag, card.name, card.emails FROM carddav_cards card
         JOIN carddav_books book ON book.id=card.book WHERE book.enabled AND card.book=?1 AND NOT card.invalid
         ORDER BY card.name COLLATE NOCASE, card.url",
    )?;
    let mut cards = Vec::new();
    let mut matched = 0;
    for row in stmt.query_map([book], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
        ))
    })? {
        let (id, version, name, emails) = row?;
        let emails = serde_json::from_str::<Vec<String>>(&emails)?;
        if !query.is_empty()
            && !name.to_lowercase().contains(&query)
            && !emails
                .iter()
                .any(|email| email.to_lowercase().contains(&query))
        {
            continue;
        }
        matched += 1;
        if matched <= offset {
            continue;
        }
        cards.push(json!({"id": id, "version": version, "name": name, "emails": emails}));
        if cards.len() == 51 {
            break;
        }
    }
    let more = cards.len() > 50;
    cards.truncate(50);
    Ok(json!({"contacts": cards, "more": more}))
}

/// The editable cards holding `email`, in every enabled address book: what
/// a directory entry (one per address) is saved as.
pub(super) fn cards_for(db: &Connection, email: &str) -> Result<Value> {
    let mut stmt = db.prepare(
        "SELECT card.book, book.name, card.url, card.etag, card.name, card.emails,
                account.username, account.google, card.note
         FROM carddav_cards card JOIN carddav_books book ON book.id=card.book
         JOIN carddav_accounts account ON account.id=book.account
         WHERE book.enabled AND NOT card.invalid
         ORDER BY book.name COLLATE NOCASE, card.name COLLATE NOCASE, card.url",
    )?;
    // Matched in Rust, not SQLite's ASCII-only lower(): the directory folds
    // case the same way, so a card is found for every address it lists.
    let wanted = email.trim().to_lowercase();
    let cards = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, bool>(7)?,
                r.get::<_, String>(8)?,
            ))
        })?
        .map(|row| {
            let (book_id, book_name, id, version, name, emails, account, google, note) = row?;
            let emails = serde_json::from_str::<Vec<String>>(&emails)?;
            if !emails.iter().any(|known| known.to_lowercase() == wanted) {
                return Ok(None);
            }
            Ok(Some(json!({
                "book_id": book_id,
                "book_name": book_name,
                "id": id,
                "version": version,
                "name": name,
                "emails": emails,
                "account": account,
                "google": google,
                "note": note,
            })))
        })
        .filter_map(Result::transpose)
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({ "cards": cards }))
}

pub(super) fn mutate(engine: &Engine, book_id: &str, method: &str, p: &Value) -> Result<Value> {
    let account: Account = accounts(&*lock(engine)?)?
        .into_iter()
        .find(|account| account.books.iter().any(|book| book.id == book_id))
        .context("Address book not found")?;
    let book = account
        .books
        .iter()
        .find(|book| book.id == book_id && book.enabled)
        .context("Enable the address book before editing contacts")?;
    let delete = method == "carddav.deleteContact";
    let draft = if delete { None } else { Some(Draft::read(p)?) };
    let client = account_client(engine, &account)?;
    if method == "carddav.createContact" {
        let uid = match p["creation_id"].as_str() {
            Some(id) => uuid::Uuid::parse_str(id).context("invalid contact creation ID")?,
            None => uuid::Uuid::new_v4(),
        }
        .to_string();
        let draft = draft.unwrap();
        ensure!(!draft.name.is_empty(), "Enter a contact name");
        let vcard = draft.serialize(None, &uid, account.google)?;
        client.create(&book.book.url, &uid, &vcard)?;
    } else {
        let id = p["contact_id"].as_str().context("missing contact_id")?;
        let expected = p["version"].as_str().context("missing contact version")?;
        let version: String = lock(engine)?
            .query_row(
                "SELECT etag FROM carddav_cards WHERE book=?1 AND url=?2 AND NOT invalid",
                params![book_id, id],
                |row| row.get(0),
            )
            .context("Contact is no longer in this address book. Sync and try again.")?;
        ensure!(
            version == expected,
            "This contact changed. Reopen it and try again."
        );
        client.change(
            &book.book.url,
            id,
            &version,
            |raw| draft.unwrap().serialize(Some(raw), "", account.google),
            delete,
        )?;
    }
    // A failed pull after a successful write must not invite a repeat POST.
    let sync_error = sync_book(engine, &client, book, account.google)
        .err()
        .map(|err| err.to_string());
    Ok(json!({"sync_error": sync_error}))
}

struct Draft {
    name: String,
    emails: Vec<String>,
    /// The new NOTE; None leaves the card's note as it is.
    note: Option<String>,
}

impl Draft {
    fn read(p: &Value) -> Result<Self> {
        let name = p["name"].as_str().unwrap_or_default().trim();
        ensure!(
            name.len() <= 4096 && !name.chars().any(char::is_control),
            "Enter a valid contact name"
        );
        let mut emails = Vec::<String>::new();
        for value in p["emails"].as_array().context("missing emails")? {
            let value = value.as_str().context("invalid email address")?.trim();
            ensure!(
                valid_email(value),
                "Enter a valid email address on each line"
            );
            if !emails.iter().any(|known| same_email(known, value)) {
                emails.push(value.into());
            }
        }
        // Meron lists book contacts by address: one without any would be
        // left on the server where Meron can no longer reach it.
        ensure!(!emails.is_empty(), "Add at least one email address");
        let note = match &p["note"] {
            Value::Null => None,
            value => {
                let note = value.as_str().context("invalid contact note")?.trim();
                ensure!(note.len() <= 65536, "The note is too long");
                Some(note.to_owned())
            }
        };
        Ok(Self {
            name: name.into(),
            emails,
            note,
        })
    }

    fn serialize(&self, raw: Option<&str>, uid: &str, google: bool) -> Result<String> {
        if let Some(raw) = raw {
            return self.edit_raw(raw);
        }
        let mut card = VCard {
            entries: vec![
                entry(Prop::Uid, Val::Text(uid.into())),
                structured_name(&self.name),
                entry(Prop::Fn, Val::Text(self.name.clone())),
            ],
        };
        card.entries.extend(
            self.emails
                .iter()
                .map(|email| entry(Prop::Email, Val::Text(email.clone()))),
        );
        if let Some(note) = self.note.as_ref().filter(|note| !note.is_empty()) {
            card.entries
                .push(entry(Prop::Note, Val::Text(note.clone())));
        }
        let mut output = String::new();
        card.write_to(
            &mut output,
            if google {
                VCardVersion::V3_0
            } else {
                VCardVersion::V4_0
            },
        )?;
        Ok(output)
    }

    // Keep unrelated content lines exactly as received, including vendor
    // properties, parameters, folding and binary data the parser may ignore.
    fn edit_raw(&self, raw: &str) -> Result<String> {
        let mut lines: Vec<String> = Vec::new();
        for line in raw.trim_end().lines() {
            if line.starts_with([' ', '\t']) {
                lines
                    .last_mut()
                    .context("invalid vCard folding")?
                    .push_str(&format!("\r\n{line}"));
            } else {
                lines.push(line.into());
            }
        }
        ensure!(
            lines
                .first()
                .is_some_and(|line| line.eq_ignore_ascii_case("BEGIN:VCARD"))
                && lines
                    .last()
                    .is_some_and(|line| line.eq_ignore_ascii_case("END:VCARD")),
            "The server did not return a complete vCard"
        );
        let body = &lines[1..lines.len() - 1];
        let (version_index, version) = body
            .iter()
            .enumerate()
            .find(|(_, line)| {
                line.get(..8)
                    .is_some_and(|head| head.eq_ignore_ascii_case("VERSION:"))
            })
            .context("The vCard is missing its version")?;
        let v4 = version.eq_ignore_ascii_case("VERSION:4.0");
        ensure!(
            v4 || version.eq_ignore_ascii_case("VERSION:3.0"),
            "Meron can safely edit only vCard 3.0 and 4.0 contacts"
        );
        let mut parsed = Vec::new();
        for (i, line) in body.iter().enumerate() {
            if i == version_index {
                continue;
            }
            let name = line.split([';', ':']).next().unwrap_or_default();
            let (group, prop) = name.rsplit_once('.').unwrap_or(("", name));
            ensure!(
                !matches!(
                    prop.to_ascii_uppercase().as_str(),
                    "BEGIN" | "END" | "VERSION"
                ),
                "Unsupported nested vCard content"
            );
            let value = if ["FN", "EMAIL", "NOTE"]
                .iter()
                .any(|p| prop.eq_ignore_ascii_case(p))
            {
                let card = VCard::parse(&format!(
                    "BEGIN:VCARD\r\n{version}\r\n{line}\r\nEND:VCARD\r\n"
                ))
                .map_err(|_| anyhow!("Could not read contact property {prop}"))?;
                card.entries
                    .into_iter()
                    .find(|entry| entry.name.as_str().eq_ignore_ascii_case(prop))
            } else {
                None
            };
            parsed.push((line, group, prop, value));
        }
        let old_name =
            super::display_name(parsed.iter().filter_map(|(_, _, _, item)| item.as_ref()));
        let has_fn = parsed
            .iter()
            .any(|(_, _, prop, _)| prop.eq_ignore_ascii_case("FN"));
        let has_n = parsed
            .iter()
            .any(|(_, _, prop, _)| prop.eq_ignore_ascii_case("N"));
        // The NOTE Meron shows (see Card::parse): the first with text. Only
        // that one is replaced; other notes, and their parameters, stay.
        let shown_note = parsed.iter().position(|(_, _, prop, item)| {
            prop.eq_ignore_ascii_case("NOTE")
                && item.as_ref().is_some_and(|item| {
                    item.values
                        .iter()
                        .any(|value| matches!(value, Val::Text(text) if !text.trim().is_empty()))
                })
        });
        let rename = old_name != self.name;
        ensure!(!rename || !self.name.is_empty(), "Enter a contact name");
        let rewrite_n = rename && (!old_name.is_empty() || !has_n);
        let mut output = format!("{}\r\n{version}\r\n", lines[0]);
        let mut removed_groups = std::collections::BTreeSet::new();
        let mut kept_groups = std::collections::BTreeSet::new();
        let mut present = Vec::new();
        // The new note, written where the shown one was so it stays the one
        // shown; appended when the card had none.
        let mut new_note = match self.note.as_ref().filter(|note| !note.is_empty()) {
            Some(note) => {
                let mut line = String::new();
                entry(Prop::Note, Val::Text(note.clone())).write_to(&mut line, v4)?;
                Some(line.trim_end_matches("\r\n").to_owned())
            }
            None => None,
        };
        let mut kept: Vec<(std::borrow::Cow<str>, &str)> = Vec::new();
        for (index, (line, group, prop, item)) in parsed.into_iter().enumerate() {
            if self.note.is_some() && shown_note == Some(index) {
                if let Some(note) = new_note.take() {
                    // Only the text changes: the group and parameters
                    // (`item1.NOTE;LANGUAGE=fr:`) are the line's own.
                    let text = note
                        .replace("\r\n ", "")
                        .split_once(':')
                        .map(|(_, text)| text.to_owned())
                        .unwrap_or_default();
                    kept.push((format!("{}:{text}", property_prefix(line)).into(), group));
                    if !group.is_empty() {
                        kept_groups.insert(group.to_ascii_lowercase());
                    }
                }
                continue;
            }
            if (rename && prop.eq_ignore_ascii_case("FN"))
                || (rewrite_n && prop.eq_ignore_ascii_case("N"))
            {
                // A rename is explicit; keep the name coherent on clients that
                // render N, without guessing how to split someone's full name.
                continue;
            }
            if prop.eq_ignore_ascii_case("EMAIL")
                && let Some(item) = item
            {
                if let [Val::Text(address)] = item.values.as_slice()
                    && valid_email(address.trim())
                {
                    if !self
                        .emails
                        .iter()
                        .any(|email| same_email(email, address.trim()))
                    {
                        if !group.is_empty() {
                            removed_groups.insert(group.to_ascii_lowercase());
                        }
                        continue;
                    }
                    present.push(address.trim().to_owned());
                }
            }
            if !group.is_empty()
                && !prop.eq_ignore_ascii_case("X-ABLabel")
                && !prop.eq_ignore_ascii_case("X-ABADR")
            {
                kept_groups.insert(group.to_ascii_lowercase());
            }
            kept.push((line.into(), group));
        }
        for (line, group) in kept {
            let group = group.to_ascii_lowercase();
            if !removed_groups.contains(&group) || kept_groups.contains(&group) {
                output.push_str(&line);
                output.push_str("\r\n");
            }
        }
        if rename || !has_fn {
            entry(Prop::Fn, Val::Text(self.name.clone())).write_to(&mut output, v4)?;
        }
        if rewrite_n {
            structured_name(&self.name).write_to(&mut output, v4)?;
        }
        for email in &self.emails {
            if !present.iter().any(|address| same_email(address, email)) {
                entry(Prop::Email, Val::Text(email.clone())).write_to(&mut output, v4)?;
            }
        }
        if let Some(note) = new_note {
            output.push_str(&note);
            output.push_str("\r\n");
        }
        output.push_str("END:VCARD\r\n");
        Ok(output)
    }
}

/// A content line up to the colon before its value: group, name and
/// parameters. A colon inside a quoted parameter value does not end it.
fn property_prefix(line: &str) -> &str {
    let mut quoted = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ':' if !quoted => return &line[..i],
            _ => {}
        }
    }
    line
}

fn same_email(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

pub(super) fn valid_email(value: &str) -> bool {
    value.len() <= 1024
        && value
            .split_once('@')
            .is_some_and(|(user, host)| !user.is_empty() && !host.is_empty() && !host.contains('@'))
        && !value
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || "<>,;:".contains(c))
}

fn entry(name: Prop, value: Val) -> VCardEntry {
    VCardEntry {
        name,
        group: None,
        params: Vec::new(),
        values: vec![value],
    }
}

fn structured_name(name: &str) -> VCardEntry {
    VCardEntry {
        name: Prop::N,
        group: None,
        params: Vec::new(),
        values: ["", name, "", "", ""]
            .map(|part| Val::Text(part.into()))
            .into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_edits_preserve_names_when_fn_is_missing_or_has_alternatives() {
        let n = "N:Fixture;Cora;Q.;Dr.;III\r\n";
        for names in [
            "",
            "FN:\r\nFN;LANGUAGE=en:Cora Fixture\r\nFN;LANGUAGE=fr:Autre nom\r\n",
        ] {
            let raw = format!(
                "BEGIN:VCARD\r\nUID:fixture\r\nVERSION:3.0\r\n{n}{names}EMAIL:old@example.test\r\nEND:VCARD\r\n\r\n"
            );
            let card = super::super::Card::parse("contact.vcf", "v1", &raw).unwrap();
            assert_eq!(
                card.name,
                if names.is_empty() { "" } else { "Cora Fixture" }
            );
            let draft = Draft {
                name: card.name,
                emails: vec!["new@example.test".into()],
                note: None,
            };
            let result = draft.edit_raw(&raw).unwrap();
            assert!(result.contains(n));
            assert!(result.contains(names));
            assert!(!result.contains("old@example.test"));
            assert!(result.contains("new@example.test"));
        }
    }

    #[test]
    fn notes_are_read_replaced_and_cleared() {
        let raw = "BEGIN:VCARD\r\nVERSION:4.0\r\nUID:n\r\nFN:Cora Fixture\r\nNOTE:Met at the fair\\, 2025\\nLikes tea\r\nEMAIL:cora@example.test\r\nEND:VCARD\r\n";
        let card = super::super::Card::parse("n.vcf", "v1", raw).unwrap();
        assert_eq!(card.note, "Met at the fair, 2025\nLikes tea");
        let draft = |note: &str| Draft {
            name: "Cora Fixture".into(),
            emails: vec!["cora@example.test".into()],
            note: Some(note.into()),
        };
        let replaced = draft("# Plans\nCall on Friday").edit_raw(raw).unwrap();
        let card = super::super::Card::parse("n.vcf", "v2", &replaced).unwrap();
        assert_eq!(card.note, "# Plans\nCall on Friday");
        assert_eq!(replaced.matches("NOTE").count(), 1);
        let cleared = draft("").edit_raw(raw).unwrap();
        assert!(!cleared.contains("NOTE"), "{cleared}");
        // The edited note keeps its own group and parameters.
        let grouped = raw.replace(
            "NOTE:Met at the fair\\, 2025\\nLikes tea",
            "item1.NOTE;LANGUAGE=en;X-SRC=\"a:b\":Met at the fair",
        );
        let edited = draft("Call on Friday, 3pm").edit_raw(&grouped).unwrap();
        assert!(
            edited.contains("item1.NOTE;LANGUAGE=en;X-SRC=\"a:b\":Call on Friday\\, 3pm\r\n"),
            "{edited}"
        );
        // Only the note shown is replaced; another, with its parameters, stays.
        let two = raw.replace("EMAIL:", "NOTE;LANGUAGE=fr:Rencontrée au salon\r\nEMAIL:");
        let edited = draft("Call on Friday").edit_raw(&two).unwrap();
        assert!(
            edited.contains("NOTE;LANGUAGE=fr:Rencontrée au salon\r\n"),
            "{edited}"
        );
        assert!(!edited.contains("Met at the fair"), "{edited}");
        assert_eq!(
            super::super::Card::parse("n.vcf", "v3", &edited)
                .unwrap()
                .note,
            "Call on Friday"
        );
        let created = draft("New note").serialize(None, "uid-1", false).unwrap();
        let card = super::super::Card::parse("c.vcf", "v1", &created).unwrap();
        assert_eq!(card.note, "New note");
    }

    #[test]
    fn editing_preserves_unrelated_fields_and_email_metadata() {
        let raw = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:test-contact\r\nFN:Cora Fixture\r\nN:Fixture;Cora;;;\r\nTEL;TYPE=WORK:+1-202-555-0100\r\nNOTE:An example note\r\nEMAIL;TYPE=WORK:cora@example.test\r\nEMAIL:old@example.test\r\nEND:VCARD\r\n";
        let draft = Draft {
            name: "Cora Edited".into(),
            emails: vec!["cora@example.test".into(), "new@example.test".into()],
            note: None,
        };
        let output = draft.serialize(Some(raw), "", true).unwrap();
        let parse = |text: &str| VCard::parse(text).unwrap().entries;
        let original = parse(raw);
        let edited = parse(&output);
        for name in [Prop::Uid, Prop::Tel, Prop::Note] {
            let select =
                |entries: &[VCardEntry]| entries.iter().find(|entry| entry.name == name).cloned();
            assert_eq!(select(&edited), select(&original));
        }
        assert!(output.contains("VERSION:3.0"));
        assert!(output.contains("FN:Cora Edited"));
        assert!(output.contains("N:;Cora Edited;;;"));
        assert!(output.contains("new@example.test"));
        assert!(!output.contains("old@example.test"));
        let has_work = |entry: &&VCardEntry| {
            entry.name == Prop::Email && entry.values == vec![Val::Text("cora@example.test".into())]
        };
        assert_eq!(original.iter().find(has_work), edited.iter().find(has_work));
    }

    #[test]
    fn email_only_edit_preserves_structured_name_and_unrelated_content_lines() {
        let raw = concat!(
            "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:fixture\r\n",
            "FN:Cora Fixture\r\nN:Fixture;Cora;Q.;Dr.;III\r\n",
            "PHOTO;ENCODING=b;TYPE=PNG:YWJj\r\n ZGVm\r\n",
            "ADR;TYPE=WORK:;;100 Example St;Exampleville;;;\r\n",
            "BDAY:2000-01-01\r\nCATEGORIES:Test,Example\r\n",
            "X-EXAMPLE;X-PARAM=\"text:with;punctuation\":value\r\n",
            "item1.EMAIL;TYPE=WORK:old@example.test\r\nitem1.X-ABLabel:Work\r\n",
            "END:VCARD\r\n",
        );
        let draft = Draft {
            name: "Cora Fixture".into(),
            emails: vec!["new@example.test".into()],
            note: None,
        };
        let output = draft.edit_raw(raw).unwrap();
        assert_eq!(
            output,
            raw.replace(
                "item1.EMAIL;TYPE=WORK:old@example.test\r\nitem1.X-ABLabel:Work\r\n",
                "EMAIL:new@example.test\r\n"
            )
        );
        assert!(
            draft
                .edit_raw(&raw.replace("VERSION:3.0", "VERSION:2.1"))
                .is_err()
        );
    }
}
