use super::*;
use crate::openpgp::keyring::NoKeyReason;

/// A store as the app opens it: the OpenPGP tables live beside the mail
/// cache, which adding a key also touches (`recheck_signed_by`).
fn db() -> Connection {
    crate::store::open_at(":memory:").unwrap()
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/openpgp/testdata")
            .join(name),
    )
    .expect("fixture")
}

#[test]
fn create_key_enables_the_account_and_loads_it() {
    let conn = db();
    let info = create_key(&conn, "acc", &["me@example.org".into()]).unwrap();
    let ring = load(&conn).unwrap();
    assert!(ring.is_enabled("acc"));
    assert_eq!(
        ring.default_key("acc").unwrap().fingerprint,
        info.fingerprint
    );
    let state = account_state(&conn, "acc").unwrap();
    assert!(state.enabled);
    assert_eq!(state.key.unwrap().fingerprint, info.fingerprint);
}

#[test]
fn replacing_a_key_keeps_the_old_one_for_decrypting() {
    let conn = db();
    let first = create_key(&conn, "acc", &["me@example.org".into()]).unwrap();
    let second = create_key(&conn, "acc", &["me@example.org".into()]).unwrap();
    let ring = load(&conn).unwrap();
    assert_eq!(ring.own.len(), 2);
    assert_eq!(
        ring.default_key("acc").unwrap().fingerprint,
        second.fingerprint
    );
    assert_eq!(
        account_state(&conn, "acc").unwrap().previous[0].fingerprint,
        first.fingerprint
    );
}

#[test]
fn importing_needs_the_passphrase_then_stores_the_key() {
    let conn = db();
    let data = fixture("bob-secret-protected.asc");
    assert!(matches!(
        import_own(
            &conn,
            "acc",
            &["bob@example.net".into()],
            &data,
            None,
            None,
            None
        )
        .unwrap(),
        ImportOwn::NeedsPassphrase { .. }
    ));
    assert!(matches!(
        import_own(&conn, "acc", &[], &data, Some("nope"), None, None).unwrap(),
        ImportOwn::NeedsPassphrase { .. }
    ));
    let ImportOwn::Imported {
        address_matches, ..
    } = import_own(
        &conn,
        "acc",
        &["Bob@Example.net".into()],
        &data,
        Some("correct horse battery staple"),
        None,
        None,
    )
    .unwrap()
    else {
        panic!("expected import");
    };
    assert!(address_matches);
    assert!(load(&conn).unwrap().is_enabled("acc"));
}

#[test]
fn a_file_with_two_keys_asks_which_one() {
    let conn = db();
    let data = fixture("two-secret-keys.asc");
    let ImportOwn::Choose { keys } =
        import_own(&conn, "acc", &[], &data, None, None, None).unwrap()
    else {
        panic!()
    };
    assert_eq!(keys.len(), 2);
    let pick = keys
        .iter()
        .find(|k| k.emails.contains(&"alice@example.org".to_string()))
        .unwrap();
    assert!(matches!(
        import_own(
            &conn,
            "acc",
            &[],
            &data,
            None,
            None,
            Some(&pick.fingerprint)
        )
        .unwrap(),
        ImportOwn::Imported { .. }
    ));
}

#[test]
fn subkeys_only_export_is_refused_as_on_card() {
    let conn = db();
    assert!(matches!(
        import_own(
            &conn,
            "acc",
            &[],
            &fixture("alice-secret-subkeys-only.asc"),
            None,
            None,
            None
        )
        .unwrap(),
        ImportOwn::OnCard
    ));
}

#[test]
fn a_corrupted_stored_key_is_never_loaded() {
    let conn = db();
    create_key(&conn, "acc", &["me@example.org".into()]).unwrap();
    let mut blob: Vec<u8> = conn
        .query_row("SELECT secret_key FROM openpgp_own_keys", [], |r| r.get(0))
        .unwrap();
    // Flip bits throughout the secret scalar area near the end.
    let n = blob.len();
    for b in &mut blob[n - 40..n - 8] {
        *b ^= 0x5a;
    }
    conn.execute("UPDATE openpgp_own_keys SET secret_key = ?1", [&blob])
        .unwrap();
    assert!(
        load(&conn).unwrap().own.is_empty(),
        "a key failing to parse or self-test must not be used"
    );
}

#[test]
fn autocrypt_tofu_adds_new_keys_and_never_moves_last_seen_back() {
    let conn = db();
    let raw = fixture("autocrypt-plain.eml");
    let mail = mailparse::parse_mail(&raw).unwrap();
    let ann = super::super::autocrypt::announced(&mail.headers, "carol@example.com");
    assert!(ingest(&conn, &ann, &[], 2_000).unwrap());
    assert!(
        !ingest(&conn, &ann, &[], 1_000).unwrap(),
        "older mail must not count as a change"
    );
    let ring = load(&conn).unwrap();
    assert_eq!(ring.peers_for("carol@example.com")[0].last_seen, 2_000);
    assert!(ring.encryption_key_for("carol@example.com").is_ok());
}

