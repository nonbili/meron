use serde_json::{Value, json};
use std::sync::Arc;

use meron_core::engine::Engine;
use meron_core::protocol::Request;
use meron_core::{proxy, store};

use crate::Writer;
use crate::sidecar::params::*;

/// Ordering state for `app.prefsSet`: the client session whose writes count, and
/// the highest sequence that session has applied per settings key.
#[derive(Default)]
pub(crate) struct PrefWriteOrder {
    /// The session allowed to write, set by [`activate_pref_session`]. Empty
    /// until a client identifies itself (mobile never does).
    active: String,
    /// Highest applied sequence per settings key, within `active`.
    seq: std::collections::HashMap<String, u64>,
}

pub(crate) static PREF_WRITES: std::sync::LazyLock<std::sync::Mutex<PrefWriteOrder>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(PrefWriteOrder::default()));

/// Make `session` the one whose `app.prefsSet` writes are applied, retiring the
/// previous one and its sequences. Driven by `app.prefsGet`, which a client
/// issues once as it boots — including after a WebView reload, which restarts
/// its counters while this process keeps running.
///
/// The caller is the stdin read loop, so activations happen in the order the
/// requests arrived rather than in whatever order their tasks get scheduled: a
/// boot read that was already on the wire cannot retire the session that booted
/// after it.
pub(crate) fn activate_pref_session(session: &str) {
    if session.is_empty() {
        return;
    }
    let mut order = PREF_WRITES.lock().unwrap();
    if order.active == session {
        return;
    }
    order.active = session.to_string();
    order.seq.clear();
}

/// Whether an `app.prefsSet` write is still the newest for its key. Unstamped
/// writes (mobile, older clients) always apply. A stamped one applies only if it
/// comes from the active session — a straggler from the session before a reload
/// is dropped rather than overwriting what replaced it — and is newer than the
/// last sequence applied to that key.
///
/// Pairs with [`record_pref_write`], which the caller runs only once the write
/// has actually persisted, holding `order` (and the database lock) across both
/// so acceptance, the write, and the record are one ordering step.
pub(crate) fn pref_write_is_current(
    order: &PrefWriteOrder,
    key: &str,
    seq: Option<u64>,
    session: &str,
) -> bool {
    let Some(seq) = seq else { return true };
    // A client that stamps writes without ever announcing itself (an older
    // build) still gets per-key ordering: the first one seen holds authority.
    if !order.active.is_empty() && order.active != session {
        return false;
    }
    !matches!(order.seq.get(key), Some(last) if *last >= seq)
}

/// Record a write that persisted, so later stragglers for that key are dropped.
/// A write that failed is never recorded: retrying it must not look stale, since
/// its value never reached the database.
pub(crate) fn record_pref_write(
    order: &mut PrefWriteOrder,
    key: &str,
    seq: Option<u64>,
    session: &str,
) {
    let Some(seq) = seq else { return };
    if order.active != session {
        order.active = session.to_string();
        order.seq.clear();
    }
    order.seq.insert(key.to_string(), seq);
}

