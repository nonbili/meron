use super::*;

pub async fn list_folders(session: &mut Session) -> Result<Vec<Folder>> {
    let mut out = Vec::new();
    let mut stream = session.list(Some(""), Some("*")).await.context("LIST")?;
    while let Some(item) = stream.next().await {
        let name = item.context("LIST item")?;
        // One role is stored per mailbox. \Flagged (a virtual "starred" view)
        // yields to a real role listed beside it, so a `(\Flagged \Trash)`
        // Trash is still the Trash `find_trash_folder` resolves.
        let noselect = name
            .attributes()
            .iter()
            .any(|attr| matches!(attr, async_imap::imap_proto::NameAttribute::NoSelect));
        let special_use = noselect
            .then_some("noselect")
            .or_else(|| {
                name.attributes()
                    .iter()
                    .find_map(|attr| {
                        use async_imap::imap_proto::NameAttribute;
                        match attr {
                            NameAttribute::Drafts => Some("drafts"),
                            NameAttribute::Sent => Some("sent"),
                            NameAttribute::Trash => Some("trash"),
                            NameAttribute::Junk => Some("junk"),
                            NameAttribute::Archive => Some("archive"),
                            NameAttribute::All => Some("all"),
                            _ => None,
                        }
                    })
                    .or_else(|| {
                        name.attributes()
                            .iter()
                            .any(|attr| {
                                matches!(attr, async_imap::imap_proto::NameAttribute::Flagged)
                            })
                            .then_some("flagged")
                    })
            })
            .map(str::to_string);
        out.push(Folder {
            display_name: crate::utf7::decode(name.name()),
            name: name.name().to_string(),
            delimiter: name.delimiter().map(|d| d.to_string()),
            unread: 0,
            special_use,
            role: String::new(),
            notify: false,
        });
    }
    Ok(out)
}

pub async fn create_folder(session: &mut Session, name: &str) -> Result<()> {
    session.create(name).await.context("CREATE")?;
    Ok(())
}

/// Rename a folder on the server. RFC 3501 renames its inferiors with it. Run
/// [`prepare_folder_delete`] first: like DELETE, some servers refuse to rename
/// the mailbox the session has selected.
pub async fn rename_folder(session: &mut Session, from: &str, to: &str) -> Result<()> {
    session.rename(from, to).await.context("RENAME")?;
    Ok(())
}

/// A multi-folder delete that failed after removing part of its target list.
/// Callers must reconcile `removed` before surfacing the failure.
#[derive(Debug)]
pub struct PartialFolderDelete {
    pub removed: Vec<String>,
    pub(super) failure: String,
}

impl std::fmt::Display for PartialFolderDelete {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} folder(s) were deleted before the server rejected the operation: {}",
            self.removed.len(),
            self.failure
        )
    }
}

impl std::error::Error for PartialFolderDelete {}

impl PartialFolderDelete {
    pub fn into_parts(self) -> (Vec<String>, String) {
        let warning = self.to_string();
        (self.removed, warning)
    }
}

/// Move a reused session away from any mailbox that is about to be deleted.
/// This is a read-only preflight so the session pool may safely repeat it on a
/// fresh connection when a warm connection has gone stale.
pub async fn prepare_folder_delete(session: &mut Session) -> Result<()> {
    // Pooled sessions retain their selected mailbox. Some servers refuse to
    // delete that mailbox, so safely move the session to INBOX first. EXAMINE
    // implicitly deselects without expunging messages marked \Deleted.
    session
        .examine("INBOX")
        .await
        .context("EXAMINE INBOX before DELETE")?;
    Ok(())
}