#[test]
fn a_future_date_header_cannot_pin_a_key_as_the_newest() {
    // The Date header is the sender's claim. A forged mail dated
    // far ahead must not make its key "seen last" forever, or the real
    // owner's later mail could never take encryption back.
    let conn = db();
    let carol = keys::parse_public(&fixture("carol-public.asc")).unwrap();
    let mallory = keys::parse_public(&fixture("mallory-public.asc")).unwrap();
    let carol_fpr = fingerprint_hex(&carol.primary_key);
    let ann = |key| {
        [Announced {
            addr: "carol@example.com".into(),
            key,
            prefer_mutual: true,
            gossip: false,
        }]
    };
    ingest(&conn, &ann(carol.clone()), &[], 10).unwrap();
    ingest(&conn, &ann(mallory), &[], 4_102_444_800).unwrap(); // Date: 2100-01-01
    let ring = load(&conn).unwrap();
    assert!(
        ring.peers_for("carol@example.com")
            .iter()
            .all(|p| p.last_seen <= now_secs()),
        "no stored time may lie ahead of now"
    );
    // An hour passes (shift the stored times back), then Carol writes again:
    // her real mail is newer than the forged one and takes encryption back.
    conn.execute(
        "UPDATE openpgp_peer_keys SET last_seen = last_seen - 3600",
        [],
    )
    .unwrap();
    ingest(&conn, &ann(carol), &[], now_secs()).unwrap();
    let ring = load(&conn).unwrap();
    assert_eq!(
        ring.encryption_key_for("carol@example.com")
            .unwrap()
            .fingerprint,
        carol_fpr
    );
}

#[test]
fn a_key_other_than_the_one_last_used_is_reported_until_it_is_used() {
    // The composer warns when the key it would encrypt
    // to isn't the one Meron last encrypted to for that address.
    let conn = db();
    let carol = "carol@example.com".to_string();
    let chosen = |fpr: &str| vec![(carol.clone(), fpr.to_string())];
    assert!(
        key_changes(&conn, &chosen("AAAA")).unwrap().is_empty(),
        "first contact: nothing to compare"
    );
    record_used(&conn, &chosen("AAAA")).unwrap();
    assert!(key_changes(&conn, &chosen("AAAA")).unwrap().is_empty());
    let changes = key_changes(&conn, &chosen("BBBB")).unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(
        (
            changes[0].addr.as_str(),
            changes[0].previous.as_str(),
            changes[0].current.as_str()
        ),
        ("carol@example.com", "AAAA", "BBBB")
    );
    record_used(&conn, &chosen("BBBB")).unwrap();
    assert!(
        key_changes(&conn, &chosen("BBBB")).unwrap().is_empty(),
        "sending to the new key settles it"
    );
}

#[test]
fn gossip_never_replaces_a_direct_key() {
    let conn = db();
    let carol = keys::parse_public(&fixture("carol-public.asc")).unwrap();
    let mallory = keys::parse_public(&fixture("mallory-public.asc")).unwrap();
    ingest(
        &conn,
        &[Announced {
            addr: "carol@example.com".into(),
            key: carol,
            prefer_mutual: true,
            gossip: false,
        }],
        &[],
        10,
    )
    .unwrap();
    ingest(
        &conn,
        &[Announced {
            addr: "carol@example.com".into(),
            key: mallory,
            prefer_mutual: true,
            gossip: true,
        }],
        &[],
        20,
    )
    .unwrap();
    assert_eq!(load(&conn).unwrap().peers_for("carol@example.com").len(), 1);
}

#[test]
fn verified_key_stays_preferred_over_a_newer_one() {
    let conn = db();
    let carol = keys::parse_public(&fixture("carol-public.asc")).unwrap();
    let carol_new = keys::parse_public(&fixture("carol-new-public.asc")).unwrap();
    let fpr = fingerprint_hex(&carol.primary_key);
    let new_fpr = fingerprint_hex(&carol_new.primary_key);
    ingest(
        &conn,
        &[Announced {
            addr: "carol@example.com".into(),
            key: carol,
            prefer_mutual: true,
            gossip: false,
        }],
        &[],
        10,
    )
    .unwrap();
    assert!(
        set_verified(&conn, "carol@example.com", &fpr, "0000", true).is_err(),
        "wrong confirmation must be refused"
    );
    let spaced: String = fpr
        .chars()
        .collect::<Vec<_>>()
        .chunks(4)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    set_verified(&conn, "carol@example.com", &fpr, &spaced, true).unwrap();
    assert!(
        ingest(
            &conn,
            &[Announced {
                addr: "carol@example.com".into(),
                key: carol_new,
                prefer_mutual: true,
                gossip: false,
            }],
            &[],
            20,
        )
        .unwrap()
    );
    let ring = load(&conn).unwrap();
    assert_eq!(
        ring.encryption_key_for("carol@example.com")
            .unwrap()
            .fingerprint,
        fpr
    );
    assert!(
        list_peers(&conn)
            .unwrap()
            .iter()
            .any(|r| r.info.fingerprint == new_fpr && r.conflicts_with_verified)
    );
}

