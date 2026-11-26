use super::*;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/openpgp/testdata")
            .join(name),
    )
    .expect("fixture")
}

#[test]
fn generated_key_is_v4_ed25519_with_an_encryption_subkey_and_no_seipd_v2() {
    let key = generate(&["me@example.org".into(), "alias@example.org".into()]).unwrap();
    let info = secret_info(&key, now());
    assert_eq!(info.emails, vec!["alias@example.org", "me@example.org"]);
    assert!(info.can_encrypt);
    assert!(!info.supports_seipd_v2, "GnuPG 2.4 can't read SEIPDv2");
    assert_eq!(info.expires, None);
    assert_eq!(info.unusable, None);
    assert_eq!(key.primary_key.version(), pgp::types::KeyVersion::V4);
    self_test(&key).unwrap();
}

#[test]
fn gnupg_secret_key_without_passphrase_unlocks_and_self_tests() {
    let parsed = parse_keys(&fixture("alice-secret.asc")).unwrap();
    assert_eq!(parsed.secret.len(), 1);
    let Unlock::Ok(key) = unlock(parsed.secret.into_iter().next().unwrap(), None).unwrap() else {
        panic!("expected an unlocked key");
    };
    assert_eq!(secret_info(&key, now()).emails, vec!["alice@example.org"]);
}

#[test]
fn protected_rsa_key_needs_the_right_passphrase() {
    let key = parse_keys(&fixture("bob-secret-protected.asc"))
        .unwrap()
        .secret
        .remove(0);
    assert!(matches!(
        unlock(key.clone(), None).unwrap(),
        Unlock::NeedsPassphrase
    ));
    assert!(matches!(
        unlock(key.clone(), Some("wrong")).unwrap(),
        Unlock::NeedsPassphrase
    ));
    assert!(matches!(
        unlock(key, Some("correct horse battery staple")).unwrap(),
        Unlock::Ok(_)
    ));
}

#[test]
fn subkeys_only_export_is_reported_as_on_card() {
    let key = parse_keys(&fixture("alice-secret-subkeys-only.asc"))
        .unwrap()
        .secret
        .remove(0);
    assert!(matches!(unlock(key, None).unwrap(), Unlock::OnCard));
}

#[test]
fn a_file_with_two_secret_keys_yields_both() {
    assert_eq!(
        parse_keys(&fixture("two-secret-keys.asc"))
            .unwrap()
            .secret
            .len(),
        2
    );
}

#[test]
fn contacts_keyring_flags_expired_and_revoked_keys() {
    let parsed = parse_keys(&fixture("contacts-keyring.asc")).unwrap();
    assert_eq!(parsed.public.len(), 5);
    let by_email = |e: &str| {
        parsed
            .public
            .iter()
            .find(|k| {
                info(k, false, now()).emails.iter().any(|x| x == e)
                    || k.details
                        .users
                        .iter()
                        .any(|u| u.id.as_str().is_some_and(|s| s.contains(e)))
            })
            .map(|k| check(k, now()))
            .expect(e)
    };
    assert_eq!(by_email("carol@example.com"), None);
    assert_eq!(by_email("bob@example.net"), None);
    assert_eq!(by_email("erin@example.com"), Some(Unusable::Expired));
    assert_eq!(by_email("rex@example.com"), Some(Unusable::Revoked));
}

#[test]
fn binary_keyring_parses_like_the_armored_one() {
    assert_eq!(
        parse_keys(&fixture("contacts-keyring.gpg"))
            .unwrap()
            .public
            .len(),
        3
    );
}

#[test]
fn concatenated_armor_blocks_are_all_read() {
    let mut both = fixture("carol-public.asc");
    both.extend_from_slice(&fixture("mallory-public.asc"));
    assert_eq!(parse_keys(&both).unwrap().public.len(), 2);
}

