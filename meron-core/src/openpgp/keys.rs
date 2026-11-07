//! OpenPGP keys on top of rPGP: generate, parse, unlock, validate.
//!
//! rPGP is a toolkit, not a policy (its README): expiry, revocation, key flags
//! and algorithm choices are the caller's job. This module is that policy, kept
//! in one place so every other part of Meron asks the same questions.

use std::io::{BufReader, Cursor};

use anyhow::{Context as _, Result, anyhow, bail};
use pgp::composed::{
    EncryptionCaps, KeyType, PublicOrSecret, SecretKeyParamsBuilder, SignedPublicKey,
    SignedPublicSubKey, SignedSecretKey, SubkeyParamsBuilder,
};
use pgp::crypto::ecc_curve::ECCCurve;
use pgp::crypto::hash::HashAlgorithm;
use pgp::crypto::sym::SymmetricKeyAlgorithm;
use pgp::packet::Signature;
use pgp::types::{CompressionAlgorithm, KeyDetails as _, Password};
use serde::Serialize;
use smallvec::smallvec;

/// SHA-1 binding signatures made after this moment are rejected (2023-02-01,
/// the cut-off Sequoia's standard policy uses). Older keys keep working.
pub const SHA1_BINDING_CUTOFF: u32 = 1_675_209_600;

/// Largest key file we parse. A big keyring export is a few MiB; anything far
/// larger is a mistake or an attack on the parser.
pub const MAX_KEY_FILE_BYTES: usize = 16 * 1024 * 1024;

/// Seconds since the epoch, as rPGP counts time.
pub fn now() -> u32 {
    u32::try_from(chrono::Utc::now().timestamp()).unwrap_or(u32::MAX)
}

/// Upper-case hex fingerprint, the form stored and shown everywhere.
pub fn fingerprint_hex(key: &impl pgp::types::KeyDetails) -> String {
    hex_upper(key.fingerprint().as_bytes())
}

fn hex_upper(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

/// Why a key (or a whole certificate) can't be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Unusable {
    Revoked,
    Expired,
    /// No valid self-signature binding a user ID (or the bindings use a weak hash).
    InvalidBindings,
    /// No subkey that may encrypt.
    NoEncryptionKey,
    /// Only a GnuPG stub: the secret part lives on a smartcard or elsewhere.
    SecretOnCard,
}

/// A key's facts for the UI and the store. Never holds secret material.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct KeyInfo {
    pub fingerprint: String,
    pub user_ids: Vec<String>,
    /// Lower-cased e-mail addresses from the user IDs.
    pub emails: Vec<String>,
    pub created: i64,
    pub expires: Option<i64>,
    pub algorithm: String,
    pub has_secret: bool,
    pub can_encrypt: bool,
    pub supports_seipd_v2: bool,
    pub unusable: Option<Unusable>,
}

/// Generates a new key for `addrs` (the first is the primary user ID): an
/// Ed25519 primary for certifying and signing plus a Cv25519 encryption
/// subkey, v4 so GnuPG 2.4 can use it, no expiry and no passphrase (the key is
/// stored inside the encrypted database). Same profile as Delta Chat's, except
/// that it does not advertise SEIPDv2: users may export the key to GnuPG 2.4,
/// which can't read SEIPDv2 (found by the GnuPG interop test), so peers keep
/// sending SEIPDv1 with MDC, which every client reads.
pub fn generate(addrs: &[String]) -> Result<SignedSecretKey> {
    let first = addrs.first().context("a key needs at least one address")?;
    let mut params = SecretKeyParamsBuilder::default();
    params
        .key_type(KeyType::Ed25519Legacy)
        .can_certify(true)
        .can_sign(true)
        .feature_seipd_v2(false)
        .primary_user_id(format!("<{first}>"))
        .passphrase(None)
        .preferred_symmetric_algorithms(smallvec![
            SymmetricKeyAlgorithm::AES256,
            SymmetricKeyAlgorithm::AES192,
            SymmetricKeyAlgorithm::AES128,
        ])
        .preferred_hash_algorithms(smallvec![
            HashAlgorithm::Sha256,
            HashAlgorithm::Sha384,
            HashAlgorithm::Sha512,
        ])
        .preferred_compression_algorithms(smallvec![
            CompressionAlgorithm::ZLIB,
            CompressionAlgorithm::ZIP,
        ])
        .subkey(
            SubkeyParamsBuilder::default()
                .key_type(KeyType::ECDH(ECCCurve::Curve25519Legacy))
                .can_encrypt(EncryptionCaps::All)
                .passphrase(None)
                .build()
                .context("subkey parameters")?,
        );
    for addr in addrs.iter().skip(1) {
        params.user_id(format!("<{addr}>"));
    }
    let params = params.build().context("key parameters")?;
    let mut rng = rand::thread_rng();
    let key = params.generate(&mut rng).context("generate key")?;
    key.verify_bindings()
        .context("generated key failed its own check")?;
    Ok(key)
}