#[test]
fn keyring_import_preview_skips_expired_and_revoked_and_imports_the_rest() {
    let conn = db();
    let data = fixture("contacts-keyring.asc");
    let preview = import_preview(&conn, &data).unwrap();
    assert_eq!(preview.ready.len(), 3);
    let mut reasons: Vec<SkipReason> = preview.skipped.iter().map(|s| s.reason).collect();
    reasons.sort_by_key(|r| *r as u8);
    assert_eq!(reasons, vec![SkipReason::Expired, SkipReason::Revoked]);
    let fprs: Vec<String> = preview
        .ready
        .iter()
        .map(|k| k.fingerprint.clone())
        .collect();
    assert_eq!(import_peers(&conn, &data, &fprs).unwrap(), 3);
    let again = import_preview(&conn, &data).unwrap();
    assert_eq!(again.already_present.len(), 3);
    assert!(again.ready.is_empty());
}

#[test]
fn a_revocation_arriving_later_takes_effect_and_an_older_copy_cannot_undo_it() {
    let conn = db();
    let carol = keys::parse_public(&fixture("carol-public.asc")).unwrap();
    let revoked = keys::parse_public(&fixture("carol-revoked-public.asc")).unwrap();
    let ann = |key: &pgp::composed::SignedPublicKey| Announced {
        addr: "carol@example.com".into(),
        key: key.clone(),
        prefer_mutual: true,
        gossip: false,
    };
    ingest(&conn, &[ann(&carol)], &[], 10).unwrap();
    assert!(
        load(&conn)
            .unwrap()
            .encryption_key_for("carol@example.com")
            .is_ok()
    );
    assert!(
        ingest(&conn, &[ann(&revoked)], &[], 20).unwrap(),
        "the revocation is a change"
    );
    let ring = load(&conn).unwrap();
    assert_eq!(
        ring.encryption_key_for("carol@example.com").err(),
        Some(NoKeyReason::Unusable)
    );
    assert_eq!(
        list_peers(&conn).unwrap()[0].info.unusable,
        Some(Unusable::Revoked)
    );
    // Old mail fetched late, carrying the unrevoked copy with a newer date.
    ingest(&conn, &[ann(&carol)], &[], 30).unwrap();
    assert_eq!(
        load(&conn)
            .unwrap()
            .encryption_key_for("carol@example.com")
            .err(),
        Some(NoKeyReason::Unusable)
    );
}

#[test]
fn a_designated_revocation_arriving_by_mail_takes_effect_and_a_forged_one_does_not() {
    use crate::openpgp::keys::tests::revoke_by;
    let unlock = |name: &str| match keys::unlock(
        keys::parse_keys(&fixture(name)).unwrap().secret.remove(0),
        None,
    )
    .unwrap()
    {
        keys::Unlock::Ok(k) => k,
        other => panic!("{other:?}"),
    };
    let ann = |addr: &str, key: &pgp::composed::SignedPublicKey| Announced {
        addr: addr.into(),
        key: key.clone(),
        prefer_mutual: true,
        gossip: false,
    };
    let tess = keys::parse_public(&fixture("tess-public.asc")).unwrap();
    let dan = unlock("dan-secret.asc");
    let mallory = unlock("mallory-secret.asc");

    let conn = db();
    ingest(
        &conn,
        &[
            ann("tess@example.com", &tess),
            ann("dan@example.com", &dan.to_public_key()),
        ],
        &[],
        10,
    )
    .unwrap();
    // A forged revocation (Mallory isn't Tess's revoker) changes nothing.
    ingest(
        &conn,
        &[ann("tess@example.com", &revoke_by(&mallory, &tess))],
        &[],
        20,
    )
    .unwrap();
    let ring = load(&conn).unwrap();
    assert!(ring.encryption_key_for("tess@example.com").is_ok());
    // Dan's revocation is taken.
    assert!(
        ingest(
            &conn,
            &[ann("tess@example.com", &revoke_by(&dan, &tess))],
            &[],
            30
        )
        .unwrap()
    );
    let stored = load(&conn).unwrap().peers_for("tess@example.com")[0]
        .key
        .clone();
    assert_eq!(
        keys::check_with(&stored, keys::now(), &|f| db_key(&conn, f)),
        Some(Unusable::Revoked)
    );
}