#[test]
fn garbage_is_reported_not_panicked_on() {
    let parsed = parse_keys(
        b"-----BEGIN PGP PUBLIC KEY BLOCK-----\n\nAAAA\n-----END PGP PUBLIC KEY BLOCK-----\n",
    )
    .unwrap();
    assert!(parsed.public.is_empty());
}

#[test]
fn protected_export_round_trips_with_the_new_passphrase() {
    let key = generate(&["me@example.org".into()]).unwrap();
    let armored = export_protected(&key, "export pass").unwrap();
    let reparsed = parse_keys(armored.as_bytes()).unwrap().secret.remove(0);
    assert!(matches!(
        unlock(reparsed.clone(), None).unwrap(),
        Unlock::NeedsPassphrase
    ));
    assert!(matches!(
        unlock(reparsed, Some("export pass")).unwrap(),
        Unlock::Ok(_)
    ));
    assert!(export_protected(&key, "").is_err());
}

#[test]
fn stored_secret_round_trips() {
    let key = generate(&["me@example.org".into()]).unwrap();
    let bytes = binary_secret(&key).unwrap();
    let back = parse_secret(&bytes).unwrap();
    assert_eq!(
        fingerprint_hex(&back.primary_key),
        fingerprint_hex(&key.primary_key)
    );
}

#[test]
fn email_of_user_id_handles_names_and_bare_addresses() {
    assert_eq!(
        email_of_user_id("Carol Example <Carol@Example.com>").as_deref(),
        Some("carol@example.com")
    );
    assert_eq!(email_of_user_id("<a@b.org>").as_deref(), Some("a@b.org"));
    assert_eq!(email_of_user_id("a@b.org").as_deref(), Some("a@b.org"));
    assert_eq!(email_of_user_id("No Address"), None);
}

#[test]
fn validity_at_a_time_respects_expiry_and_revocation() {
    let erin = parse_keys(&fixture("expired-public.asc"))
        .unwrap()
        .public
        .remove(0);
    // Created 2024-01-01, one-year expiry.
    assert_eq!(
        primary_valid_at(&erin, 1_717_200_000),
        Ok(()),
        "June 2024: valid"
    );
    assert_eq!(primary_valid_at(&erin, now()), Err(SigningProblem::Expired));
    let rex = parse_keys(&fixture("revoked-public.asc"))
        .unwrap()
        .public
        .remove(0);
    // Revoked with no reason: every signature is void, even old ones.
    assert_eq!(
        primary_valid_at(&rex, rex.primary_key.created_at().as_secs()),
        Err(SigningProblem::Revoked)
    );
}

#[test]
fn merging_a_revoked_copy_revokes_the_stored_key() {
    let carol = parse_keys(&fixture("carol-public.asc"))
        .unwrap()
        .public
        .remove(0);
    let revoked = parse_keys(&fixture("carol-revoked-public.asc"))
        .unwrap()
        .public
        .remove(0);
    assert_eq!(check(&carol, now()), None);
    let merged = merge(&carol, &revoked).expect("the revocation is new");
    assert_eq!(check(&merged, now()), Some(Unusable::Revoked));
    assert!(
        merge(&merged, &revoked).is_none(),
        "nothing new the second time"
    );
    assert!(
        merge(&merged, &carol).is_none(),
        "an older copy adds nothing and can't undo it"
    );
    let mallory = parse_keys(&fixture("mallory-public.asc"))
        .unwrap()
        .public
        .remove(0);
    assert!(
        merge(&carol, &mallory).is_none(),
        "a different key is never merged"
    );
}