/// Delete folders on the server, messages and all. The caller must first run
/// [`prepare_folder_delete`] on the same session. `names` is deleted in order,
/// so a subtree must arrive deepest first — RFC 3501 lets a server refuse DELETE
/// on a mailbox that still has inferiors. Returns the names actually removed;
/// if a later command fails, the error carries the successful prefix so callers
/// can reconcile irreversible changes. Callers must refuse special-use folders
/// first: IMAP happily deletes Sent or Archive, and nothing on the server brings
/// them back.
pub async fn delete_folders(session: &mut Session, names: &[String]) -> Result<Vec<String>> {
    let mut removed = Vec::new();
    for name in names {
        if let Err(err) = session.delete(name).await {
            let failure = format!("DELETE {name}: {err:#}");
            if removed.is_empty() {
                return Err(anyhow::anyhow!(failure));
            }
            return Err(PartialFolderDelete { removed, failure }.into());
        }
        removed.push(name.clone());
    }
    Ok(removed)
}

/// Whether `err` is a FETCH response our IMAP parser could not read, as opposed
/// to a network or server failure.
///
/// This matters because such a response is unrecoverable *on that connection*:
/// async-imap reports the parse failure without consuming the offending bytes
/// from its read buffer, so every later poll re-parses them and fails again.
/// The only way forward is to drop the session and re-fetch a range that
/// excludes the message — see `fetch_headers_isolating_unparseable` in
/// `engine/sync.rs`.
///
/// Detection is by message text because async-imap surfaces the failure as a
/// plain `io::Error::other`, with no variant to match on. The substring is the
/// literal from its `ImapStream::decode`; if the pinned fork moves, this needs
/// rechecking (`unparseable_response_marker_matches_async_imap` covers it).
pub fn is_unparseable_response(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io| io.to_string().contains(UNPARSEABLE_RESPONSE_MARKER))
    })
}

pub(super) const UNPARSEABLE_RESPONSE_MARKER: &str = " during parsing of ";

/// Best-effort resolver for an account's Sent folder. Prefers IMAP SPECIAL-USE
/// (`\Sent` attribute) when the server advertises it; falls back to common name
/// patterns (Gmail's `[Gmail]/Sent Mail`, Dovecot's `Sent`, Courier's `INBOX.Sent`).
/// Returns `None` when the server has no LIST entries matching either path.
pub async fn find_sent_folder(session: &mut Session) -> Result<Option<String>> {
    let mut stream = session.list(Some(""), Some("*")).await.context("LIST")?;
    let mut by_attr: Option<String> = None;
    let mut by_name: Option<String> = None;
    while let Some(item) = stream.next().await {
        let entry = item.context("LIST item")?;
        let name = entry.name().to_string();
        for attr in entry.attributes() {
            if matches!(attr, async_imap::imap_proto::NameAttribute::Sent) {
                by_attr = Some(name.clone());
            }
        }
        if by_name.is_none() && looks_like_sent(&name) {
            by_name = Some(name);
        }
    }
    drop(stream);
    Ok(by_attr.or(by_name))
}

pub async fn find_trash_folder(session: &mut Session) -> Result<Option<String>> {
    let mut stream = session.list(Some(""), Some("*")).await.context("LIST")?;
    let mut by_attr: Option<String> = None;
    let mut by_name: Option<String> = None;
    while let Some(item) = stream.next().await {
        let entry = item.context("LIST item")?;
        let name = entry.name().to_string();
        for attr in entry.attributes() {
            if matches!(attr, async_imap::imap_proto::NameAttribute::Trash) {
                by_attr = Some(name.clone());
            }
        }
        if by_name.is_none() && looks_like_trash(&name) {
            by_name = Some(name);
        }
    }
    drop(stream);
    Ok(by_attr.or(by_name))
}