/// Everything found in an imported file.
#[derive(Debug, Default)]
pub struct Parsed {
    pub public: Vec<SignedPublicKey>,
    pub secret: Vec<SignedSecretKey>,
    /// Keys that could not be parsed at all (count only: the bytes are untrusted).
    pub unreadable: usize,
}

/// Parses one or many keys, armored or binary, public or secret. A text file
/// may hold several armor blocks (concatenated exports).
pub fn parse_keys(data: &[u8]) -> Result<Parsed> {
    if data.len() > MAX_KEY_FILE_BYTES {
        bail!("key file is too large");
    }
    let mut out = Parsed::default();
    let text = std::str::from_utf8(data).ok();
    let blocks: Vec<&[u8]> = match text {
        Some(t) if t.contains("-----BEGIN PGP") => split_armor_blocks(t),
        _ => vec![data],
    };
    for block in blocks {
        guarded(|| {
            let (iter, _headers) =
                PublicOrSecret::from_reader_many(BufReader::new(Cursor::new(block)))?;
            let mut found = Vec::new();
            for item in iter {
                found.push(item);
            }
            Ok(found)
        })
        .map(|items| {
            for item in items {
                match item {
                    Ok(PublicOrSecret::Public(k)) => out.public.push(k),
                    Ok(PublicOrSecret::Secret(k)) => out.secret.push(k),
                    Err(_) => out.unreadable += 1,
                }
            }
        })
        .unwrap_or_else(|_| out.unreadable += 1);
    }
    Ok(out)
}

fn split_armor_blocks(text: &str) -> Vec<&[u8]> {
    let mut blocks = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("-----BEGIN PGP") {
        let tail = &rest[start..];
        let end = tail
            .find("-----END PGP")
            .and_then(|e| {
                tail[e..]
                    .find('\n')
                    .map(|nl| e + nl + 1)
                    .or(Some(tail.len()))
            })
            .unwrap_or(tail.len());
        blocks.push(tail[..end].as_bytes());
        rest = &tail[end..];
    }
    blocks
}

/// Runs a closure that calls into rPGP and turns a panic into an error: rPGP
/// has panicked on crafted input before (CVE-2024-53856), and one bad mail
/// or key file must not take down the sidecar.
pub fn guarded<T>(
    f: impl FnOnce() -> pgp::errors::Result<T> + std::panic::UnwindSafe,
) -> Result<T> {
    match std::panic::catch_unwind(f) {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => Err(anyhow!("{e}")),
        Err(_) => Err(anyhow!("OpenPGP library panicked on this input")),
    }
}

/// The result of opening an imported secret key with its passphrase.
#[derive(Debug)]
pub enum Unlock {
    Ok(SignedSecretKey),
    /// The key has a passphrase and none (or the wrong one) was given.
    NeedsPassphrase,
    /// A GnuPG stub without the secret primary key (smartcard, offline primary).
    OnCard,
}

/// Removes the passphrase from every secret part of `key`, so it can be stored
/// inside the encrypted database and used without prompts. Then self-tests it.
pub fn unlock(mut key: SignedSecretKey, passphrase: Option<&str>) -> Result<Unlock> {
    if is_stub(&key) {
        return Ok(Unlock::OnCard);
    }
    let pw = Password::from(passphrase.unwrap_or(""));
    if key.primary_key.remove_password(&pw).is_err() {
        return Ok(Unlock::NeedsPassphrase);
    }
    for sub in &mut key.secret_subkeys {
        if sub.key.remove_password(&pw).is_err() {
            return Ok(Unlock::NeedsPassphrase);
        }
    }
    self_test(&key)?;
    Ok(Unlock::Ok(key))
}

/// GnuPG marks a missing secret with the private S2K extension 101
/// ("gnu-dummy" or "divert-to-card"). rPGP keeps it as an encrypted part it
/// can never open, which looks like a wrong passphrase unless detected here.
fn is_stub(key: &SignedSecretKey) -> bool {
    use pgp::types::{S2kParams, SecretParams, StringToKey};
    let SecretParams::Encrypted(enc) = key.primary_key.secret_params() else {
        return false;
    };
    // GnuPG's gnu-dummy / divert-to-card uses a Private (100-110) S2K type.
    // Match the parsed S2K structurally rather than its debug string.
    let s2k = match enc.string_to_key_params() {
        S2kParams::Cfb { s2k, .. }
        | S2kParams::MalleableCfb { s2k, .. }
        | S2kParams::Aead { s2k, .. } => s2k,
        S2kParams::Unprotected | S2kParams::LegacyCfb { .. } => return false,
    };
    matches!(s2k, StringToKey::Private { .. })
}

