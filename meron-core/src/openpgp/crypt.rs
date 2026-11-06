//! Decrypt, verify and encrypt OpenPGP messages, with Meron's limits.
//!
//! Every input here is attacker-controlled. The rules (security notes R2/R4):
//! fail closed (no plaintext from a message whose integrity check failed),
//! bound the plaintext size (rPGP's decompression is not capped,
//! GHSA-9rq6-pw5r-326p), turn library panics into errors, and never use the
//! streaming SEIPDv1 mode that releases plaintext before the MDC is checked.

use std::io::{BufReader, Cursor, Read as _};
use std::panic::AssertUnwindSafe;

use pgp::composed::{
    Deserializable as _, DetachedSignature, Message, MessageBuilder, SignedPublicKey,
    SignedSecretKey, SubpacketConfig, TheRing,
};
use pgp::crypto::aead::{AeadAlgorithm, ChunkSize};
use pgp::crypto::hash::HashAlgorithm;
use pgp::crypto::sym::SymmetricKeyAlgorithm;
use pgp::packet::{Signature, Subpacket, SubpacketData};
use pgp::types::{CompressionAlgorithm, KeyDetails as _, Password, VerifyingKey};
use serde::Serialize;

use super::keys::{self, data_hash_ok, fingerprint_hex};

/// Largest ciphertext we try to open.
pub const MAX_CIPHERTEXT_BYTES: usize = 50 * 1024 * 1024;
/// Largest plaintext we accept after decryption and decompression.
pub const MAX_PLAINTEXT_BYTES: usize = 64 * 1024 * 1024;

/// Why a message could not be opened. No plaintext ever accompanies these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecryptError {
    /// Not encrypted to any of our keys.
    NoKey,
    /// The integrity check failed, or the data is corrupt: it may have been
    /// modified after it was sent.
    Integrity,
    /// Larger than [`MAX_CIPHERTEXT_BYTES`] or decompresses past
    /// [`MAX_PLAINTEXT_BYTES`].
    TooLarge,
    /// Not an OpenPGP message we can read (unsupported packets, malformed armor).
    Unreadable,
}

/// One signature found on a message, checked against the keys we know.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SigCheck {
    /// Issuer fingerprint (or 16-hex key ID when only that is present).
    pub issuer: Option<String>,
    /// Primary fingerprint of the known key that verified it.
    pub signer: Option<String>,
    /// The signature verified against `signer`, passed the hash policy, and
    /// the key was valid (not revoked, not expired) when it was made.
    pub valid: bool,
    /// The signature verified, but the key was revoked or expired at the time.
    pub key_problem: Option<keys::SigningProblem>,
    /// Made with SHA-1/MD5: never counts as valid.
    pub weak_hash: bool,
    pub created: Option<i64>,
    /// Primary fingerprints the signer named as intended recipients (RFC 9580
    /// §5.2.3.36), from the signed (hashed) subpackets. Empty when the sender
    /// set none. When present, our key must be among them, or the message was
    /// redirected to us by someone else.
    pub intended_recipients: Vec<String>,
    /// Verification could not complete: the signature broke the verifier (an
    /// rPGP panic, caught) rather than simply failing to match a key. Treated
    /// as an invalid signature, never as merely unconfirmed.
    pub errored: bool,
}

/// A successfully decrypted message.
#[derive(Debug)]
pub struct Decrypted {
    pub plaintext: Vec<u8>,
    pub signatures: Vec<SigCheck>,
    /// Primary fingerprint of our key that opened it.
    pub decrypted_with: Option<String>,
}

fn is_armored(data: &[u8]) -> bool {
    let head = &data[..data.len().min(1024)];
    std::str::from_utf8(head).is_ok_and(|s| s.trim_start().starts_with("-----BEGIN PGP"))
}

/// Decrypts `data` (armored or binary) with any of `secret_keys`, then checks
/// its signatures against `verifiers`.
pub fn decrypt(
    data: &[u8],
    secret_keys: &[&SignedSecretKey],
    verifiers: &[&SignedPublicKey],
) -> Result<Decrypted, DecryptError> {
    decrypt_with(data, secret_keys, verifiers, &keys::stored_key)
}