/// A key revocation of `target` signed by `revoker` (what GnuPG's
/// `--desig-revoke` makes; it needs a terminal, so tests build it here).
pub(crate) fn revoke_by(revoker: &SignedSecretKey, target: &SignedPublicKey) -> SignedPublicKey {
    use pgp::packet::{SignatureConfig, SignatureType, Subpacket, SubpacketData};
    let mut config = SignatureConfig::v4(
        SignatureType::KeyRevocation,
        revoker.primary_key.algorithm(),
        HashAlgorithm::Sha256,
    );
    config.hashed_subpackets = vec![
        Subpacket::critical(SubpacketData::SignatureCreationTime(
            pgp::types::Timestamp::now(),
        ))
        .unwrap(),
        Subpacket::regular(SubpacketData::IssuerFingerprint(
            revoker.primary_key.fingerprint(),
        ))
        .unwrap(),
    ];
    let sig = config
        .sign_key(
            &revoker.primary_key,
            &Password::empty(),
            &target.primary_key,
        )
        .unwrap();
    let mut out = target.clone();
    out.details.revocation_signatures.push(sig);
    out
}

/// A signature of `typ` by `key`'s primary, made at `at` with `extra`
/// subpackets: over its first user ID (certifications), the key itself
/// (direct-key signatures and revocations), or its first subkey (subkey
/// revocations).
pub(crate) fn self_sig(
    key: &SignedSecretKey,
    typ: pgp::packet::SignatureType,
    at: u32,
    extra: Vec<pgp::packet::SubpacketData>,
) -> Signature {
    use pgp::packet::{SignatureConfig, SignatureType, Subpacket, SubpacketData};
    let mut config = SignatureConfig::v4(typ, key.primary_key.algorithm(), HashAlgorithm::Sha256);
    config.hashed_subpackets = vec![
        Subpacket::critical(SubpacketData::SignatureCreationTime(
            pgp::types::Timestamp::from_secs(at),
        ))
        .unwrap(),
        Subpacket::regular(SubpacketData::IssuerFingerprint(
            key.primary_key.fingerprint(),
        ))
        .unwrap(),
    ];
    config
        .hashed_subpackets
        .extend(extra.into_iter().map(|d| Subpacket::regular(d).unwrap()));
    let public = key.to_public_key();
    let pw = Password::empty();
    match typ {
        SignatureType::CertPositive | SignatureType::CertRevocation => config
            .sign_certification(
                &key.primary_key,
                &public.primary_key,
                &pw,
                pgp::types::Tag::UserId,
                &public.details.users[0].id,
            )
            .unwrap(),
        SignatureType::SubkeyRevocation => config
            .sign_subkey_binding(
                &key.primary_key,
                &public.primary_key,
                &pw,
                &public.public_subkeys[0].key,
            )
            .unwrap(),
        _ => config
            .sign_key(&key.primary_key, &pw, &public.primary_key)
            .unwrap(),
    }
}

// A user ID whose newest self-signature revokes it isn't one of the key's
// addresses any more (rPGP verifies the revocation as a certification).
#[test]
fn a_revoked_user_id_is_not_an_address_of_the_key() {
    use pgp::packet::SignatureType;
    let key = generate(&["gone@example.org".into()]).unwrap();
    let mut public = key.to_public_key();
    assert_eq!(info(&public, false, now()).emails, vec!["gone@example.org"]);
    public.details.users[0].signatures.push(self_sig(
        &key,
        SignatureType::CertRevocation,
        now() + 10,
        vec![],
    ));
    assert!(info(&public, false, now() + 20).emails.is_empty());
}

// A subkey revocation counts even when a binding is newer (or made the
// same second): nothing is encrypted to that subkey again.
#[test]
fn a_revoked_encryption_subkey_stays_revoked_under_a_newer_binding() {
    use pgp::packet::SignatureType;
    let key = generate(&["sub@example.org".into()]).unwrap();
    let mut public = key.to_public_key();
    assert!(encryption_subkey(&public, now()).is_some());
    let created = public.public_subkeys[0].key.created_at().as_secs();
    public.public_subkeys[0].signatures.push(self_sig(
        &key,
        SignatureType::SubkeyRevocation,
        created.saturating_sub(10),
        vec![],
    ));
    assert!(encryption_subkey(&public, now()).is_none());
}