/// Signs a nonce with every signing-capable secret part and verifies it with
/// the matching public part. Catches a corrupted key, or one whose public half
/// was swapped (the "key overwriting" attack), before it is ever used.
pub fn self_test(key: &SignedSecretKey) -> Result<()> {
    key.verify_bindings().context("key bindings")?;
    let public = key.to_public_key();
    let nonce = uuid::Uuid::new_v4();
    let mut rng = rand::thread_rng();
    let sig = pgp::composed::DetachedSignature::sign_binary_data(
        &mut rng,
        &key.primary_key,
        &Password::empty(),
        HashAlgorithm::Sha256,
        nonce.as_bytes().as_slice(),
    )
    .context("self-test signature")?;
    sig.verify(&public, nonce.as_bytes().as_slice())
        .context("self-test signature didn't verify: the key is damaged")?;
    Ok(())
}

/// Exports `key` armored, protected with `passphrase` (iterated+salted S2K
/// with CFB, which GnuPG, Thunderbird and OpenKeychain all read).
pub fn export_protected(key: &SignedSecretKey, passphrase: &str) -> Result<String> {
    if passphrase.is_empty() {
        bail!("a passphrase is required to export a secret key");
    }
    let mut key = key.clone();
    let pw = Password::from(passphrase);
    let mut rng = rand::thread_rng();
    key.primary_key
        .set_password(&mut rng, &pw)
        .context("protect primary key")?;
    for sub in &mut key.secret_subkeys {
        sub.key
            .set_password(&mut rng, &pw)
            .context("protect subkey")?;
    }
    Ok(key.to_armored_string(Default::default())?)
}

/// Armored public key.
pub fn armored_public(key: &SignedPublicKey) -> Result<String> {
    Ok(key.to_armored_string(Default::default())?)
}

/// Binary (unarmored) public key, as Autocrypt's `keydata` wants it.
pub fn binary_public(key: &SignedPublicKey) -> Result<Vec<u8>> {
    use pgp::ser::Serialize as _;
    let mut out = Vec::new();
    key.to_writer(&mut out)?;
    Ok(out)
}

/// Parses one public key from armored or binary bytes.
pub fn parse_public(data: &[u8]) -> Result<SignedPublicKey> {
    let parsed = parse_keys(data)?;
    if let Some(k) = parsed.public.into_iter().next() {
        return Ok(k);
    }
    if let Some(k) = parsed.secret.into_iter().next() {
        return Ok(k.to_public_key());
    }
    bail!("no OpenPGP key found")
}

/// Parses one stored (unprotected) secret key.
pub fn parse_secret(data: &[u8]) -> Result<SignedSecretKey> {
    parse_keys(data)?
        .secret
        .into_iter()
        .next()
        .context("no secret key found")
}

/// Serialises a secret key for storage (binary, unprotected: it lives inside
/// SQLCipher).
pub fn binary_secret(key: &SignedSecretKey) -> Result<zeroize::Zeroizing<Vec<u8>>> {
    use pgp::ser::Serialize as _;
    let mut out = zeroize::Zeroizing::new(Vec::new());
    key.to_writer(&mut *out)?;
    Ok(out)
}

/// Lower-cased address inside a user ID such as `Name <a@b>` or `<a@b>`.
pub fn email_of_user_id(uid: &str) -> Option<String> {
    let addr = match (uid.rfind('<'), uid.rfind('>')) {
        (Some(s), Some(e)) if e > s => &uid[s + 1..e],
        _ => uid.trim(),
    };
    (addr.contains('@') && !addr.contains(char::is_whitespace)).then(|| addr.to_ascii_lowercase())
}

/// Policy check for one self-signature: a SHA-1/MD5 binding is only accepted
/// when it is old (see [`SHA1_BINDING_CUTOFF`]).
pub(crate) fn binding_hash_ok(sig: &Signature) -> bool {
    match sig.hash_alg() {
        Some(HashAlgorithm::Md5) | Some(HashAlgorithm::Ripemd160) => false,
        Some(HashAlgorithm::Sha1) => sig
            .created()
            .map(|t| t.as_secs() < SHA1_BINDING_CUTOFF)
            .unwrap_or(false),
        _ => true,
    }
}

