//! Who a reply addresses and who it merely copies. Desktop and mobile both used
//! to decide this for themselves, in their own language, and drifted: one kept
//! display names in the To line, the other reduced it to a bare address, and a
//! follow-up to our own message addressed us on one platform but not the other.
//! The rule lives here instead, beside the addresses it has to exclude, and
//! travels with every message the core serves (see
//! [`crate::thread_read::thread_message_json`]) so no frontend recomputes it.

use std::collections::HashSet;

use serde_json::{Value, json};

/// The recipients of one reply, as address-list strings with display names
/// intact — exactly what a composer's To and Cc fields hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyRecipients {
    pub to: String,
    pub cc: String,
}

/// The address headers of the message being replied to.
pub struct ReplyTarget<'a> {
    pub from_name: &'a str,
    pub from_addr: &'a str,
    pub reply_to: &'a str,
    pub to: &'a str,
    pub cc: &'a str,
}

/// Build the To and Cc of a reply.
///
/// To is the target's Reply-To, or its From when that header is absent. Cc is
/// the target's Cc, minus anyone already addressed. Our own addresses stay out
/// of both: a reply that copies us back is noise.
///
/// When the target is one of ours — replying inside a Sent thread, where every
/// message is ours — the reply is a follow-up instead, addressed to the people
/// the original went to rather than back to ourselves.
///
/// `reply_all` keeps the other original recipients, everyone the message was
/// addressed to alongside us, whom a plain reply drops. They are copied, not
/// addressed, as Gmail and the rest do: the reply still goes to the sender
/// alone.
pub fn reply_recipients(
    target: &ReplyTarget,
    ours: &HashSet<String>,
    reply_all: bool,
) -> ReplyRecipients {
    let own_target = ours.contains(&bare_addr(target.from_addr));
    let addressed: Vec<String> = if own_target {
        split_address_list(target.to)
    } else {
        let reply_to = split_address_list(target.reply_to);
        if reply_to.is_empty() {
            vec![from_entry(target)]
        } else {
            reply_to
        }
    };

    let mut to_list = Vec::new();
    let mut to_addrs = HashSet::new();
    for entry in &addressed {
        let addr = bare_addr(entry);
        if addr.is_empty() || ours.contains(&addr) || !to_addrs.insert(addr) {
            continue;
        }
        to_list.push(entry.clone());
    }
    // Everyone the reply would address is us — a note to self, or a Sent message
    // whose only recipient was another of our own addresses. Reply to it anyway
    // rather than handing the composer an empty To line.
    if to_list.is_empty() {
        for entry in &addressed {
            let addr = bare_addr(entry);
            if addr.is_empty() || !to_addrs.insert(addr) {
                continue;
            }
            to_list.push(entry.clone());
        }
    }

    let copied = if reply_all && !own_target {
        let mut all = split_address_list(target.to);
        all.extend(split_address_list(target.cc));
        all
    } else {
        split_address_list(target.cc)
    };
    let mut cc_list = Vec::new();
    let mut cc_addrs = HashSet::new();
    for entry in copied {
        let addr = bare_addr(&entry);
        if addr.is_empty()
            || ours.contains(&addr)
            || to_addrs.contains(&addr)
            || !cc_addrs.insert(addr)
        {
            continue;
        }
        cc_list.push(entry);
    }

    ReplyRecipients {
        to: to_list.join(", "),
        cc: cc_list.join(", "),
    }
}

/// Whether replying to all would reach anyone a plain reply does not. False
/// makes the two actions identical, and the menus hide reply-all rather than
/// offering a second way to do the same thing.
pub fn reply_all_adds_recipients(target: &ReplyTarget, ours: &HashSet<String>) -> bool {
    adds_recipients(
        &reply_recipients(target, ours, false),
        &reply_recipients(target, ours, true),
    )
}

/// Both reply forms of one message, as the frontends receive them: `to`/`cc`
/// for a plain reply, `all_to`/`all_cc` for reply-all, and whether reply-all is
/// worth offering at all.
pub fn reply_json(target: &ReplyTarget, ours: &HashSet<String>) -> Value {
    let reply = reply_recipients(target, ours, false);
    let all = reply_recipients(target, ours, true);
    json!({
        "to": reply.to,
        "cc": reply.cc,
        "all_to": all.to,
        "all_cc": all.cc,
        "all_adds_recipients": adds_recipients(&reply, &all),
    })
}