pub async fn find_archive_folder(session: &mut Session) -> Result<Option<String>> {
    let mut stream = session.list(Some(""), Some("*")).await.context("LIST")?;
    // A real \Archive folder wins over \All (Gmail's All Mail), whatever
    // order the server lists them in, as `engine::cached_archive_folder_from_folders`.
    let mut by_archive: Option<String> = None;
    let mut by_all: Option<String> = None;
    let mut by_name: Option<String> = None;
    while let Some(item) = stream.next().await {
        let entry = item.context("LIST item")?;
        let name = entry.name().to_string();
        for attr in entry.attributes() {
            match attr {
                async_imap::imap_proto::NameAttribute::Archive if by_archive.is_none() => {
                    by_archive = Some(name.clone());
                }
                async_imap::imap_proto::NameAttribute::All if by_all.is_none() => {
                    by_all = Some(name.clone());
                }
                _ => {}
            }
        }
        if by_name.is_none() && looks_like_archive(&name) {
            by_name = Some(name);
        }
    }
    drop(stream);
    Ok(by_archive.or(by_all).or(by_name))
}

pub async fn find_drafts_folder(session: &mut Session) -> Result<Option<String>> {
    let mut stream = session.list(Some(""), Some("*")).await.context("LIST")?;
    let mut by_attr: Option<String> = None;
    let mut by_name: Option<String> = None;
    while let Some(item) = stream.next().await {
        let entry = item.context("LIST item")?;
        let name = entry.name().to_string();
        for attr in entry.attributes() {
            if matches!(attr, async_imap::imap_proto::NameAttribute::Drafts) {
                by_attr = Some(name.clone());
            }
        }
        if by_name.is_none() && looks_like_drafts(&name) {
            by_name = Some(name);
        }
    }
    drop(stream);
    Ok(by_attr.or(by_name))
}

pub fn looks_like_archive(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    matches!(
        n.as_str(),
        "archive"
            | "archives"
            | "all mail"
            | "inbox.archive"
            | "inbox.archives"
            | "[gmail]/all mail"
            | "[google mail]/all mail"
    )
}

pub fn looks_like_sent(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    // Gmail's localized account uses "[Gmail]/Sent Mail"; Dovecot ships "Sent";
    // Courier/Cyrus often namespace under INBOX (`INBOX.Sent`).
    matches!(
        n.as_str(),
        "sent" | "sent mail" | "sent messages" | "sent items" | "inbox.sent" | "[gmail]/sent mail"
    )
}

pub fn looks_like_trash(name: &str) -> bool {
    // Servers without SPECIAL-USE leave only the name to go by, and many name
    // the mailbox in the account's language ("Papierkorb", "Corbeille"), sent
    // as modified UTF-7 when it is not ASCII ("Gel&APY-scht").
    let decoded = crate::utf7::decode(name).to_lowercase();
    let leaf = ["inbox.", "inbox/", "[gmail]/", "[google mail]/"]
        .iter()
        .find_map(|prefix| decoded.strip_prefix(prefix))
        .unwrap_or(&decoded);
    matches!(
        leaf,
        "trash"
            | "deleted"
            | "deleted items"
            | "deleted messages"
            | "bin"
            // German
            | "papierkorb"
            | "gelöscht"
            | "gelöschte elemente"
            | "gelöschte objekte"
            | "gelöschte nachrichten"
            // French
            | "corbeille"
            | "éléments supprimés"
            | "messages supprimés"
            // Spanish
            | "papelera"
            | "elementos eliminados"
            // Italian
            | "cestino"
            | "posta eliminata"
            // Dutch
            | "prullenbak"
            | "verwijderde items"
            // Portuguese
            | "lixeira"
            | "lixo"
            | "itens excluídos"
            | "itens eliminados"
            // Polish
            | "kosz"
            // Russian, Ukrainian
            | "корзина"
            | "удаленные"
            | "удалённые"
            | "кошик"
    )
}

pub fn looks_like_junk(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    matches!(
        n.as_str(),
        "junk" | "spam" | "bulk mail" | "inbox.junk" | "inbox.spam" | "[gmail]/spam"
    )
}

pub fn looks_like_drafts(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    matches!(
        n.as_str(),
        "drafts" | "draft" | "inbox.drafts" | "inbox.draft" | "[gmail]/drafts" | "[gmail]/draft"
    )
}