/// Whether a data signature's hash is acceptable today.
pub fn data_hash_ok(alg: Option<HashAlgorithm>) -> bool {
    !matches!(
        alg,
        Some(HashAlgorithm::Md5)
            | Some(HashAlgorithm::Sha1)
            | Some(HashAlgorithm::Ripemd160)
            | None
    )
}

fn newest<'a>(sigs: impl Iterator<Item = &'a Signature>) -> Option<&'a Signature> {
    sigs.max_by_key(|s| s.created().map(|t| t.as_secs()).unwrap_or(0))
}

/// A revocation (of the key, a user ID or a subkey). rPGP's certification and
/// key checks accept these too, so they have to be told apart from bindings.
fn is_revocation(sig: &Signature) -> bool {
    use pgp::packet::SignatureType;
    matches!(
        sig.typ(),
        Some(SignatureType::KeyRevocation)
            | Some(SignatureType::CertRevocation)
            | Some(SignatureType::SubkeyRevocation)
    )
}

/// User IDs whose newest self-certification verifies and passes the hash
/// policy, and isn't a revocation of that user ID.
fn valid_user_ids(key: &SignedPublicKey) -> Vec<String> {
    key.details
        .users
        .iter()
        .filter(|u| {
            let valid: Vec<&Signature> = u
                .signatures
                .iter()
                .filter(|s| {
                    s.verify_certification(&key.primary_key, pgp::types::Tag::UserId, &u.id)
                        .is_ok()
                })
                .collect();
            newest(valid.into_iter()).is_some_and(|s| !is_revocation(s) && binding_hash_ok(s))
        })
        .filter_map(|u| u.id.as_str().map(str::to_string))
        .collect()
}

/// The key's verified self-signatures: direct-key signatures and user-ID
/// self-certifications the primary key actually made. Expiry and feature flags
/// must be read only from these, never from a signature a third party attached:
/// an unverified signature could otherwise move the expiry or turn
/// on SEIPDv2 for a key that can't read it.
fn self_signatures(key: &SignedPublicKey) -> impl Iterator<Item = &Signature> {
    let direct = key
        .details
        .direct_signatures
        .iter()
        .filter(move |s| s.verify_key(&key.primary_key).is_ok());
    let certs = key.details.users.iter().flat_map(move |u| {
        u.signatures.iter().filter(move |s| {
            s.verify_certification(&key.primary_key, pgp::types::Tag::UserId, &u.id)
                .is_ok()
        })
    });
    direct.chain(certs).filter(|s| !is_revocation(s))
}

/// Whether the key's primary is flagged to make data signatures, read from the
/// newest verified self-signature that carries key flags. A self-signature
/// without a key-flags subpacket (a later direct-key signature naming a
/// revoker, say) says nothing about them, and a key with no key flags at all
/// can do what its algorithm can, as GnuPG reads it. Meron signs with the
/// primary key, so a key whose primary is flagged not to sign (an
/// offline-primary + signing-subkey setup) would produce signatures strict
/// verifiers reject; such a key is refused on import rather than used to sign
/// silently-invalid mail.
pub fn primary_can_sign(key: &SignedPublicKey) -> bool {
    match newest(self_signatures(key).filter(|s| has_key_flags(s))) {
        Some(sig) => sig.key_flags().sign(),
        None => key.primary_key.algorithm().can_sign(),
    }
}

/// Whether `sig` carries a key-flags subpacket (`Signature::key_flags` reads an
/// absent one as "no capabilities", which isn't what absence means).
fn has_key_flags(sig: &Signature) -> bool {
    sig.config().is_some_and(|c| {
        c.hashed_subpackets()
            .any(|p| matches!(p.data, pgp::packet::SubpacketData::KeyFlags(_)))
    })
}

/// When the primary key expires, from its newest valid user-ID binding
/// (or direct-key signature), as an absolute timestamp. A later self-signature
/// that only adds something else (a designated revoker, say: no key flags)
/// doesn't speak to the expiry, as for [`primary_can_sign`].
fn primary_expiry(key: &SignedPublicKey) -> Option<i64> {
    let created = i64::from(key.primary_key.created_at().as_secs());
    let sig = newest(self_signatures(key).filter(|s| has_key_flags(s)))
        .or_else(|| newest(self_signatures(key)))?;
    sig.key_expiration_time()
        .filter(|d| d.as_secs() != 0)
        .map(|d| created + i64::from(d.as_secs()))
}

/// Looks up public keys by primary fingerprint (designated revokers).
pub type Resolver<'a> = &'a dyn Fn(&str) -> Option<SignedPublicKey>;