// A later self-signature that only adds something (a direct-key signature
// naming a revoker, no key flags) doesn't clear the key's expiry.
#[test]
fn a_self_signature_without_key_flags_keeps_the_expiry() {
    use pgp::packet::{KeyFlags, SignatureType, SubpacketData};
    let key = generate(&["exp@example.org".into()]).unwrap();
    let created = key.primary_key.created_at().as_secs();
    let mut flags = KeyFlags::default();
    flags.set_certify(true);
    flags.set_sign(true);
    let mut public = key.to_public_key();
    public.details.direct_signatures.push(self_sig(
        &key,
        SignatureType::Key,
        now() + 10,
        vec![
            SubpacketData::KeyFlags(flags),
            SubpacketData::KeyExpirationTime(pgp::types::Duration::from_secs(1000)),
        ],
    ));
    public
        .details
        .direct_signatures
        .push(self_sig(&key, SignatureType::Key, now() + 20, vec![]));
    assert_eq!(primary_expiry(&public), Some(i64::from(created) + 1000));
}

/// `key` with a new self-certification of its first user ID made at `at`:
/// what a renewal (`gpg --quick-set-expire`) adds.
pub(crate) fn self_certified_at(key: &SignedSecretKey, at: u32) -> SignedSecretKey {
    self_certified_with_hash(key, at, HashAlgorithm::Sha256)
}

pub(crate) fn self_certified_with_hash(
    key: &SignedSecretKey,
    at: u32,
    hash: HashAlgorithm,
) -> SignedSecretKey {
    use pgp::packet::{SignatureConfig, SignatureType, Subpacket, SubpacketData};
    let mut config = SignatureConfig::v4(
        SignatureType::CertPositive,
        key.primary_key.algorithm(),
        hash,
    );
    config.hashed_subpackets = vec![
        Subpacket::critical(SubpacketData::SignatureCreationTime(
            pgp::types::Timestamp::from_secs(at),
        ))
        .unwrap(),
        Subpacket::regular(SubpacketData::IssuerFingerprint(
            key.primary_key.fingerprint(),
        ))
        .unwrap(),
    ];
    let public = key.to_public_key();
    let user = &public.details.users[0];
    let sig = config
        .sign_certification(
            &key.primary_key,
            &public.primary_key,
            &Password::empty(),
            pgp::types::Tag::UserId,
            &user.id,
        )
        .unwrap();
    let mut out = key.clone();
    out.details.users[0].signatures.push(sig);
    out
}

// Importing an own key again: a renewal (newer self-signature) replaces
// the stored copy; an older export, or one missing a stored subkey, doesn't.
#[test]
fn a_renewed_own_key_supersedes_the_stored_one_and_an_older_one_does_not() {
    let key = generate(&["me@example.org".into()]).unwrap();
    let renewed = self_certified_at(&key, now() + 100);
    assert!(secret_supersedes(&renewed, &key));
    assert!(secret_supersedes(&key, &key), "the same export again");
    assert!(!secret_supersedes(&key, &renewed), "never rolled back");
    let mut fewer = renewed.clone();
    fewer.secret_subkeys.clear();
    assert!(!secret_supersedes(&fewer, &key), "a stored subkey is kept");
}

/// A user-ID certification of `target`'s first user ID made by `signer` (so
/// it does NOT verify against `target`'s own key), carrying an attacker-set
/// key expiration and the SEIPDv2 feature flag, created now so it is the
/// newest signature. Models a third party attaching a junk self-signature
/// to someone's genuine key.
fn forge_user_sig_by(
    signer: &SignedSecretKey,
    target: &SignedPublicKey,
    exp_secs: u32,
    seipd_v2: bool,
) -> SignedPublicKey {
    use pgp::packet::{SignatureConfig, SignatureType, Subpacket, SubpacketData};
    use pgp::types::{Duration, Timestamp};
    let mut features = pgp::packet::Features::new();
    features.set_seipd_v2(seipd_v2);
    let mut config = SignatureConfig::v4(
        SignatureType::CertPositive,
        signer.primary_key.algorithm(),
        HashAlgorithm::Sha256,
    );
    config.hashed_subpackets = vec![
        Subpacket::critical(SubpacketData::SignatureCreationTime(Timestamp::now())).unwrap(),
        Subpacket::regular(SubpacketData::IssuerFingerprint(
            signer.primary_key.fingerprint(),
        ))
        .unwrap(),
        Subpacket::regular(SubpacketData::KeyExpirationTime(Duration::from_secs(
            exp_secs,
        )))
        .unwrap(),
        Subpacket::regular(SubpacketData::Features(features)).unwrap(),
    ];
    let user = &target.details.users[0];
    let sig = config
        .sign_certification(
            &signer.primary_key,
            &target.primary_key,
            &Password::empty(),
            pgp::types::Tag::UserId,
            &user.id,
        )
        .unwrap();
    let mut out = target.clone();
    out.details.users[0].signatures.push(sig);
    out
}