/// A cached message row of `json` (with a body, so it reads as fetched).
fn cache_message(conn: &Connection, uid: u32, json: serde_json::Value) {
    conn.execute(
        "INSERT INTO messages (account, folder, msg_id, uid, subject, from_name, from_addr, date, body, json)
         VALUES ('a', 'INBOX', ?1, ?1, 's', 'n', 'x@example.com', 0, 'body', ?2)",
        params![uid, json.to_string()],
    )
    .unwrap();
}

fn body_cached(conn: &Connection, uid: u32) -> bool {
    conn.query_row(
        "SELECT body IS NOT NULL FROM messages WHERE uid = ?1",
        [uid],
        |r| r.get(0),
    )
    .unwrap()
}

// Our own key imported (or restored from a backup) after mail it opens or
// signed was cached: that mail is read again, not left "no key" or with an
// unknown signer.
#[test]
fn importing_or_restoring_an_own_key_has_its_mail_read_again() {
    let data = fixture("alice-secret.asc");
    let alice = keys::parse_keys(&data)
        .unwrap()
        .secret
        .remove(0)
        .to_public_key();
    let fpr = fingerprint_hex(&alice.primary_key);
    let seed = |conn: &Connection| {
        cache_message(
            conn,
            1,
            serde_json::json!({ "crypto": { "encrypted": true, "problem": "no_key" } }),
        );
        cache_message(
            conn,
            2,
            serde_json::json!({ "crypto": { "signed": true, "signer": null, "issuer": fpr } }),
        );
        cache_message(
            conn,
            3,
            serde_json::json!({ "crypto": { "signed": true, "signer": null, "issuer": "0123456789ABCDEF" } }),
        );
    };

    let conn = db();
    seed(&conn);
    let ImportOwn::Imported { .. } = import_own(
        &conn,
        "acct",
        &["alice@example.com".into()],
        &data,
        None,
        None,
        None,
    )
    .unwrap() else {
        panic!("imported");
    };
    assert!(!body_cached(&conn, 2), "signed with the imported key");
    assert!(body_cached(&conn, 3), "someone else's signature");

    let backup = backup_collect(&conn, true).unwrap();
    let fresh = db();
    seed(&fresh);
    backup_apply(&fresh, &backup).unwrap();
    assert!(!body_cached(&fresh, 1), "undecryptable until the restore");
    assert!(!body_cached(&fresh, 2), "signed with a restored key");
    assert!(body_cached(&fresh, 3));
}

// A renewed copy of a stored key merged in (a lookup, an import, Autocrypt)
// can verify a signature the old copy couldn't: such messages are read again.
#[test]
fn a_stored_key_updated_by_a_merge_has_its_unverified_messages_read_again() {
    let conn = db();
    let carol = keys::parse_public(&fixture("carol-public.asc")).unwrap();
    let fpr = fingerprint_hex(&carol.primary_key);
    import_peers(&conn, &fixture("carol-public.asc"), &[fpr.clone()]).unwrap();
    cache_message(
        &conn,
        1,
        serde_json::json!({
            "crypto": { "signed": true, "signer": null, "issuer": fpr, "problem": "key_expired" }
        }),
    );
    // Not picked: only merged into the stored copy (it carries something new).
    import_peers(&conn, &fixture("carol-revoked-public.asc"), &[]).unwrap();
    assert!(!body_cached(&conn, 1));
}

// A recipient's newest verified key expired while an older verified one is
// still good: encrypt to that one. And say why when none can be used.
#[test]
fn the_encryption_key_is_any_usable_verified_key_with_the_right_reason() {
    use crate::openpgp::keyring::{Keyring, PeerKey};
    let peer = |file: &str, verified: bool, last_seen: i64| {
        let key = keys::parse_public(&fixture(file)).unwrap();
        PeerKey {
            addr: "bob@example.com".into(),
            fingerprint: fingerprint_hex(&key.primary_key),
            key,
            source: Source::Imported,
            verified,
            last_seen,
        }
    };
    let ring = |peers: Vec<PeerKey>| Keyring {
        peers,
        ..Default::default()
    };
    let both = ring(vec![
        peer("expired-public.asc", true, 2),
        peer("carol-public.asc", true, 1),
    ]);
    let carol = keys::parse_public(&fixture("carol-public.asc")).unwrap();
    assert_eq!(
        both.encryption_key_for("bob@example.com")
            .unwrap()
            .fingerprint,
        fingerprint_hex(&carol.primary_key)
    );
    let only_expired = ring(vec![peer("expired-public.asc", true, 1)]);
    assert_eq!(
        only_expired.encryption_key_for("bob@example.com").err(),
        Some(NoKeyReason::Unusable)
    );
    let new_unverified = ring(vec![
        peer("carol-public.asc", false, 2),
        peer("expired-public.asc", true, 1),
    ]);
    assert_eq!(
        new_unverified.encryption_key_for("bob@example.com").err(),
        Some(NoKeyReason::NewKeyUnverified)
    );
}

