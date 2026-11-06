//! OpenPGP (PGP/MIME, Autocrypt) for Meron, on rPGP. See nonbili/meron#46.
//!
//! - [`keys`]: generate, parse, unlock and check keys (the policy rPGP leaves to us)
//! - [`crypt`]: decrypt, verify and encrypt messages, with size caps, fail closed
//! - [`autocrypt`]: the headers that carry keys in ordinary mail
//! - [`lookup`]: on-demand key lookup (WKD, keys.openpgp.org, HKP keyservers)
//! - [`keyring`]: the in-memory key snapshot the parser and sender use
//! - [`store`]: the SQLCipher tables behind it
//! - [`mime`]: PGP/MIME detection and the decrypted inner message
//! - [`outgoing`]: encrypted sending (RFC 3156 + RFC 9788 protected headers)

pub mod autocrypt;
pub mod crypt;
pub mod keyring;
pub mod keys;
pub mod lookup;
pub mod mime;
pub mod outgoing;
pub mod store;