/// The default resolver: the user's own and stored peer keys.
pub fn stored_key(fingerprint: &str) -> Option<SignedPublicKey> {
    super::keyring::current().key_by_fingerprint(fingerprint)
}

/// Fingerprints `key` names as designated revokers (RFC 4880 §5.2.3.15), read
/// only from its valid self-signatures (direct-key signatures and user-ID
/// self-certifications), so nobody else can add a revoker.
pub fn designated_revokers(key: &SignedPublicKey) -> Vec<String> {
    let direct = key
        .details
        .direct_signatures
        .iter()
        .filter(|s| s.verify_key(&key.primary_key).is_ok());
    let certs = key.details.users.iter().flat_map(|u| {
        u.signatures.iter().filter(move |s| {
            s.verify_certification(&key.primary_key, pgp::types::Tag::UserId, &u.id)
                .is_ok()
        })
    });
    let mut out: Vec<String> = direct
        .chain(certs)
        .filter_map(|s| s.revocation_key())
        .map(|r| hex_upper(&r.fingerprint))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Whether `rev` is a genuine revocation of `key`: made by the key itself, or
/// a key revocation made by one of its designated revokers whose key we have.
fn revocation_valid(key: &SignedPublicKey, rev: &Signature, resolve: Resolver<'_>) -> bool {
    if rev.verify_key(&key.primary_key).is_ok() {
        return true;
    }
    if rev.typ() != Some(pgp::packet::SignatureType::KeyRevocation) {
        return false;
    }
    designated_revokers(key).iter().any(|fpr| {
        resolve(fpr).is_some_and(|revoker| {
            keys_match(&revoker, fpr)
                && rev
                    .verify_key_third_party(&key.primary_key, &revoker.primary_key)
                    .is_ok()
        })
    })
}

fn keys_match(key: &SignedPublicKey, fingerprint: &str) -> bool {
    fingerprint_hex(&key.primary_key) == fingerprint
}

fn is_revoked_with(key: &SignedPublicKey, resolve: Resolver<'_>) -> bool {
    key.details
        .revocation_signatures
        .iter()
        .any(|s| revocation_valid(key, s, resolve))
}

/// Why a key could not make a valid signature at a given time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SigningProblem {
    Revoked,
    Expired,
}

/// Whether a revocation also voids signatures made before it. RFC 9580
/// §5.2.3.31: a key revoked as superseded or retired was fine until then;
/// compromised, unspecified or unknown reasons void every signature.
fn revocation_applies_at(rev: &Signature, when: u32) -> bool {
    use pgp::packet::RevocationCode;
    let soft = matches!(
        rev.revocation_reason_code(),
        Some(RevocationCode::KeySuperseded) | Some(RevocationCode::KeyRetired)
    );
    match (soft, rev.created()) {
        (true, Some(at)) => when >= at.as_secs(),
        _ => true,
    }
}

/// Whether `key`'s primary could make a valid signature at `when` (the
/// signature's creation time): not revoked then, not expired then.
pub fn primary_valid_at(key: &SignedPublicKey, when: u32) -> Result<(), SigningProblem> {
    primary_valid_at_with(key, when, &stored_key)
}

pub fn primary_valid_at_with(
    key: &SignedPublicKey,
    when: u32,
    resolve: Resolver<'_>,
) -> Result<(), SigningProblem> {
    let revoked = key
        .details
        .revocation_signatures
        .iter()
        .filter(|s| revocation_valid(key, s, resolve))
        .any(|s| revocation_applies_at(s, when));
    if revoked {
        return Err(SigningProblem::Revoked);
    }
    if primary_expiry(key).is_some_and(|e| i64::from(when) > e) {
        return Err(SigningProblem::Expired);
    }
    Ok(())
}

/// The same for a signing subkey: its primary must be valid at `when`, the
/// subkey not revoked then, and its newest binding not expired then.
pub fn subkey_valid_at(
    key: &SignedPublicKey,
    sub: &SignedPublicSubKey,
    when: u32,
) -> Result<(), SigningProblem> {
    subkey_valid_at_with(key, sub, when, &stored_key)
}

