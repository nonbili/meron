//! The stdio sidecar's request handling, split out of `main.rs`: request
//! dispatch per domain, background sync tasks, IMAP IDLE watches, and the
//! shared param helpers.

pub(crate) mod dispatch;
pub(crate) mod idle;
pub(crate) mod local;
pub(crate) mod params;
pub(crate) mod prefs;
pub(crate) mod spawn;

pub(crate) mod db;