// A message signed by a key we didn't have stays unverified until it is
// checked again: importing that key drops the cached body (the signed bytes
// aren't kept) so the next read fetches and verifies it. Messages signed by
// someone else, or already verified, keep theirs.
#[test]
fn adding_a_signers_key_has_its_unverified_messages_checked_again() {
    let conn = crate::store::open_at(":memory:").unwrap();
    let carol = keys::parse_public(&fixture("carol-public.asc")).unwrap();
    let fpr = fingerprint_hex(&carol.primary_key);
    let key_id = fpr[fpr.len() - 16..].to_string();
    let insert = |uid: u32, crypto: serde_json::Value| {
        conn.execute(
            "INSERT INTO messages (account, folder, msg_id, uid, subject, from_name, from_addr, date, body, json)
             VALUES ('a', 'INBOX', ?1, ?1, 's', 'n', 'carol@example.com', 0, 'body', ?2)",
            params![uid, serde_json::json!({ "crypto": crypto }).to_string()],
        )
        .unwrap();
    };
    insert(
        1,
        serde_json::json!({ "signed": true, "signer": null, "issuer": key_id }),
    );
    insert(
        2,
        serde_json::json!({ "signed": true, "signer": null, "issuer": fpr }),
    );
    insert(
        3,
        serde_json::json!({ "signed": true, "signer": null, "issuer": "0123456789ABCDEF" }),
    );
    insert(4, serde_json::json!({ "signed": true, "signer": fpr }));
    let cached = |uid: u32| -> bool {
        conn.query_row(
            "SELECT body IS NOT NULL AND json_extract(json, '$.crypto') IS NOT NULL
             FROM messages WHERE uid = ?1",
            [uid],
            |r| r.get(0),
        )
        .unwrap()
    };

    import_peers(&conn, &fixture("carol-public.asc"), &[fpr.clone()]).unwrap();

    assert!(!cached(1), "issuer named by key ID");
    assert!(!cached(2), "issuer named by fingerprint");
    assert!(cached(3), "someone else's signature");
    assert!(cached(4), "already verified");
}

// A draft saved encrypted is found by its bare Message-ID (how draft ids
// travel), so replacing it keeps it encrypted.
#[test]
fn an_encrypted_draft_is_found_by_its_bare_id() {
    let conn = crate::store::open_at(":memory:").unwrap();
    cache_message(
        &conn,
        1,
        serde_json::json!({ "message_id": "draft-1@example.com", "crypto": { "encrypted": true } }),
    );
    cache_message(
        &conn,
        2,
        serde_json::json!({ "message_id": "draft-2@example.com" }),
    );
    assert!(answers_encrypted(&conn, "a", "draft-1@example.com").unwrap());
    assert!(!answers_encrypted(&conn, "a", "draft-2@example.com").unwrap());
}

// Dropping an encrypted message's cached status for a new signature check
// must not make it look plain until it is fetched again: a reply sent in
// between (an MCP reply, say) would go out unencrypted.
#[test]
fn a_message_checked_again_still_answers_encrypted() {
    let conn = crate::store::open_at(":memory:").unwrap();
    let carol = keys::parse_public(&fixture("carol-public.asc")).unwrap();
    let fpr = fingerprint_hex(&carol.primary_key);
    let json = serde_json::json!({
        "message_id": "secret@example.com",
        "crypto": { "encrypted": true, "signed": true, "signer": null, "issuer": fpr },
    });
    conn.execute(
        "INSERT INTO messages (account, folder, msg_id, uid, subject, from_name, from_addr, date, body, json)
         VALUES ('a', 'INBOX', 1, 1, 's', 'n', 'carol@example.com', 0, 'body', ?1)",
        [json.to_string()],
    )
    .unwrap();
    let protection = |conn: &Connection| {
        reply_protection(conn, "a", "<secret@example.com>", false, false).unwrap()
    };
    assert_eq!(protection(&conn), (true, true));

    import_peers(&conn, &fixture("carol-public.asc"), &[fpr.clone()]).unwrap();

    let body_dropped: bool = conn
        .query_row("SELECT body IS NULL FROM messages", [], |r| r.get(0))
        .unwrap();
    assert!(body_dropped, "it is checked again");
    assert_eq!(protection(&conn), (true, true));
}

// "Use my existing key" with a renewed export of the stored key takes the
// renewal; importing the older export afterwards doesn't undo it.
#[test]
fn reimporting_an_own_key_keeps_the_newest_copy() {
    let conn = db();
    let stored_blob = |conn: &Connection| -> Vec<u8> {
        conn.query_row("SELECT secret_key FROM openpgp_own_keys", [], |r| r.get(0))
            .unwrap()
    };
    let key = keys::generate(&["me@example.org".into()]).unwrap();
    let renewed = keys::tests::self_certified_at(&key, keys::now() + 100);
    store_default_key(&conn, "acct", &key).unwrap();
    store_default_key(&conn, "acct", &renewed).unwrap();
    assert_eq!(
        stored_blob(&conn),
        keys::binary_secret(&renewed).unwrap().as_slice()
    );
    store_default_key(&conn, "acct", &key).unwrap();
    assert_eq!(
        stored_blob(&conn),
        keys::binary_secret(&renewed).unwrap().as_slice()
    );
}

