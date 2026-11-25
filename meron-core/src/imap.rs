//! IMAP read path: TLS connect + login (async-imap over tokio-rustls), folder
//! listing, recent-message envelopes, and full message reads.
//!
//! `connect` performs a fresh TLS + auth handshake. The request path does not
//! call it directly per operation — `Engine::with_session` (in `main.rs`) pools
//! warm authenticated sessions and reuses them, falling back to `connect` only
//! when no live session is available. IMAP IDLE watchers hold their own
//! dedicated long-lived connections, separate from that pool.

pub mod idle;

use anyhow::{Context, Result, anyhow};
use futures::StreamExt;
use mailparse::MailHeaderMap;
use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;

use crate::parse;
use crate::search_query::{SearchQuery, close_open_terms};

mod connect;
mod fetch;
mod flags;
mod folders;
mod mutate;
mod search;
pub use connect::*;
pub use fetch::*;
pub use flags::*;
pub use folders::*;
pub use mutate::*;
pub use search::*;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod append_tests;

#[cfg(test)]
mod mcp_mutation_tests;
