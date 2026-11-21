//! Retrying work that lost a race for the database lock. `busy_timeout`
//! covers most contention, but not every case: a deferred transaction that
//! must upgrade to a writer under WAL fails at once rather than waiting.

use anyhow::Result;
use rusqlite::ErrorCode;
use std::time::{Duration, Instant};

pub(super) fn with_busy_retry<T>(
    mut f: impl FnMut() -> rusqlite::Result<T>,
) -> rusqlite::Result<T> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut delay = Duration::from_millis(10);
    loop {
        match f() {
            Err(err) if is_database_locked(&err) && Instant::now() < deadline => {
                std::thread::sleep(delay);
                delay = (delay * 2).min(Duration::from_millis(100));
            }
            result => return result,
        }
    }
}

pub(super) fn with_anyhow_busy_retry<T>(mut f: impl FnMut() -> Result<T>) -> Result<T> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut delay = Duration::from_millis(10);
    loop {
        match f() {
            Err(err) if anyhow_error_is_database_locked(&err) && Instant::now() < deadline => {
                std::thread::sleep(delay);
                delay = (delay * 2).min(Duration::from_millis(100));
            }
            result => return result,
        }
    }
}

fn anyhow_error_is_database_locked(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        cause
            .downcast_ref::<rusqlite::Error>()
            .is_some_and(is_database_locked)
    })
}

fn is_database_locked(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(failure, _)
            if matches!(
                failure.code,
                ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked
            )
    )
}