#[test]
fn importing_a_revoked_copy_updates_the_stored_key() {
    let conn = db();
    let carol = keys::parse_public(&fixture("carol-public.asc")).unwrap();
    ingest(
        &conn,
        &[Announced {
            addr: "carol@example.com".into(),
            key: carol,
            prefer_mutual: true,
            gossip: false,
        }],
        &[],
        10,
    )
    .unwrap();
    let data = fixture("carol-revoked-public.asc");
    let preview = import_preview(&conn, &data).unwrap();
    assert_eq!(preview.updates.len(), 1);
    assert!(
        preview.ready.is_empty(),
        "revoked keys are never offered as new"
    );
    import_peers(&conn, &data, &[]).unwrap();
    assert_eq!(
        list_peers(&conn).unwrap()[0].info.unusable,
        Some(Unusable::Revoked)
    );
}

#[test]
fn cached_sender_mismatches_are_reparsed_once_and_keep_encryption() {
    let conn = db();
    cache_message(
        &conn,
        1,
        serde_json::json!({ "crypto": { "signed": true, "problem": "signer_mismatch" } }),
    );
    cache_message(
        &conn,
        2,
        serde_json::json!({ "crypto": { "encrypted": true, "signed": true, "problem": "signer_mismatch" } }),
    );
    cache_message(
        &conn,
        3,
        serde_json::json!({ "crypto": { "signed": true, "problem": "invalid_signature" } }),
    );
    assert_eq!(reparse_signer_mismatches_once(&conn).unwrap(), 2);
    assert!(!body_cached(&conn, 1));
    assert!(!body_cached(&conn, 2));
    assert!(body_cached(&conn, 3));
    let json: String = conn
        .query_row("SELECT json FROM messages WHERE uid = 2", [], |row| {
            row.get(0)
        })
        .unwrap();
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(json["encrypted"], true);
    assert!(json.get("crypto").is_none());
    // Even a subsequently cached mismatch is left alone on later starts.
    cache_message(
        &conn,
        4,
        serde_json::json!({ "crypto": { "signed": true, "problem": "signer_mismatch" } }),
    );
    assert_eq!(reparse_signer_mismatches_once(&conn).unwrap(), 0);
    assert!(body_cached(&conn, 4));
}