/// [`decrypt`] with an explicit resolver for designated revokers (used when
/// judging whether a signing key was revoked).
pub fn decrypt_with(
    data: &[u8],
    secret_keys: &[&SignedSecretKey],
    verifiers: &[&SignedPublicKey],
    resolve: keys::Resolver<'_>,
) -> Result<Decrypted, DecryptError> {
    if data.len() > MAX_CIPHERTEXT_BYTES {
        return Err(DecryptError::TooLarge);
    }
    let run = AssertUnwindSafe(|| decrypt_inner(data, secret_keys, verifiers, resolve));
    match std::panic::catch_unwind(run) {
        Ok(result) => result,
        Err(_) => Err(DecryptError::Unreadable),
    }
}

fn decrypt_inner(
    data: &[u8],
    secret_keys: &[&SignedSecretKey],
    verifiers: &[&SignedPublicKey],
    resolve: keys::Resolver<'_>,
) -> Result<Decrypted, DecryptError> {
    let reader = BufReader::new(Cursor::new(data));
    let msg = if is_armored(data) {
        Message::from_armor(reader).map(|(m, _)| m)
    } else {
        Message::from_bytes(reader)
    }
    .map_err(|_| DecryptError::Unreadable)?;
    if !msg.is_encrypted() {
        return Err(DecryptError::Unreadable);
    }
    let no_password = Password::empty();
    let ring = TheRing {
        secret_keys: secret_keys.to_vec(),
        key_passwords: vec![&no_password],
        // Default options: SEIPDv1 is MDC-checked before any plaintext is
        // released; legacy SED and GnuPG's packet-20 AEAD stay off.
        ..Default::default()
    };
    let (msg, ring_result) = msg.decrypt_the_ring(ring, true).map_err(|e| match e {
        pgp::errors::Error::MissingKey => DecryptError::NoKey,
        _ => DecryptError::Integrity,
    })?;
    let decrypted_with = secret_keys
        .iter()
        .zip(ring_result.secret_keys.iter())
        .find(|(_, status)| matches!(status, pgp::composed::InnerRingResult::Ok))
        .map(|(k, _)| fingerprint_hex(&k.primary_key));
    let mut msg = msg.decompress().map_err(|_| DecryptError::Integrity)?;
    let plaintext = read_bounded(&mut msg)?;
    let signatures = check_message_signatures(&msg, verifiers, resolve);
    Ok(Decrypted {
        plaintext,
        signatures,
        decrypted_with,
    })
}

/// Reads the literal data with a hard cap. Signatures can only be verified
/// once everything is read, which the cap guarantees is bounded.
fn read_bounded(msg: &mut Message<'_>) -> Result<Vec<u8>, DecryptError> {
    let mut out = Vec::new();
    let limit = u64::try_from(MAX_PLAINTEXT_BYTES).unwrap_or(u64::MAX) + 1;
    msg.by_ref()
        .take(limit)
        .read_to_end(&mut out)
        .map_err(|_| DecryptError::Integrity)?;
    if out.len() > MAX_PLAINTEXT_BYTES {
        return Err(DecryptError::TooLarge);
    }
    // A nested message (signed inside compressed inside signed) must also have
    // ended; anything left is treated as tampering.
    let mut probe = [0u8; 1];
    if msg.read(&mut probe).map_err(|_| DecryptError::Integrity)? != 0 {
        return Err(DecryptError::TooLarge);
    }
    Ok(out)
}

/// Every key that may have made a data signature: the primary and any
/// subkey with a valid binding that carries the sign flag.
fn signing_keys(key: &SignedPublicKey) -> Vec<(String, SigningPart<'_>)> {
    let mut out = vec![(
        fingerprint_hex(&key.primary_key),
        SigningPart::Primary(&key.primary_key),
    )];
    for sub in &key.public_subkeys {
        let bound = sub.signatures.iter().any(|s| {
            s.verify_subkey_binding(&key.primary_key, &sub.key).is_ok() && s.key_flags().sign()
        });
        if bound {
            out.push((fingerprint_hex(&sub.key), SigningPart::Sub(sub)));
        }
    }
    out
}