pub fn subkey_valid_at_with(
    key: &SignedPublicKey,
    sub: &SignedPublicSubKey,
    when: u32,
    resolve: Resolver<'_>,
) -> Result<(), SigningProblem> {
    primary_valid_at_with(key, when, resolve)?;
    let valid: Vec<&Signature> = sub
        .signatures
        .iter()
        .filter(|s| s.verify_subkey_binding(&key.primary_key, &sub.key).is_ok())
        .collect();
    let revoked = valid
        .iter()
        .filter(|s| s.typ() == Some(pgp::packet::SignatureType::SubkeyRevocation))
        .any(|s| revocation_applies_at(s, when));
    if revoked {
        return Err(SigningProblem::Revoked);
    }
    let binding = newest(
        valid
            .iter()
            .copied()
            .filter(|s| s.typ() != Some(pgp::packet::SignatureType::SubkeyRevocation)),
    );
    let expires = binding
        .and_then(|b| b.key_expiration_time())
        .filter(|d| d.as_secs() != 0)
        .map(|d| sub.key.created_at().as_secs().saturating_add(d.as_secs()));
    if expires.is_some_and(|e| when > e) {
        return Err(SigningProblem::Expired);
    }
    Ok(())
}

/// Most signatures kept per component when merging, so a hostile key can't
/// grow a stored one without bound.
const MAX_SIGNATURES: usize = 64;

/// Revocations kept for a designated revoker whose key Meron doesn't have yet
/// (so they can't be verified). Capped separately, so forgeries naming the
/// revoker can't fill [`MAX_SIGNATURES`] and crowd out verified revocations.
const MAX_PENDING_REVOCATIONS: usize = 4;

fn add_signatures(
    into: &mut Vec<Signature>,
    from: &[Signature],
    valid: impl Fn(&Signature) -> bool,
) -> bool {
    let mut changed = false;
    for sig in from {
        if into.len() >= MAX_SIGNATURES {
            break;
        }
        if !into.contains(sig) && valid(sig) {
            into.push(sig.clone());
            changed = true;
        }
    }
    changed
}

/// A copy of `key` carrying only what verifies against its own primary key:
/// self direct-key signatures, self/designated revocations, user-ID
/// self-certifications and subkey bindings (each capped at `MAX_SIGNATURES`);
/// user IDs and subkeys with no valid self-signature are dropped. A third
/// party's junk signatures attached to a genuine key are stripped before it is
/// stored, where they could otherwise be read as the key's expiry or feature
/// flags. Implemented by merging `key` into a bare copy of its
/// primary, so it shares [`merge`]'s verification exactly.
pub fn minimize(key: &SignedPublicKey) -> SignedPublicKey {
    minimize_with(key, &stored_key)
}

/// [`minimize`] with an explicit resolver for designated revokers.
pub fn minimize_with(key: &SignedPublicKey, resolve: Resolver<'_>) -> SignedPublicKey {
    let mut bare = key.clone();
    bare.details.direct_signatures.clear();
    bare.details.revocation_signatures.clear();
    bare.details.users.clear();
    bare.public_subkeys.clear();
    merge_with(&bare, key, resolve).unwrap_or(bare)
}

/// Merges a newer copy of the same key (same primary fingerprint) into the
/// stored one: revocations, direct-key signatures, user ID certifications and
/// subkey bindings it adds, and user IDs and subkeys that are new. Only
/// signatures that verify against the primary key are taken, so nobody but the
/// key's owner can change it. Returns `None` when nothing changed or the keys
/// differ. This is how a revocation published later takes effect.
pub fn merge(stored: &SignedPublicKey, incoming: &SignedPublicKey) -> Option<SignedPublicKey> {
    merge_with(stored, incoming, &stored_key)
}