// Expiry and SEIPDv2 must be read only from verified self-signatures. A
// third party who serves someone's genuine key with a junk signature
// attached must not be able to change when Meron thinks the key expires, or
// trick Meron into sending SEIPDv2 (AEAD) to a key that can't read it.
#[test]
fn unverified_signatures_do_not_set_expiry_or_seipd_v2() {
    let mallory = secret("mallory-secret.asc");

    // SEIPDv2: alice's real self-sig advertises none; a forged one claims it.
    let alice = parse_public(&fixture("alice-public.asc")).unwrap();
    assert!(
        !supports_seipd_v2(&alice),
        "baseline: alice does not advertise SEIPDv2"
    );
    let alice_spiked = forge_user_sig_by(&mallory, &alice, 2_000_000_000, true);
    assert!(
        !supports_seipd_v2(&alice_spiked),
        "a forged signature must not enable SEIPDv2"
    );

    // Expiry: expired-public has a real expiry; a forged newer sig claims another.
    let expired = parse_public(&fixture("expired-public.asc")).unwrap();
    let real_expiry = primary_expiry(&expired);
    assert!(
        real_expiry.is_some(),
        "baseline: expired-public has a real expiry"
    );
    let spiked = forge_user_sig_by(&mallory, &expired, 2_000_000_000, false);
    assert_eq!(
        primary_expiry(&spiked),
        real_expiry,
        "a forged signature must not change the expiry"
    );
}

/// A user-ID self-certification of `secret`'s own primary, signed by it,
/// carrying key flags with or without the sign capability, created now so
/// it is the newest self-signature. Lets a test make a key whose primary
/// can or cannot sign.
fn self_cert_with_sign_flag(secret: &SignedSecretKey, sign: bool) -> Signature {
    use pgp::packet::{KeyFlags, SignatureConfig, SignatureType, Subpacket, SubpacketData};
    use pgp::types::Timestamp;
    let mut flags = KeyFlags::default();
    flags.set_certify(true);
    flags.set_encrypt_comms(true);
    flags.set_encrypt_storage(true);
    flags.set_sign(sign);
    let pubkey = secret.to_public_key();
    let user = &pubkey.details.users[0];
    let mut config = SignatureConfig::v4(
        SignatureType::CertPositive,
        secret.primary_key.algorithm(),
        HashAlgorithm::Sha256,
    );
    config.hashed_subpackets = vec![
        Subpacket::critical(SubpacketData::SignatureCreationTime(Timestamp::now())).unwrap(),
        Subpacket::regular(SubpacketData::IssuerFingerprint(
            secret.primary_key.fingerprint(),
        ))
        .unwrap(),
        Subpacket::regular(SubpacketData::KeyFlags(flags)).unwrap(),
    ];
    config
        .sign_certification(
            &secret.primary_key,
            &pubkey.primary_key,
            &Password::empty(),
            pgp::types::Tag::UserId,
            &user.id,
        )
        .unwrap()
}