/// A primary key or signing subkey, the two concrete verifier types.
enum SigningPart<'a> {
    Primary(&'a pgp::packet::PublicKey),
    Sub(&'a pgp::composed::SignedPublicSubKey),
}

impl SigningPart<'_> {
    /// Whether this part's key was valid when `sig` was made (signatures
    /// by revoked or expired keys don't count).
    fn valid_at(
        &self,
        key: &SignedPublicKey,
        sig: &Signature,
        resolve: keys::Resolver<'_>,
    ) -> Result<(), keys::SigningProblem> {
        let when = sig.created().map(|t| t.as_secs()).unwrap_or_else(keys::now);
        match self {
            SigningPart::Primary(_) => keys::primary_valid_at_with(key, when, resolve),
            SigningPart::Sub(sub) => keys::subkey_valid_at_with(key, sub, when, resolve),
        }
    }

    fn as_dyn(&self) -> &dyn VerifyingKey {
        match self {
            SigningPart::Primary(k) => *k,
            SigningPart::Sub(k) => &k.key,
        }
    }

    fn verify_detached(&self, sig: &Signature, content: &[u8]) -> bool {
        match self {
            SigningPart::Primary(k) => sig.verify(*k, content).is_ok(),
            SigningPart::Sub(k) => sig.verify(&k.key, content).is_ok(),
        }
    }
}

fn issuer_of(sig: &Signature) -> Option<String> {
    sig.issuer_fingerprint()
        .first()
        .map(|f| keys_hex(f.as_bytes()))
        .or_else(|| sig.issuer_key_id().first().map(|id| keys_hex(id.as_ref())))
}

fn keys_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

/// Primary fingerprints named as intended recipients in the signature's signed
/// (hashed) subpackets. Unhashed subpackets are attacker-malleable, so only the
/// hashed ones count.
fn intended_recipients(sig: &Signature) -> Vec<String> {
    sig.config()
        .into_iter()
        .flat_map(|c| c.hashed_subpackets())
        .filter_map(|sp| match &sp.data {
            SubpacketData::IntendedRecipientFingerprint(fp) => Some(keys_hex(fp.as_bytes())),
            _ => None,
        })
        .collect()
}

/// Whether the signature's issuer could be one of `key`'s signing parts.
fn issuer_matches(issuer: Option<&str>, key: &SignedPublicKey) -> bool {
    let Some(issuer) = issuer else { return true };
    signing_keys(key)
        .iter()
        .any(|(fpr, _)| fpr == issuer || fpr.ends_with(issuer))
}

fn check_message_signatures(
    msg: &Message<'_>,
    verifiers: &[&SignedPublicKey],
    resolve: keys::Resolver<'_>,
) -> Vec<SigCheck> {
    let Message::Signed { reader, .. } = msg else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for index in 0..reader.num_signatures() {
        let Some(sig) = reader.signature(index) else {
            continue;
        };
        let issuer = issuer_of(sig);
        let weak_hash = !data_hash_ok(sig.hash_alg());
        let created = sig.created().map(|t| i64::from(t.as_secs()));
        let mut check = SigCheck {
            issuer: issuer.clone(),
            signer: None,
            valid: false,
            key_problem: None,
            weak_hash,
            created,
            intended_recipients: intended_recipients(sig),
            errored: false,
        };
        'keys: for key in verifiers
            .iter()
            .filter(|k| issuer_matches(issuer.as_deref(), k))
        {
            for (_, vk) in signing_keys(key) {
                if msg.verify_nested_explicit(index, vk.as_dyn()).is_ok() {
                    check.signer = Some(fingerprint_hex(&key.primary_key));
                    check.key_problem = vk.valid_at(key, sig, resolve).err();
                    check.valid = !weak_hash && check.key_problem.is_none();
                    break 'keys;
                }
            }
        }
        out.push(check);
    }
    out
}