fn adds_recipients(reply: &ReplyRecipients, all: &ReplyRecipients) -> bool {
    let existing: HashSet<String> = addresses_of(reply).collect();
    addresses_of(all).any(|addr| !existing.contains(&addr))
}

fn addresses_of(recipients: &ReplyRecipients) -> impl Iterator<Item = String> + '_ {
    split_address_list(&recipients.to)
        .into_iter()
        .chain(split_address_list(&recipients.cc))
        .map(|entry| bare_addr(&entry))
}

fn from_entry(target: &ReplyTarget) -> String {
    let name = target.from_name.trim();
    let addr = target.from_addr.trim();
    if name.is_empty() {
        addr.to_string()
    } else {
        format!("{} <{addr}>", display_name_entry(name))
    }
}

/// A display name as it may appear in an address list. `from_name` is the
/// decoded header text, so a name holding one of RFC 5322's specials — most
/// often `Doe, Jane` — has to be quoted before it goes back into a list, or the
/// comma splits it into a recipient named `Doe` that no address validation
/// accepts.
fn display_name_entry(name: &str) -> String {
    if !name.contains(|c: char| "()<>[]:;@\\,.\"".contains(c) || c.is_control()) {
        return name.to_string();
    }
    let mut quoted = String::with_capacity(name.len() + 2);
    quoted.push('"');
    for ch in name.chars() {
        if ch.is_control() {
            continue;
        }
        if ch == '"' || ch == '\\' {
            quoted.push('\\');
        }
        quoted.push(ch);
    }
    quoted.push('"');
    quoted
}