/// A self-signature by `secret`'s primary without a key-flags subpacket,
/// created now: a direct-key signature, or (`on_user`) a user-ID
/// self-certification.
fn self_sig_without_key_flags(secret: &SignedSecretKey, on_user: bool) -> Signature {
    use pgp::packet::{SignatureConfig, SignatureType, Subpacket, SubpacketData};
    use pgp::types::Timestamp;
    let pubkey = secret.to_public_key();
    let typ = if on_user {
        SignatureType::CertPositive
    } else {
        SignatureType::Key
    };
    let mut config =
        SignatureConfig::v4(typ, secret.primary_key.algorithm(), HashAlgorithm::Sha256);
    config.hashed_subpackets = vec![
        Subpacket::critical(SubpacketData::SignatureCreationTime(Timestamp::now())).unwrap(),
        Subpacket::regular(SubpacketData::IssuerFingerprint(
            secret.primary_key.fingerprint(),
        ))
        .unwrap(),
    ];
    if on_user {
        let id = &pubkey.details.users[0].id;
        config
            .sign_certification(
                &secret.primary_key,
                &pubkey.primary_key,
                &Password::empty(),
                pgp::types::Tag::UserId,
                id,
            )
            .unwrap()
    } else {
        config
            .sign_key(&secret.primary_key, &Password::empty(), &pubkey.primary_key)
            .unwrap()
    }
}

/// A key revocation of `target` signed by `signer` but naming `named` as its
/// issuer: a forgery that claims to come from a designated revoker. `n`
/// shifts the creation time so each forgery is a distinct signature.
fn revocation_claiming(
    signer: &SignedSecretKey,
    named: &SignedSecretKey,
    target: &SignedPublicKey,
    n: u32,
) -> Signature {
    use pgp::packet::{SignatureConfig, SignatureType, Subpacket, SubpacketData};
    use pgp::types::Timestamp;
    let mut config = SignatureConfig::v4(
        SignatureType::KeyRevocation,
        signer.primary_key.algorithm(),
        HashAlgorithm::Sha256,
    );
    config.hashed_subpackets = vec![
        Subpacket::critical(SubpacketData::SignatureCreationTime(Timestamp::from_secs(
            now() - n,
        )))
        .unwrap(),
        Subpacket::regular(SubpacketData::IssuerFingerprint(
            named.primary_key.fingerprint(),
        ))
        .unwrap(),
    ];
    config
        .sign_key(&signer.primary_key, &Password::empty(), &target.primary_key)
        .unwrap()
}

// A self-signature without key flags doesn't decide whether the primary
// can sign; with no flags anywhere, the primary's algorithm does.
#[test]
fn primary_can_sign_ignores_self_signatures_without_key_flags() {
    let secret = secret("alice-secret.asc");
    let mut newer_flagless = secret.to_public_key();
    newer_flagless
        .details
        .direct_signatures
        .push(self_sig_without_key_flags(&secret, false));
    assert!(
        primary_can_sign(&newer_flagless),
        "a newer direct-key signature without flags"
    );

    let mut no_flags = secret.to_public_key();
    no_flags.details.direct_signatures.clear();
    no_flags.details.users[0].signatures = vec![self_sig_without_key_flags(&secret, true)];
    assert!(
        primary_can_sign(&no_flags),
        "no key flags anywhere: EdDSA can sign"
    );
}