/// Verifies a detached (PGP/MIME `multipart/signed`) signature over `content`,
/// the exact bytes of the signed MIME part.
pub fn verify_detached(
    content: &[u8],
    signature: &[u8],
    verifiers: &[&SignedPublicKey],
) -> Vec<SigCheck> {
    verify_detached_with(content, signature, verifiers, &keys::stored_key)
}

/// [`verify_detached`] with an explicit resolver for designated revokers.
pub fn verify_detached_with(
    content: &[u8],
    signature: &[u8],
    verifiers: &[&SignedPublicKey],
    resolve: keys::Resolver<'_>,
) -> Vec<SigCheck> {
    let run = AssertUnwindSafe(|| verify_detached_inner(content, signature, verifiers, resolve));
    // A panic in rPGP means verification could not complete. Surface it as one
    // failed, errored check so the message is flagged invalid, not shown as a
    // bland "signed" with no problem.
    std::panic::catch_unwind(run).unwrap_or_else(|_| {
        vec![SigCheck {
            issuer: None,
            signer: None,
            valid: false,
            key_problem: None,
            weak_hash: false,
            created: None,
            intended_recipients: Vec::new(),
            errored: true,
        }]
    })
}

fn verify_detached_inner(
    content: &[u8],
    signature: &[u8],
    verifiers: &[&SignedPublicKey],
    resolve: keys::Resolver<'_>,
) -> Vec<SigCheck> {
    let parsed = if is_armored(signature) {
        DetachedSignature::from_armor_many(BufReader::new(Cursor::new(signature)))
            .map(|(iter, _)| iter.filter_map(Result::ok).collect::<Vec<_>>())
    } else {
        DetachedSignature::from_bytes_many(BufReader::new(Cursor::new(signature)))
            .map(|iter| iter.filter_map(Result::ok).collect::<Vec<_>>())
    };
    let Ok(sigs) = parsed else {
        return vec![SigCheck {
            issuer: None,
            signer: None,
            valid: false,
            key_problem: None,
            weak_hash: false,
            created: None,
            intended_recipients: Vec::new(),
            errored: false,
        }];
    };
    sigs.iter()
        .map(|detached| {
            let sig = &detached.signature;
            let issuer = issuer_of(sig);
            let weak_hash = !data_hash_ok(sig.hash_alg());
            let created = sig.created().map(|t| i64::from(t.as_secs()));
            let mut check = SigCheck {
                issuer: issuer.clone(),
                signer: None,
                valid: false,
                key_problem: None,
                weak_hash,
                created,
                intended_recipients: intended_recipients(sig),
                errored: false,
            };
            'keys: for key in verifiers
                .iter()
                .filter(|k| issuer_matches(issuer.as_deref(), k))
            {
                for (_, vk) in signing_keys(key) {
                    if vk.verify_detached(sig, content) {
                        check.signer = Some(fingerprint_hex(&key.primary_key));
                        check.key_problem = vk.valid_at(key, sig, resolve).err();
                        check.valid = !weak_hash && check.key_problem.is_none();
                        break 'keys;
                    }
                }
            }
            check
        })
        .collect()
}

/// Encrypts `plain` to every key in `recipients` (include our own key, or the
/// Sent copy is unreadable to us), signed by `signer`.
///
/// SEIPDv2 (AEAD, RFC 9580) is used only when every recipient key advertises
/// it; GnuPG 2.4 doesn't, so mixed audiences get SEIPDv1 with MDC.
/// Recipient key IDs are written in the clear, as GnuPG and Thunderbird
/// expect; callers keep Bcc recipients in separate messages.
pub fn encrypt(
    plain: &[u8],
    recipients: &[&SignedPublicKey],
    signer: &SignedSecretKey,
) -> anyhow::Result<String> {
    // The signature names exactly the recipients the message is encrypted to.
    encrypt_to(plain, recipients, recipients, signer)
}

