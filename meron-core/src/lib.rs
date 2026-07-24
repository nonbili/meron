//! Shared Meron core library surface.
//!
//! The desktop app still runs `src/main.rs` as a JSON-lines stdio sidecar. This
//! library is the first extraction point for mobile hosts: protocol constants
//! and wire types live here so desktop, Android, and future FFI bindings share
//! one source of truth.

pub mod changelog;
pub mod engine;
pub mod ffi;
pub mod imap;
pub mod log;
pub mod mail_model;
pub mod parse;
pub mod protocol;
pub mod rss;
pub mod secrets;
#[cfg(target_os = "linux")]
mod secrets_portal;
pub mod smtp;
pub mod store;
pub mod thread_read;
pub mod unified;