// A revocation by a designated revoker whose key Meron doesn't have yet is
// kept, so it takes effect once that key is known: on first store and when
// it arrives as an update. A revocation by anyone the key doesn't name as a
// revoker is still dropped.
#[test]
fn a_designated_revocation_is_kept_until_the_revokers_key_is_known() {
    let tess = parse_public(&fixture("tess-public.asc")).unwrap();
    let dan = secret("dan-secret.asc");
    let dan_pub = dan.to_public_key();
    let dan_fpr = fingerprint_hex(&dan.primary_key);
    let resolve_dan = |f: &str| (f == dan_fpr).then(|| dan_pub.clone());
    let none = |_: &str| None;
    let revoked = revoke_by(&dan, &tess);

    let stored = minimize_with(&revoked, &none);
    assert_eq!(
        stored.details.revocation_signatures.len(),
        1,
        "kept while Dan's key is unknown"
    );
    assert_eq!(
        check_with(&stored, now(), &none),
        None,
        "doesn't count without Dan's key"
    );
    assert_eq!(
        check_with(&stored, now(), &resolve_dan),
        Some(Unusable::Revoked),
        "counts once it is known"
    );

    let updated = merge_with(&tess, &revoked, &none).expect("the pending revocation is new");
    assert_eq!(
        check_with(&updated, now(), &resolve_dan),
        Some(Unusable::Revoked)
    );

    let mallory = secret("mallory-secret.asc");
    let not_named = revoke_by(&mallory, &tess);
    assert!(
        minimize_with(&not_named, &none)
            .details
            .revocation_signatures
            .is_empty()
    );
    assert!(merge_with(&tess, &not_named, &none).is_none());
}

// A forgery naming the designated revoker can be stored while that key is
// unknown, but never counts once it is known; such pending revocations are
// capped, so a genuine revocation still gets in.
#[test]
fn forged_pending_revocations_never_count_and_are_capped() {
    let tess = parse_public(&fixture("tess-public.asc")).unwrap();
    let dan = secret("dan-secret.asc");
    let mallory = secret("mallory-secret.asc");
    let dan_pub = dan.to_public_key();
    let dan_fpr = fingerprint_hex(&dan.primary_key);
    let resolve_dan = |f: &str| (f == dan_fpr).then(|| dan_pub.clone());
    let none = |_: &str| None;
    let mut spammed = tess.clone();
    for n in 0..10 {
        spammed
            .details
            .revocation_signatures
            .push(revocation_claiming(&mallory, &dan, &tess, n));
    }
    let stored = minimize_with(&spammed, &none);
    assert_eq!(
        stored.details.revocation_signatures.len(),
        MAX_PENDING_REVOCATIONS
    );
    assert_eq!(
        check_with(&stored, now(), &resolve_dan),
        None,
        "forgeries never count"
    );
    // With Dan's key known the forgeries fail verification and aren't kept at all.
    assert!(
        minimize_with(&spammed, &resolve_dan)
            .details
            .revocation_signatures
            .is_empty()
    );

    let genuine = revoke_by(&dan, &tess);
    let merged =
        merge_with(&stored, &genuine, &resolve_dan).expect("the genuine revocation is added");
    assert_eq!(
        check_with(&merged, now(), &resolve_dan),
        Some(Unusable::Revoked)
    );
}

// Meron signs with the primary key, so a key whose primary is not
// flagged to sign (an offline-primary + signing-subkey setup) must be
// detected — signing with it would make signatures strict verifiers reject.
#[test]
fn primary_can_sign_reads_the_newest_self_signature() {
    let secret = secret("alice-secret.asc");
    // Alice's generated key signs.
    assert!(primary_can_sign(&secret.to_public_key()));

    let mut cannot = secret.to_public_key();
    cannot.details.users[0]
        .signatures
        .push(self_cert_with_sign_flag(&secret, false));
    assert!(
        !primary_can_sign(&cannot),
        "a newer self-sig without the sign flag is respected"
    );

    let mut still_can = secret.to_public_key();
    still_can.details.users[0]
        .signatures
        .push(self_cert_with_sign_flag(&secret, true));
    assert!(primary_can_sign(&still_can));
}