/// Split a `Name <addr>, addr2` list into its entries, keeping each one's own
/// text and dropping empties.
///
/// A comma inside a quoted display name (`"Doe, Jane" <jane@x.com>`) or inside
/// angle brackets does not separate entries: splitting there produces fragments
/// that match no address, survive the own-address filtering, and go out as
/// malformed recipients. Only the double quote opens a quoted string (RFC 5322);
/// an apostrophe is an ordinary character in a name like O'Connor, and treating
/// it as a delimiter swallows every recipient after it.
pub fn split_address_list(raw: &str) -> Vec<String> {
    let mut entries = Vec::new();
    let mut quoted = false;
    let mut angle_depth = 0usize;
    let mut start = 0usize;
    let mut chars = raw.char_indices();
    while let Some((index, ch)) = chars.next() {
        if quoted {
            match ch {
                '\\' => {
                    chars.next();
                }
                '"' => quoted = false,
                _ => {}
            }
            continue;
        }
        match ch {
            '"' => quoted = true,
            '<' => angle_depth += 1,
            '>' if angle_depth > 0 => angle_depth -= 1,
            ',' if angle_depth == 0 => {
                push_entry(&mut entries, &raw[start..index]);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    push_entry(&mut entries, &raw[start..]);
    entries
}

fn push_entry(entries: &mut Vec<String>, slice: &str) {
    let entry = slice.trim();
    if !entry.is_empty() {
        entries.push(entry.to_string());
    }
}

/// The bare address of a `Name <addr>` or `addr` entry, lowercased, for
/// comparing recipients regardless of how they were written.
pub fn bare_addr(entry: &str) -> String {
    let trimmed = entry.trim();
    if let Some(start) = trimmed.find('<')
        && let Some(end) = trimmed[start + 1..].find('>')
    {
        return trimmed[start + 1..start + 1 + end].trim().to_lowercase();
    }
    trimmed.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ours() -> HashSet<String> {
        ["me@example.com", "sales@example.com"]
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    fn incoming() -> ReplyTarget<'static> {
        ReplyTarget {
            from_name: "Them",
            from_addr: "them@example.com",
            reply_to: "",
            to: "me@example.com, Alice <alice@example.com>",
            cc: "bob@example.com",
        }
    }

    #[test]
    fn plain_reply_addresses_the_sender_and_keeps_the_cc() {
        let reply = reply_recipients(&incoming(), &ours(), false);
        assert_eq!(reply.to, "Them <them@example.com>");
        assert_eq!(reply.cc, "bob@example.com");
    }

    #[test]
    fn reply_all_copies_the_other_recipients_without_our_own() {
        let reply = reply_recipients(&incoming(), &ours(), true);
        assert_eq!(reply.to, "Them <them@example.com>");
        assert_eq!(reply.cc, "Alice <alice@example.com>, bob@example.com");
    }

    #[test]
    fn reply_to_header_wins_over_from() {
        let target = ReplyTarget {
            reply_to: "List <list@example.com>",
            ..incoming()
        };
        assert_eq!(
            reply_recipients(&target, &ours(), true).to,
            "List <list@example.com>"
        );
    }

    #[test]
    fn a_sender_listed_in_to_is_not_copied_as_well() {
        let target = ReplyTarget {
            from_name: "",
            from_addr: "them@example.com",
            reply_to: "",
            to: "them@example.com, sales@example.com, alice@example.com",
            cc: "",
        };
        let reply = reply_recipients(&target, &ours(), true);
        assert_eq!(reply.to, "them@example.com");
        assert_eq!(reply.cc, "alice@example.com");
    }

    #[test]
    fn our_own_message_is_a_follow_up_to_its_recipients() {
        let target = ReplyTarget {
            from_name: "Me",
            from_addr: "me@example.com",
            reply_to: "",
            to: "them@example.com, alice@example.com",
            cc: "bob@example.com",
        };
        let reply = reply_recipients(&target, &ours(), true);
        assert_eq!(reply.to, "them@example.com, alice@example.com");
        assert_eq!(reply.cc, "bob@example.com");
    }

    #[test]
    fn a_note_to_self_still_has_a_recipient() {
        let target = ReplyTarget {
            from_name: "Me",
            from_addr: "me@example.com",
            reply_to: "",
            to: "sales@example.com",
            cc: "",
        };
        assert_eq!(
            reply_recipients(&target, &ours(), false).to,
            "sales@example.com"
        );
    }

    #[test]
    fn reply_all_is_offered_only_when_it_reaches_someone_new() {
        let only_us = ReplyTarget {
            to: "me@example.com",
            cc: "",
            ..incoming()
        };
        assert!(!reply_all_adds_recipients(&only_us, &ours()));
        // Our own alias alongside us is nobody new.
        let alias = ReplyTarget {
            to: "me@example.com, sales@example.com",
            cc: "",
            ..incoming()
        };
        assert!(!reply_all_adds_recipients(&alias, &ours()));
        // A Cc-only third party is already kept by a plain reply.
        let copied = ReplyTarget {
            to: "me@example.com",
            cc: "bob@example.com",
            ..incoming()
        };
        assert!(!reply_all_adds_recipients(&copied, &ours()));
        // The same person in To and Cc, written differently, is still one person.
        let restated = ReplyTarget {
            to: "me@example.com, Bob <BOB@example.com>",
            cc: "bob@example.com",
            ..incoming()
        };
        assert!(!reply_all_adds_recipients(&restated, &ours()));
        assert!(reply_all_adds_recipients(&incoming(), &ours()));
    }

    #[test]
    fn a_comma_inside_a_quoted_name_does_not_split_the_entry() {
        assert_eq!(
            split_address_list("\"Doe, Jane\" <jane@x.com>, bob@x.com"),
            vec!["\"Doe, Jane\" <jane@x.com>", "bob@x.com"]
        );
        assert_eq!(split_address_list("O'Connor <pat@x.com>").len(), 1);
        assert_eq!(split_address_list("  ,  ").len(), 0);
    }

    #[test]
    fn a_display_name_with_a_comma_is_quoted_as_one_recipient() {
        let target = ReplyTarget {
            from_name: "Doe, Jane",
            from_addr: "jane@example.com",
            reply_to: "",
            to: "me@example.com",
            cc: "",
        };
        let reply = reply_recipients(&target, &ours(), false);
        assert_eq!(reply.to, "\"Doe, Jane\" <jane@example.com>");
        assert_eq!(split_address_list(&reply.to).len(), 1);
        assert_eq!(bare_addr(&reply.to), "jane@example.com");

        let quoting = ReplyTarget {
            from_name: "Jane \"J\" Doe",
            ..target
        };
        assert_eq!(
            reply_recipients(&quoting, &ours(), false).to,
            "\"Jane \\\"J\\\" Doe\" <jane@example.com>"
        );
    }

    #[test]
    fn the_bridge_field_carries_both_reply_forms() {
        let value = reply_json(&incoming(), &ours());
        assert_eq!(value["to"], "Them <them@example.com>");
        assert_eq!(value["cc"], "bob@example.com");
        assert_eq!(value["all_to"], "Them <them@example.com>");
        assert_eq!(
            value["all_cc"],
            "Alice <alice@example.com>, bob@example.com"
        );
        assert_eq!(value["all_adds_recipients"], true);
    }
}