/// [`merge`] with an explicit resolver for designated revokers.
pub fn merge_with(
    stored: &SignedPublicKey,
    incoming: &SignedPublicKey,
    resolve: Resolver<'_>,
) -> Option<SignedPublicKey> {
    if fingerprint_hex(&stored.primary_key) != fingerprint_hex(&incoming.primary_key) {
        return None;
    }
    let primary = &stored.primary_key;
    let mut out = stored.clone();
    let mut changed = false;
    changed |= add_signatures(
        &mut out.details.direct_signatures,
        &incoming.details.direct_signatures,
        |s| s.verify_key(primary).is_ok(),
    );
    for user in &incoming.details.users {
        let certifies = |s: &Signature| {
            s.verify_certification(primary, pgp::types::Tag::UserId, &user.id)
                .is_ok()
        };
        match out.details.users.iter_mut().find(|u| u.id == user.id) {
            Some(existing) => {
                changed |= add_signatures(&mut existing.signatures, &user.signatures, certifies)
            }
            None => {
                let sigs: Vec<Signature> = user
                    .signatures
                    .iter()
                    .filter(|s| certifies(s))
                    .take(MAX_SIGNATURES)
                    .cloned()
                    .collect();
                if !sigs.is_empty() {
                    out.details
                        .users
                        .push(pgp::types::SignedUser::new(user.id.clone(), sigs));
                    changed = true;
                }
            }
        }
    }
    for sub in &incoming.public_subkeys {
        let binds = |s: &Signature| s.verify_subkey_binding(primary, &sub.key).is_ok();
        let fpr = fingerprint_hex(&sub.key);
        match out
            .public_subkeys
            .iter_mut()
            .find(|s| fingerprint_hex(&s.key) == fpr)
        {
            Some(existing) => {
                changed |= add_signatures(&mut existing.signatures, &sub.signatures, binds)
            }
            None => {
                let sigs: Vec<Signature> = sub
                    .signatures
                    .iter()
                    .filter(|s| binds(s))
                    .take(MAX_SIGNATURES)
                    .cloned()
                    .collect();
                if !sigs.is_empty() {
                    out.public_subkeys
                        .push(SignedPublicSubKey::new(sub.key.clone(), sigs));
                    changed = true;
                }
            }
        }
    }
    // Revocations last: designated revokers come from the (merged) self-signatures.
    // A revocation by a named revoker whose key we don't have yet can't be
    // checked now; it is kept (at most MAX_PENDING_REVOCATIONS of them) and
    // counts once that key is known, since `is_revoked_with` verifies on use.
    let revokers = designated_revokers(&out);
    let mut pending = out
        .details
        .revocation_signatures
        .iter()
        .filter(|s| !revocation_valid(&out, s, resolve))
        .count();
    for sig in &incoming.details.revocation_signatures {
        if out.details.revocation_signatures.len() >= MAX_SIGNATURES {
            break;
        }
        if out.details.revocation_signatures.contains(sig) {
            continue;
        }
        if revocation_valid(&out, sig, resolve) {
            out.details.revocation_signatures.push(sig.clone());
            changed = true;
        } else if pending < MAX_PENDING_REVOCATIONS && pending_revocation(sig, &revokers, resolve) {
            out.details.revocation_signatures.push(sig.clone());
            pending += 1;
            changed = true;
        }
    }
    changed.then_some(out)
}

/// A key revocation whose issuer is one of `revokers` but whose key `resolve`
/// can't provide yet, so it can't be verified now. Once the revoker's key is
/// known it is verified like any other revocation; a forgery then never counts.
fn pending_revocation(sig: &Signature, revokers: &[String], resolve: Resolver<'_>) -> bool {
    if sig.typ() != Some(pgp::packet::SignatureType::KeyRevocation) {
        return false;
    }
    let fingerprints: Vec<String> = sig
        .issuer_fingerprint()
        .iter()
        .map(|f| hex_upper(f.as_bytes()))
        .collect();
    let key_ids: Vec<String> = sig
        .issuer_key_id()
        .iter()
        .map(|id| hex_upper(id.as_ref()))
        .collect();
    revokers.iter().any(|r| {
        (fingerprints.contains(r) || key_ids.iter().any(|id| r.ends_with(id.as_str())))
            && resolve(r).is_none()
    })
}

/// The subkey to encrypt to now, chosen like Delta Chat and the Autocrypt v2
/// draft: valid binding, encrypt flag, not expired, not revoked; of several,
/// the one expiring soonest.
pub fn encryption_subkey(key: &SignedPublicKey, now: u32) -> Option<&SignedPublicSubKey> {
    key.public_subkeys
        .iter()
        .filter(|sub| sub.key.algorithm().can_encrypt())
        .filter_map(|sub| {
            let valid: Vec<&Signature> = sub
                .signatures
                .iter()
                .filter(|s| s.verify_subkey_binding(&key.primary_key, &sub.key).is_ok())
                .collect();
            // Any revocation of the subkey, not just the newest signature:
            // a later binding (or one made the same second) can't undo it.
            if valid.iter().any(|s| is_revocation(s)) {
                return None;
            }
            let binding = newest(valid.iter().copied())?;
            if !binding_hash_ok(binding) {
                return None;
            }
            let flags = binding.key_flags();
            if !(flags.encrypt_comms() || flags.encrypt_storage()) {
                return None;
            }
            let expires = binding
                .key_expiration_time()
                .filter(|d| d.as_secs() != 0)
                .map(|d| sub.key.created_at().as_secs().saturating_add(d.as_secs()));
            if expires.is_some_and(|e| now > e) {
                return None;
            }
            Some((sub, expires.unwrap_or(u32::MAX)))
        })
        .min_by_key(|(_, exp)| *exp)
        .map(|(sub, _)| sub)
}

/// Whether the key's features advertise SEIPDv2 (RFC 9580 AEAD).
pub fn supports_seipd_v2(key: &SignedPublicKey) -> bool {
    self_signatures(key)
        .filter_map(|s| s.features())
        .any(|f| f.seipd_v2())
}