// A key is minimized before it is stored, so a third
// party's junk signature attached to a genuine key never reaches the store.
#[test]
fn minimize_drops_signatures_that_do_not_verify_and_keeps_the_key_usable() {
    let mallory = secret("mallory-secret.asc");
    let alice = parse_public(&fixture("alice-public.asc")).unwrap();
    let spiked = forge_user_sig_by(&mallory, &alice, 2_000_000_000, true);
    let before: usize = spiked
        .details
        .users
        .iter()
        .map(|u| u.signatures.len())
        .sum();

    let clean = minimize(&spiked);
    let after: usize = clean.details.users.iter().map(|u| u.signatures.len()).sum();
    assert!(
        after < before,
        "the forged signature is stripped ({before} -> {after})"
    );

    // Alice's genuine key still works: her user ID binds and her encryption
    // subkey survives.
    assert!(
        valid_user_ids(&clean).iter().any(|u| u.contains("alice")),
        "the real user ID survives"
    );
    assert!(
        encryption_subkey(&clean, now()).is_some(),
        "the encryption subkey survives"
    );
    assert!(
        !supports_seipd_v2(&clean),
        "the forged SEIPDv2 flag is gone"
    );
}

fn secret(name: &str) -> SignedSecretKey {
    let k = parse_keys(&fixture(name)).unwrap().secret.remove(0);
    match unlock(k, None).unwrap() {
        Unlock::Ok(k) => k,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_designated_revoker_can_revoke_and_nobody_else_can() {
    let tess = parse_public(&fixture("tess-public.asc")).unwrap();
    let dan = secret("dan-secret.asc");
    let dan_pub = dan.to_public_key();
    let dan_fpr = fingerprint_hex(&dan.primary_key);
    assert_eq!(designated_revokers(&tess), vec![dan_fpr.clone()]);
    let resolve_dan = |f: &str| (f == dan_fpr).then(|| dan_pub.clone());
    let none = |_: &str| None;

    let revoked = revoke_by(&dan, &tess);
    assert_eq!(
        check_with(&revoked, now(), &resolve_dan),
        Some(Unusable::Revoked)
    );
    assert_eq!(
        primary_valid_at_with(&revoked, now(), &resolve_dan),
        Err(SigningProblem::Revoked)
    );
    // Without Dan's key the revocation can't be checked, so it doesn't count.
    assert_eq!(check_with(&revoked, now(), &none), None);

    // Mallory isn't named as a revoker: her "revocation" never counts.
    let mallory = secret("mallory-secret.asc");
    let mallory_pub = mallory.to_public_key();
    let forged = revoke_by(&mallory, &tess);
    let resolve_both = |f: &str| {
        if f == dan_fpr {
            Some(dan_pub.clone())
        } else if f == fingerprint_hex(&mallory_pub.primary_key) {
            Some(mallory_pub.clone())
        } else {
            None
        }
    };
    assert_eq!(check_with(&forged, now(), &resolve_both), None);

    // Merging takes Dan's revocation and drops Mallory's.
    let merged = merge_with(&tess, &revoked, &resolve_dan).expect("Dan's revocation is new");
    assert_eq!(
        check_with(&merged, now(), &resolve_dan),
        Some(Unusable::Revoked)
    );
    assert!(merge_with(&tess, &forged, &resolve_both).is_none());
}

#[test]
fn sha1_and_md5_data_signatures_are_rejected() {
    assert!(!data_hash_ok(Some(HashAlgorithm::Sha1)));
    assert!(!data_hash_ok(Some(HashAlgorithm::Md5)));
    assert!(data_hash_ok(Some(HashAlgorithm::Sha256)));
}

#[test]
fn the_signer_is_the_primary_or_else_a_valid_signing_subkey() {
    let own = generate(&["me@example.org".into()]).unwrap();
    assert!(matches!(
        signer(&own, Some(now())),
        Some(Signer::Primary(_))
    ));

    let split = crate::openpgp::crypt::tests::certify_only_primary("me@example.org");
    let Some(Signer::Sub(sub)) = signer(&split, Some(now())) else {
        panic!("expected the signing subkey");
    };
    assert_eq!(
        sub.fingerprint(),
        split.secret_subkeys[0].key.fingerprint(),
        "the signing subkey, not the encryption subkey"
    );
    self_test(&split).unwrap();

    // Nothing that signs at all: refused by the self-test every import runs.
    let mut none = split.clone();
    none.secret_subkeys.remove(0);
    assert!(signer(&none, None).is_none());
    assert!(self_test(&none).is_err());
}