/// [`encrypt`], but the signature's intended-recipient subpackets name
/// `intended` rather than `recipients`. In real use they are the same; tests
/// pass a different `intended` to forge a message redirected to someone the
/// sender did not name.
pub(crate) fn encrypt_to(
    plain: &[u8],
    recipients: &[&SignedPublicKey],
    intended: &[&SignedPublicKey],
    signer: &SignedSecretKey,
) -> anyhow::Result<String> {
    use anyhow::Context as _;
    let now = keys::now();
    let subkeys: Vec<_> = recipients
        .iter()
        .map(|k| {
            keys::encryption_subkey(k, now).with_context(|| {
                format!(
                    "no usable encryption key in {}",
                    fingerprint_hex(&k.primary_key)
                )
            })
        })
        .collect::<anyhow::Result<_>>()?;
    if subkeys.is_empty() {
        anyhow::bail!("no recipients");
    }
    let v2 = recipients.iter().all(|k| keys::supports_seipd_v2(k));
    let mut rng = rand::thread_rng();
    let ts = pgp::types::Timestamp::now();
    let subpackets = signature_subpackets(ts, signer, intended)?;
    let builder = MessageBuilder::from_bytes("", plain.to_vec());
    let armored = if v2 {
        let mut msg = builder.seipd_v2(
            &mut rng,
            SymmetricKeyAlgorithm::AES256,
            AeadAlgorithm::Ocb,
            ChunkSize::C8KiB,
        );
        for sub in &subkeys {
            msg.encrypt_to_key(&mut rng, sub)?;
        }
        msg.sign_with_subpackets(
            &signer.primary_key,
            Password::empty(),
            HashAlgorithm::Sha256,
            subpackets,
        );
        msg.compression(CompressionAlgorithm::ZLIB);
        msg.to_armored_string(&mut rng, Default::default())?
    } else {
        let mut msg = builder.seipd_v1(&mut rng, SymmetricKeyAlgorithm::AES256);
        for sub in &subkeys {
            msg.encrypt_to_key(&mut rng, sub)?;
        }
        msg.sign_with_subpackets(
            &signer.primary_key,
            Password::empty(),
            HashAlgorithm::Sha256,
            subpackets,
        );
        msg.compression(CompressionAlgorithm::ZLIB);
        msg.to_armored_string(&mut rng, Default::default())?
    };
    Ok(armored)
}

/// Hashed subpackets for our data signatures: creation time, issuer
/// fingerprint and the intended recipients (RFC 9580 §5.2.3.36), which lets a
/// recipient see the message was not re-encrypted to them by someone else.
fn signature_subpackets(
    now: pgp::types::Timestamp,
    signer: &SignedSecretKey,
    recipients: &[&SignedPublicKey],
) -> anyhow::Result<SubpacketConfig> {
    let mut hashed = vec![Subpacket::critical(SubpacketData::SignatureCreationTime(
        now,
    ))?];
    hashed.push(Subpacket::regular(SubpacketData::IssuerFingerprint(
        signer.primary_key.fingerprint(),
    ))?);
    for key in recipients {
        hashed.push(Subpacket::regular(
            SubpacketData::IntendedRecipientFingerprint(key.primary_key.fingerprint()),
        )?);
    }
    let unhashed = vec![Subpacket::regular(SubpacketData::IssuerKeyId(
        signer.primary_key.legacy_key_id(),
    ))?];
    Ok(SubpacketConfig::UserDefined { hashed, unhashed })
}