/// Checks a public key against Meron's policy at `now`.
pub fn check(key: &SignedPublicKey, now: u32) -> Option<Unusable> {
    check_with(key, now, &stored_key)
}

/// [`check`] with an explicit resolver for designated revokers.
pub fn check_with(key: &SignedPublicKey, now: u32, resolve: Resolver<'_>) -> Option<Unusable> {
    if is_revoked_with(key, resolve) {
        return Some(Unusable::Revoked);
    }
    if valid_user_ids(key).is_empty() {
        return Some(Unusable::InvalidBindings);
    }
    if primary_expiry(key).is_some_and(|e| i64::from(now) > e) {
        return Some(Unusable::Expired);
    }
    if encryption_subkey(key, now).is_none() {
        return Some(Unusable::NoEncryptionKey);
    }
    None
}

/// Describes a public key.
pub fn info(key: &SignedPublicKey, has_secret: bool, now: u32) -> KeyInfo {
    let user_ids = valid_user_ids(key);
    let mut emails: Vec<String> = user_ids
        .iter()
        .filter_map(|u| email_of_user_id(u))
        .collect();
    emails.sort();
    emails.dedup();
    KeyInfo {
        fingerprint: fingerprint_hex(&key.primary_key),
        user_ids,
        emails,
        created: i64::from(key.primary_key.created_at().as_secs()),
        expires: primary_expiry(key),
        algorithm: format!("{:?}", key.primary_key.algorithm()),
        has_secret,
        can_encrypt: encryption_subkey(key, now).is_some(),
        supports_seipd_v2: supports_seipd_v2(key),
        unusable: check(key, now),
    }
}

/// When the key last signed itself: its newest verified user-ID or direct-key
/// self-signature or subkey binding (a renewal or a new subkey makes one).
fn newest_self_signature_at(key: &SignedPublicKey) -> u32 {
    let bindings = key.public_subkeys.iter().flat_map(move |sub| {
        sub.signatures
            .iter()
            .filter(move |s| s.verify_subkey_binding(&key.primary_key, &sub.key).is_ok())
    });
    self_signatures(key)
        .chain(bindings)
        .filter_map(|s| s.created().map(|t| t.as_secs()))
        .max()
        .unwrap_or(0)
}

/// Whether `incoming`, an import of an own key already stored as `stored`
/// (same primary), should replace it: it keeps every stored subkey (so mail
/// encrypted to one stays readable) and is at least as recent (so a renewal or
/// a new subkey is taken, and an older export never rolls one back).
pub fn secret_supersedes(incoming: &SignedSecretKey, stored: &SignedSecretKey) -> bool {
    let has = |key: &SignedSecretKey, fpr: &str| {
        key.secret_subkeys
            .iter()
            .any(|sub| fingerprint_hex(&sub.key) == fpr)
    };
    let keeps_subkeys = stored
        .secret_subkeys
        .iter()
        .all(|sub| has(incoming, &fingerprint_hex(&sub.key)));
    keeps_subkeys
        && newest_self_signature_at(&incoming.to_public_key())
            >= newest_self_signature_at(&stored.to_public_key())
}

/// Describes a secret key (from its public half).
pub fn secret_info(key: &SignedSecretKey, now: u32) -> KeyInfo {
    let mut info = info(&key.to_public_key(), true, now);
    if is_stub(key) {
        info.unusable = Some(Unusable::SecretOnCard);
    }
    info
}

#[cfg(test)]
pub(crate) mod tests {
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
        let Unlock::Ok(key) = unlock(parsed.secret.into_iter().next().unwrap(), None).unwrap()
        else {
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
    pub(crate) fn revoke_by(
        revoker: &SignedSecretKey,
        target: &SignedPublicKey,
    ) -> SignedPublicKey {
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
        let mut config =
            SignatureConfig::v4(typ, key.primary_key.algorithm(), HashAlgorithm::Sha256);
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
        public.details.direct_signatures.push(self_sig(
            &key,
            SignatureType::Key,
            now() + 20,
            vec![],
        ));
        assert_eq!(primary_expiry(&public), Some(i64::from(created) + 1000));
    }

    /// `key` with a new self-certification of its first user ID made at `at`:
    /// what a renewal (`gpg --quick-set-expire`) adds.
    pub(crate) fn self_certified_at(key: &SignedSecretKey, at: u32) -> SignedSecretKey {
        use pgp::packet::{SignatureConfig, SignatureType, Subpacket, SubpacketData};
        let mut config = SignatureConfig::v4(
            SignatureType::CertPositive,
            key.primary_key.algorithm(),
            HashAlgorithm::Sha256,
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
}