/// Handle the `app.prefs*` requests.
pub(crate) async fn dispatch(
    engine: &Arc<Engine>,
    req: &Request,
    out: &Writer,
) -> anyhow::Result<Value> {
    let p = &req.params;
    match req.method.as_str() {
        "app.prefsGet" => {
            let keys = p
                .get("keys")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let prefs = store::settings_get(&engine.db.lock().unwrap(), &keys)?;
            Ok(json!({ "prefs": prefs }))
        }

        "app.prefsSet" => {
            let key = req_str(p, "key")?;
            let value = p.get("value").cloned().unwrap_or(Value::Null);
            // Each request runs on its own task, so two writes to one key can
            // reach here out of order. A client that stamps its writes with a
            // rising `seq` per key gets last-writer-wins instead: a straggler
            // carrying an older stamp is dropped rather than resurrecting the
            // value the user just replaced.
            let seq = p.get("seq").and_then(Value::as_u64);
            let session = p.get("session").and_then(Value::as_str).unwrap_or_default();
            {
                // Both locks span the write, so a straggler that passed the
                // stamp check cannot slip its value in after the newer one's,
                // and a write that fails leaves the stamps as they were.
                let db = engine.db.lock().unwrap();
                let mut order = PREF_WRITES.lock().unwrap();
                if !pref_write_is_current(&order, &key, seq, session) {
                    return Ok(json!({ "ok": true, "stale": true }));
                }
                store::setting_set(&db, &key, &value)?;
                record_pref_write(&mut order, &key, seq, session);
                // The proxy lives in a process-global slot that socket code
                // reads without a DB handle, so republish it as soon as it
                // changes — inside this section, or a straggler could publish
                // its value after the newer one and leave new connections
                // using a proxy the database no longer holds.
                if key == proxy::SETTING_KEY {
                    proxy::set_global(proxy::parse_global(&value));
                }
            }
            // Tasks sync rests while Tasks is off: catch up when it is back.
            if key == "tasks_enabled" && value == Value::Bool(true) {
                crate::sidecar::dispatch::google_tasks::spawn_sync(engine.clone(), out.clone());
                crate::sidecar::dispatch::caldav::spawn_sync(engine.clone(), out.clone(), None);
            }
            Ok(json!({ "ok": true }))
        }

        other => Err(anyhow::anyhow!("unknown method: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::{PREF_WRITES, activate_pref_session, pref_write_is_current, record_pref_write};

    /// One write, as the handler does it: check, "persist", record. `stored`
    /// says whether the database write succeeded.
    fn write_pref(key: &str, seq: Option<u64>, session: &str, stored: bool) -> bool {
        let mut order = PREF_WRITES.lock().unwrap();
        if !pref_write_is_current(&order, key, seq, session) {
            return false;
        }
        if stored {
            record_pref_write(&mut order, key, seq, session);
        }
        true
    }

    fn apply_pref(key: &str, seq: Option<u64>, session: &str) -> bool {
        write_pref(key, seq, session, true)
    }

    #[test]
    fn pref_writes_are_ordered_per_session_and_key() {
        // One process-global ordering state, so this covers it in one test.
        activate_pref_session("s1");

        // Two writes to one key, applied in the order the client stamped them.
        assert!(apply_pref("remote_image_senders", Some(1), "s1"));
        assert!(apply_pref("remote_image_senders", Some(2), "s1"));
        // The straggler carrying the older stamp must not resurrect its value,
        // and a repeat of the newest stamp is not a newer write either.
        assert!(!apply_pref("remote_image_senders", Some(1), "s1"));
        assert!(!apply_pref("remote_image_senders", Some(2), "s1"));

        // Stamps are per key: another key starts its own comparison.
        assert!(apply_pref("signature", Some(1), "s1"));

        // Clients that do not stamp their writes (mobile) keep working.
        assert!(apply_pref("remote_image_senders", None, ""));
        assert!(apply_pref("theme_id", None, ""));

        // A WebView reload restarts the client's counters while this process
        // keeps running; its boot read retires the old session, so its writes
        // persist even though they start from 1 again.
        activate_pref_session("s2");
        assert!(apply_pref("remote_image_senders", Some(1), "s2"));
        assert!(apply_pref("remote_image_senders", Some(2), "s2"));
        // Ordering still holds inside the new session.
        assert!(!apply_pref("remote_image_senders", Some(1), "s2"));

        // old -> new -> old: a request left over from before the reload cannot
        // take authority back and overwrite what the live session wrote.
        assert!(!apply_pref("remote_image_senders", Some(3), "s1"));
        assert!(!apply_pref("theme_id", Some(1), "s1"));
        // Re-announcing the session already in charge keeps its sequences.
        activate_pref_session("s2");
        assert!(!apply_pref("remote_image_senders", Some(2), "s2"));
        assert!(apply_pref("remote_image_senders", Some(3), "s2"));

        // A write the database rejected records nothing, so retrying the same
        // request stores the value rather than being dropped as a straggler.
        assert!(write_pref("signature", Some(9), "s2", false));
        assert!(apply_pref("signature", Some(9), "s2"));
        assert!(!apply_pref("signature", Some(9), "s2"));
    }
}