/// A detached signature over `content` (for tests and possible sign-only use).
#[cfg(test)]
pub fn sign_detached(content: &[u8], signer: &SignedSecretKey) -> anyhow::Result<String> {
    let mut rng = rand::thread_rng();
    let sig = DetachedSignature::sign_binary_data(
        &mut rng,
        &signer.primary_key,
        &Password::empty(),
        HashAlgorithm::Sha256,
        content,
    )?;
    Ok(sig.to_armored_string(Default::default())?)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::openpgp::keys::{Unlock, parse_keys, unlock};

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/openpgp/testdata")
                .join(name),
        )
        .expect("fixture")
    }

    fn alice() -> SignedSecretKey {
        let key = parse_keys(&fixture("alice-secret.asc"))
            .unwrap()
            .secret
            .remove(0);
        match unlock(key, None).unwrap() {
            Unlock::Ok(k) => k,
            other => panic!("{other:?}"),
        }
    }

    fn public(name: &str) -> SignedPublicKey {
        parse_keys(&fixture(name)).unwrap().public.remove(0)
    }

    /// The armored block inside a fixture .eml.
    fn armored_block(eml: &[u8]) -> Vec<u8> {
        let text = String::from_utf8_lossy(eml);
        let start = text.find("-----BEGIN PGP MESSAGE-----").unwrap();
        let end =
            text.find("-----END PGP MESSAGE-----").unwrap() + "-----END PGP MESSAGE-----".len();
        text[start..end].replace("\r\n", "\n").into_bytes()
    }

    #[test]
    fn decrypts_and_verifies_gnupg_signed_and_encrypted_mail() {
        let alice = alice();
        let carol = public("carol-public.asc");
        let out = decrypt(
            &armored_block(&fixture("enc-signed.eml")),
            &[&alice],
            &[&carol],
        )
        .unwrap();
        let text = String::from_utf8(out.plaintext).unwrap();
        assert!(text.contains("The meeting moved to Thursday"));
        assert!(text.contains("protected-headers=\"v1\""));
        assert_eq!(
            out.decrypted_with.as_deref(),
            Some(fingerprint_hex(&alice.primary_key).as_str())
        );
        assert_eq!(out.signatures.len(), 1);
        assert!(out.signatures[0].valid);
        assert_eq!(
            out.signatures[0].signer.as_deref(),
            Some(fingerprint_hex(&carol.primary_key).as_str())
        );
    }

    #[test]
    fn unknown_signer_is_reported_with_its_issuer_but_not_valid() {
        let out = decrypt(&armored_block(&fixture("enc-signed.eml")), &[&alice()], &[]).unwrap();
        assert_eq!(out.signatures.len(), 1);
        assert!(!out.signatures[0].valid);
        assert!(out.signatures[0].issuer.is_some());
        assert_eq!(out.signatures[0].signer, None);
    }

    #[test]
    fn signature_by_another_key_does_not_verify_as_carol() {
        let carol = public("carol-public.asc");
        let out = decrypt(
            &armored_block(&fixture("enc-signer-mismatch.eml")),
            &[&alice()],
            &[&carol],
        )
        .unwrap();
        assert!(!out.signatures[0].valid);
        let mallory = public("mallory-public.asc");
        let out = decrypt(
            &armored_block(&fixture("enc-signer-mismatch.eml")),
            &[&alice()],
            &[&carol, &mallory],
        )
        .unwrap();
        assert_eq!(
            out.signatures[0].signer.as_deref(),
            Some(fingerprint_hex(&mallory.primary_key).as_str())
        );
    }

    #[test]
    fn unsigned_encrypted_mail_has_no_signatures() {
        let out = decrypt(
            &armored_block(&fixture("enc-unsigned.eml")),
            &[&alice()],
            &[],
        )
        .unwrap();
        assert!(out.signatures.is_empty());
        assert!(
            String::from_utf8(out.plaintext)
                .unwrap()
                .contains("Unsigned secret")
        );
    }

    #[test]
    fn tampered_ciphertext_fails_closed() {
        let err = decrypt(
            &armored_block(&fixture("enc-tampered.eml")),
            &[&alice()],
            &[],
        )
        .unwrap_err();
        assert_eq!(err, DecryptError::Integrity);
    }

    #[test]
    fn mail_for_someone_else_reports_no_key() {
        let err = decrypt(
            &armored_block(&fixture("enc-wrong-recipient.eml")),
            &[&alice()],
            &[],
        )
        .unwrap_err();
        assert_eq!(err, DecryptError::NoKey);
    }

    #[test]
    fn decompression_bomb_is_stopped_at_the_plaintext_cap() {
        let err = decrypt(&armored_block(&fixture("bomb.eml")), &[&alice()], &[]).unwrap_err();
        assert_eq!(err, DecryptError::TooLarge);
    }

    #[test]
    fn garbage_is_unreadable_not_a_panic() {
        assert_eq!(
            decrypt(
                b"-----BEGIN PGP MESSAGE-----\n\nAAAA\n-----END PGP MESSAGE-----\n",
                &[&alice()],
                &[]
            )
            .unwrap_err(),
            DecryptError::Unreadable
        );
        assert_eq!(
            decrypt(&[0xff; 64], &[&alice()], &[]).unwrap_err(),
            DecryptError::Unreadable
        );
    }

    #[test]
    fn our_encryption_round_trips_and_verifies() {
        let me = keys::generate(&["me@example.org".into()]).unwrap();
        let peer = keys::generate(&["peer@example.org".into()]).unwrap();
        let armored = encrypt(
            b"hello peer",
            &[&peer.to_public_key(), &me.to_public_key()],
            &me,
        )
        .unwrap();
        for reader in [&peer, &me] {
            let out = decrypt(armored.as_bytes(), &[reader], &[&me.to_public_key()]).unwrap();
            assert_eq!(out.plaintext, b"hello peer");
            assert!(out.signatures[0].valid);
        }
    }

    #[test]
    fn mixed_audience_with_gnupg_key_uses_seipd_v1() {
        let me = keys::generate(&["me@example.org".into()]).unwrap();
        let carol = public("carol-public.asc");
        assert!(!keys::supports_seipd_v2(&carol));
        let armored = encrypt(b"to carol", &[&carol, &me.to_public_key()], &me).unwrap();
        let out = decrypt(armored.as_bytes(), &[&me], &[]).unwrap();
        assert_eq!(out.plaintext, b"to carol");
    }

    #[test]
    fn detached_signature_verifies_over_exact_bytes() {
        let eml = fixture("signed.eml");
        let text = String::from_utf8(eml).unwrap();
        // Signed part: between the first boundary line and the CRLF before the next one.
        let b = "--sig-boundary\r\n";
        let start = text.find(b).unwrap() + b.len();
        let end = text[start..].find("\r\n--sig-boundary").unwrap() + start;
        let content = &text.as_bytes()[start..end];
        let sig = armored_block_sig(&text);
        let carol = public("carol-public.asc");
        let checks = verify_detached(content, sig.as_bytes(), &[&carol]);
        assert!(checks[0].valid, "{checks:?}");
        let mut altered = content.to_vec();
        altered[0] ^= 0x20;
        assert!(!verify_detached(&altered, sig.as_bytes(), &[&carol])[0].valid);
    }

    #[test]
    fn sha1_detached_signature_is_weak_and_never_valid() {
        let text = String::from_utf8(fixture("signed-sha1.eml")).unwrap();
        let b = "--sig-boundary\r\n";
        let start = text.find(b).unwrap() + b.len();
        let end = text[start..].find("\r\n--sig-boundary").unwrap() + start;
        let checks = verify_detached(
            &text.as_bytes()[start..end],
            armored_block_sig(&text).as_bytes(),
            &[&public("carol-public.asc")],
        );
        assert!(checks[0].weak_hash);
        assert!(!checks[0].valid);
    }

    /// Runs gpg in a throwaway home. None when gpg isn't installed (the
    /// interop tests then say so and pass, rather than fail on CI images
    /// without GnuPG).
    pub(crate) fn gpg(
        home: &std::path::Path,
        args: &[&str],
        stdin: &[u8],
    ) -> Option<(bool, Vec<u8>, String)> {
        use std::io::Write as _;
        let mut child = std::process::Command::new("gpg")
            .arg("--homedir")
            .arg(home)
            .args([
                "--batch",
                "--yes",
                "--no-tty",
                "--pinentry-mode",
                "loopback",
                "--passphrase",
                "",
                "--trust-model",
                "always",
            ])
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .ok()?;
        child.stdin.take()?.write_all(stdin).ok()?;
        let out = child.wait_with_output().ok()?;
        Some((
            out.status.success(),
            out.stdout,
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ))
    }

    /// A throwaway GNUPGHOME, removed (and its agent stopped) on drop.
    pub(crate) struct GpgHome(std::path::PathBuf);

    impl GpgHome {
        pub(crate) fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for GpgHome {
        fn drop(&mut self) {
            let _ = std::process::Command::new("gpgconf")
                .arg("--homedir")
                .arg(&self.0)
                .args(["--kill", "all"])
                .output();
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn new_gpg_home() -> Option<GpgHome> {
        let dir = std::env::temp_dir().join(format!("meron-gpg-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir(&dir).ok()?;
        std::fs::set_permissions(&dir, std::os::unix::fs::PermissionsExt::from_mode(0o700)).ok()?;
        Some(GpgHome(dir))
    }

    /// A throwaway GNUPGHOME for an interop test, or None (the reason on
    /// stderr) when gpg is missing or can't use secret keys. Some CI images
    /// (GitHub's ubuntu-26.04 runner) import a secret key, then gpg-agent
    /// answers NO_SECKEY / "Bad secret key": that is the environment, not
    /// Meron's output (the same GnuPG release decrypts it elsewhere), so the
    /// tests skip there instead of failing.
    pub(crate) fn gpg_home() -> Option<GpgHome> {
        let home = new_gpg_home()?;
        if gpg(home.path(), &["--version"], b"").is_none() {
            eprintln!("gpg not installed: interop test skipped");
            return None;
        }
        if !gpg_can_use_secret_keys() {
            eprintln!(
                "gpg can't use an imported secret key here (gpg-agent): interop test skipped"
            );
            return None;
        }
        Some(home)
    }

    /// What the interop tests need from gpg-agent: import a fresh secret key
    /// into its own home and decrypt a message to it.
    fn gpg_can_use_secret_keys() -> bool {
        let Some(probe) = new_gpg_home() else {
            return false;
        };
        let key = keys::generate(&["probe@example.org".into()]).unwrap();
        let Ok(secret) = keys::binary_secret(&key) else {
            return false;
        };
        if !gpg(probe.path(), &["--import"], &secret).is_some_and(|(ok, _, _)| ok) {
            return false;
        }
        let Ok(armored) = encrypt(b"probe", &[&key.to_public_key()], &key) else {
            return false;
        };
        gpg(probe.path(), &["--decrypt"], armored.as_bytes())
            .is_some_and(|(ok, plain, _)| ok && plain == b"probe")
    }

    #[test]
    fn gnupg_decrypts_and_verifies_what_we_encrypt() {
        let Some(home) = gpg_home() else {
            return;
        };
        let me = keys::generate(&["me@example.org".into()]).unwrap();
        let peer = keys::generate(&["peer@example.org".into()]).unwrap();
        let (ok, _, err) = gpg(
            home.path(),
            &["--import"],
            keys::binary_secret(&peer).unwrap().as_slice(),
        )
        .unwrap();
        assert!(ok, "{err}");
        let (ok, _, err) = gpg(
            home.path(),
            &["--import"],
            keys::armored_public(&me.to_public_key())
                .unwrap()
                .as_bytes(),
        )
        .unwrap();
        assert!(ok, "{err}");
        let armored = encrypt(
            b"interop check",
            &[&peer.to_public_key(), &me.to_public_key()],
            &me,
        )
        .unwrap();
        let (ok, plain, err) = gpg(
            home.path(),
            &["--status-fd", "2", "--decrypt"],
            armored.as_bytes(),
        )
        .unwrap();
        assert!(ok, "{err}");
        assert_eq!(plain, b"interop check");
        assert!(err.contains("[GNUPG:] GOODSIG"), "{err}");
        assert!(err.contains("[GNUPG:] DECRYPTION_OKAY"), "{err}");
    }

    fn armored_block_sig(text: &str) -> String {
        let start = text.find("-----BEGIN PGP SIGNATURE-----").unwrap();
        let end =
            text.find("-----END PGP SIGNATURE-----").unwrap() + "-----END PGP SIGNATURE-----".len();
        text[start..end].replace("\r\n", "\n")
    }
}