#[test]
fn cached_messages_with_key_attachments_are_reparsed_once() {
    let conn = crate::store::open_at(":memory:").unwrap();
    conn.execute(
        "INSERT INTO messages (account, folder, msg_id, uid, subject, from_name, from_addr, date, body, json, files)
         VALUES ('a', 'INBOX', 1, 1, 's', 'n', 'x@y.z', 0, 'body', '{\"body_html\":\"<p>x</p>\"}',
                 '[{\"filename\":\"OpenPGP_0x1.asc\",\"mime\":\"application/pgp-keys\"}]'),
                ('a', 'INBOX', 2, 2, 's', 'n', 'x@y.z', 0, 'other', '{}', NULL)",
        [],
    )
    .unwrap();
    assert_eq!(reparse_key_attachments_once(&conn).unwrap(), 1);
    let body: Option<String> = conn
        .query_row("SELECT body FROM messages WHERE uid = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(body, None);
    let other: Option<String> = conn
        .query_row("SELECT body FROM messages WHERE uid = 2", [], |r| r.get(0))
        .unwrap();
    assert_eq!(other.as_deref(), Some("other"));
    assert_eq!(reparse_key_attachments_once(&conn).unwrap(), 0, "only once");
}

// One query answers a whole page: plain mail, encrypted mail and uids the
// cache lacks, with trust judged against the stored sender (a Sent card's
// header names the recipient instead).
#[test]
fn card_locks_answers_a_page_in_one_pass() {
    let conn = crate::store::open_at(":memory:").unwrap();
    let insert = |uid: u32, folder: &str, json: &str| {
        conn.execute(
            "INSERT INTO messages (account, folder, msg_id, uid, subject, from_name, from_addr, date, json)
             VALUES ('a', ?2, ?1, ?1, 's', 'n', 'x@y.z', 0, ?3)",
            params![uid, folder, json],
        )
        .unwrap();
    };
    insert(1, "INBOX", r#"{"crypto":{"encrypted":true}}"#);
    insert(2, "INBOX", "{}");
    insert(
        3,
        "INBOX",
        r#"{"crypto":{"encrypted":false,"signed":true}}"#,
    );
    insert(4, "Sent", r#"{"crypto":{"encrypted":true}}"#);
    let ring = keyring::Keyring::default();
    let locks = card_locks(&conn, &ring, "a", "INBOX", &[1, 2, 3, 4, 9]).unwrap();
    assert_eq!(
        locks.into_iter().collect::<Vec<_>>(),
        vec![(1, keyring::Trust::Unconfirmed)]
    );
    assert!(
        card_locks(&conn, &ring, "a", "INBOX", &[])
            .unwrap()
            .is_empty()
    );
}

#[test]
fn encrypted_bodies_cached_without_openpgp_are_refetched_once() {
    let conn = crate::store::open_at(":memory:").unwrap();
    let insert = |uid: u32, subject: &str, body: Option<&str>, json: &str, files: Option<&str>| {
        conn.execute(
            "INSERT INTO messages (account, folder, msg_id, uid, subject, from_name, from_addr, date, body, json, files)
             VALUES ('a', 'INBOX', ?1, ?1, ?2, 'n', 'x@y.z', 0, ?3, ?4, ?5)",
            params![uid, subject, body, json, files],
        )
        .unwrap();
    };
    let asc = r#"[{"filename":"encrypted.asc","mime":"application/octet-stream"}]"#;
    // Cached by a build without OpenPGP: never decrypted.
    insert(
        1,
        "...",
        Some(""),
        r#"{"attachments":[{"filename":"encrypted.asc"}]}"#,
        Some(asc),
    );
    insert(
        2,
        "Hi",
        Some("-----BEGIN PGP MESSAGE-----\nhQEM\n-----END PGP MESSAGE-----"),
        "{}",
        None,
    );
    insert(3, "[...]", Some(""), "{}", None);
    // Left alone: decrypted, plain, not cached, a plain mail with a .pgp file.
    insert(
        4,
        "Secret",
        Some("text"),
        r#"{"crypto":{"encrypted":true}}"#,
        None,
    );
    insert(5, "Lunch", Some("hello"), "{}", None);
    insert(6, "...", None, "{}", None);
    insert(
        7,
        "Report",
        Some("see file"),
        "{}",
        Some(r#"[{"filename":"report.pgp","mime":"application/octet-stream"}]"#),
    );

    assert_eq!(
        refetch_encrypted_cached_without_openpgp_once(&conn).unwrap(),
        3
    );
    let bodies: Vec<(u32, Option<String>, Option<String>)> = conn
        .prepare("SELECT uid, body, json_extract(json, '$.attachments') FROM messages ORDER BY uid")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    for (uid, body, attachments) in bodies {
        let cleared = matches!(uid, 1..=3);
        assert_eq!(body.is_none(), cleared || uid == 6, "uid {uid} body");
        if cleared {
            assert!(
                attachments.is_none(),
                "uid {uid} keeps no stale attachments"
            );
        }
    }
    assert_eq!(
        refetch_encrypted_cached_without_openpgp_once(&conn).unwrap(),
        0,
        "only once"
    );
}

#[test]
fn placeholder_rows_without_a_body_are_listed_for_prefetch() {
    let conn = crate::store::open_at(":memory:").unwrap();
    let insert = |folder: &str, uid: u32, subject: &str, body: Option<&str>| {
        conn.execute(
            "INSERT INTO messages (account, folder, msg_id, uid, subject, from_name, from_addr, date, body, json)
             VALUES ('a', ?1, ?2, ?2, ?3, 'n', 'x@y.z', 0, ?4, '{}')",
            params![folder, uid, subject, body],
        )
        .unwrap();
    };
    insert("INBOX", 3, "...", None);
    insert("INBOX", 1, " [...] ", None);
    insert("INBOX", 2, "...", Some("decrypted already"));
    insert("INBOX", 4, "Lunch", None);
    insert("Archive", 5, "...", None);
    insert("INBOX", 6, "Re: [...]", None);
    insert("INBOX", 7, "Waiting...", None);
    assert_eq!(
        undecrypted_placeholder_uids(&conn, "a", "INBOX").unwrap(),
        vec![1, 3, 6]
    );
    assert!(
        undecrypted_placeholder_uids(&conn, "b", "INBOX")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn offer_dismissal_per_sender_and_for_all() {
    let conn = db();
    assert!(!offer_dismissed(&conn, "acc", "carol@example.com"));
    dismiss_offer(&conn, "acc", "Carol@Example.com").unwrap();
    assert!(offer_dismissed(&conn, "acc", "carol@example.com"));
    assert!(!offer_dismissed(&conn, "acc", "bob@example.net"));
    dismiss_offer(&conn, "acc", "*").unwrap();
    assert!(offer_dismissed(&conn, "acc", "bob@example.net"));
}

#[test]
fn backup_without_secrets_carries_no_secret_bytes_and_restore_reports_it() {
    let conn = db();
    create_key(&conn, "acc", &["me@example.org".into()]).unwrap();
    let without = backup_collect(&conn, false).unwrap();
    assert!(without[0].secret.is_empty());
    assert!(
        serde_json::to_string(&without)
            .unwrap()
            .contains("BEGIN PGP PUBLIC KEY BLOCK")
    );
    assert!(!serde_json::to_string(&without).unwrap().contains("PRIVATE"));
    let fresh = db();
    assert_eq!(
        backup_apply(&fresh, &without).unwrap(),
        vec!["acc".to_string()]
    );
    let with = backup_collect(&conn, true).unwrap();
    let fresh = db();
    assert!(backup_apply(&fresh, &with).unwrap().is_empty());
    let ring = load(&fresh).unwrap();
    assert!(ring.is_enabled("acc"));
    assert_eq!(
        ring.default_key("acc").unwrap().fingerprint,
        load(&conn).unwrap().default_key("acc").unwrap().fingerprint
    );
}

// Mobile and desktop both expose crypto state and restore protected backups.
#[test]
fn backup_restore_keeps_secret_keys_on_all_platforms() {
    let conn = db();
    create_key(&conn, "acc", &["me@example.org".into()]).unwrap();
    let backup = backup_collect(&conn, true).unwrap();
    assert!(
        !backup[0].secret.is_empty(),
        "the backup carries a secret key"
    );

    let phone = db();
    let missing = super::backup_apply(&phone, &backup).unwrap();
    assert!(missing.is_empty());
    assert!(
        !load(&phone).unwrap().own.is_empty(),
        "secret key restored on mobile"
    );
    assert!(
        load(&phone).unwrap().is_enabled("acc"),
        "encryption enabled on mobile"
    );

    // The same public restore entry point is used on desktop.
    let desktop = db();
    assert!(super::backup_apply(&desktop, &backup).unwrap().is_empty());
    assert!(
        !load(&desktop).unwrap().own.is_empty(),
        "desktop restores the key"
    );
}

// OpenKeychain's key layout: a certify-only primary with signing and
// encryption subkeys. It imports, and signs with the subkey.
#[test]
fn a_key_with_a_certify_only_primary_imports() {
    let conn = db();
    let key = crate::openpgp::crypt::tests::certify_only_primary("me@example.org");
    let data = key.to_armored_string(Default::default()).unwrap();
    let ImportOwn::Imported {
        address_matches, ..
    } = import_own(
        &conn,
        "acc",
        &["me@example.org".into()],
        data.as_bytes(),
        None,
        None,
        None,
    )
    .unwrap()
    else {
        panic!("not imported");
    };
    assert!(address_matches);
    let ring = load(&conn).unwrap();
    assert!(
        ring.default_key("acc").is_some(),
        "passes the load self-test"
    );
}

// OpenKeychain's "Backup keys" file: the keys, encrypted with the backup
// code. The code is asked for, then the key inside is imported.
#[test]
fn an_openkeychain_backup_imports_with_its_code() {
    let conn = db();
    let key = crate::openpgp::crypt::tests::certify_only_primary("me@example.org");
    let inner = format!(
        "{}{}",
        keys::armored_public(&key.to_public_key()).unwrap(),
        key.to_armored_string(Default::default()).unwrap()
    );
    let code = "1234-5678-9012-3456-7890-1234-5678-9012-3456";
    let file = crate::openpgp::crypt::tests::password_encrypted(inner.as_bytes(), code);
    let import = |code: Option<&str>| {
        import_own(&conn, "acc", &[], file.as_bytes(), None, code, None).unwrap()
    };
    assert!(matches!(
        import(None),
        ImportOwn::NeedsBackupCode { wrong: false }
    ));
    assert!(matches!(
        import(Some("1111-2222")),
        ImportOwn::NeedsBackupCode { wrong: true }
    ));
    let ImportOwn::Imported { key: info, .. } = import(Some(&code.replace('-', " "))) else {
        panic!("not imported");
    };
    assert_eq!(info.fingerprint, fingerprint_hex(&key.primary_key));
}

#[test]
fn signing_unencrypted_mail_is_a_per_account_setting() {
    let conn = db();
    create_key(&conn, "acc", &["me@example.org".into()]).unwrap();
    assert!(!account_state(&conn, "acc").unwrap().sign_plain);
    assert!(!load(&conn).unwrap().signs_plain("acc"));

    set_sign_plain(&conn, "acc", true).unwrap();
    assert!(account_state(&conn, "acc").unwrap().sign_plain);
    assert!(load(&conn).unwrap().signs_plain("acc"));
    assert!(backup_collect(&conn, true).unwrap()[0].sign_plain);

    // Only with the key in use: encryption off means nothing is signed.
    disable(&conn, "acc").unwrap();
    assert!(!load(&conn).unwrap().signs_plain("acc"));

    enable_existing(&conn, "acc").unwrap();
    set_sign_plain(&conn, "acc", false).unwrap();
    assert!(!load(&conn).unwrap().signs_plain("acc"));
}
