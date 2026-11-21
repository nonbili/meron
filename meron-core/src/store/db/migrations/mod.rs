//! Schema migrations, applied in order and tracked by SQLite's
//! `PRAGMA user_version`.
//!
//! To change the schema, add a step at the end of [`MIGRATIONS`]: a
//! `NNNN_name.sql` file for plain SQL, or an `mNNNN_name.rs` module with a
//! `run` function when it needs Rust. Never edit, reorder or remove a step
//! that has shipped — `checksums.txt` and the tests in this module hold that
//! line. Steps don't set `user_version`; the runner does. Then run
//! `MERON_BLESS=1 cargo test store::db` to record the new step's checksum and
//! the new schema snapshot (`../testdata/schema.sql`), and review the diff.
//!
//! A step must not depend on code that can change later (shared DDL
//! constants, query helpers, formatters): copy what it needs, so replaying it
//! on an old store does what it did when it shipped.
//!
//! The local-folder archive runs these too, and there the `messages` rows are
//! not a cache: a step may rebuild them, but never drop cached mail on the
//! assumption that a sync will bring it back.

use anyhow::{Context, Result};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use std::time::Instant;

// Frozen with their checksums: a formatter change must not rewrite them.
#[rustfmt::skip]
mod m0006_recipients;
#[rustfmt::skip]
mod m0030_caldav;
#[rustfmt::skip]
mod m0032_carddav_notes;

#[cfg(test)]
mod tests;

pub(crate) enum Step {
    Sql(&'static str),
    Rust(fn(&Connection) -> Result<()>),
}

pub(crate) struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub step: Step,
    /// What `checksums.txt` pins: the SQL file, or the Rust module's source.
    #[cfg_attr(not(test), allow(dead_code))]
    pub source: &'static str,
}

macro_rules! sql {
    ($version:literal, $name:literal) => {
        Migration {
            version: $version,
            name: $name,
            step: Step::Sql(include_str!(concat!($name, ".sql"))),
            source: include_str!(concat!($name, ".sql")),
        }
    };
}

macro_rules! rust {
    ($version:literal, $module:ident) => {
        Migration {
            version: $version,
            name: stringify!($module),
            step: Step::Rust($module::run),
            source: include_str!(concat!(stringify!($module), ".rs")),
        }
    };
}

pub(crate) const MIGRATIONS: &[Migration] = &[
    sql!(1, "0001_initial"),
    sql!(2, "0002_subscription_json"),
    sql!(3, "0003_account_secrets"),
    sql!(4, "0004_folder_special_use"),
    sql!(5, "0005_observed_mail_identities"),
    rust!(6, m0006_recipients),
    sql!(7, "0007_mail_search_hits"),
    sql!(8, "0008_thread_key_indexes"),
    sql!(9, "0009_observe_identities_on_write"),
    sql!(10, "0010_tasks"),
    sql!(11, "0011_message_files"),
    sql!(12, "0012_mail_search_snapshots"),
    sql!(13, "0013_uncached_unseen"),
    sql!(14, "0014_removed_message_uids"),
    sql!(15, "0015_active_message_syncs"),
    sql!(16, "0016_folder_notify"),
    sql!(17, "0017_contacts"),
    sql!(18, "0018_invite_responses"),
    sql!(19, "0019_events"),
    sql!(20, "0020_reminder_deliveries"),
    sql!(21, "0021_event_alerts"),
    sql!(22, "0022_tags_and_rules"),
    sql!(23, "0023_contact_last_date"),
    sql!(24, "0024_contacts_by_folder"),
    sql!(25, "0025_rule_events"),
    sql!(26, "0026_rule_event_sequence"),
    sql!(27, "0027_rule_evaluations"),
    sql!(28, "0028_rule_evaluations_inputs"),
    sql!(29, "0029_openpgp"),
    rust!(30, m0030_caldav),
    sql!(31, "0031_carddav"),
    rust!(32, m0032_carddav_notes),
    sql!(33, "0033_google_grants"),
    sql!(34, "0034_openpgp_sign_plain"),
];

/// The `user_version` [`run_migrations`] leaves behind.
pub(crate) const SCHEMA_VERSION: i64 = MIGRATIONS[MIGRATIONS.len() - 1].version;

pub(crate) fn user_version(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
}

/// The store was last opened by a newer Meron, whose schema this one doesn't
/// know. Returned before anything is written, so the store is left as it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewerSchema {
    pub found: i64,
    pub supported: i64,
}

impl std::fmt::Display for NewerSchema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "this data was last used by a newer version of Meron (schema {}; this version supports {})",
            self.found, self.supported
        )
    }
}

impl std::error::Error for NewerSchema {}

/// Bring the schema up to [`SCHEMA_VERSION`].
pub(crate) fn run_migrations(conn: &Connection) -> Result<()> {
    migrate_to(conn, SCHEMA_VERSION)
}

/// Apply the pending steps up to `target`, all inside one IMMEDIATE
/// transaction, so concurrent first-open callers serialize before reading the
/// version and a crash mid-upgrade rolls back cleanly, to re-run next launch.
/// A store newer than this build fails with [`NewerSchema`], untouched.
pub(crate) fn migrate_to(conn: &Connection, target: i64) -> Result<()> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let from = user_version(&tx)?;
    if from > SCHEMA_VERSION {
        return Err(NewerSchema {
            found: from,
            supported: SCHEMA_VERSION,
        }
        .into());
    }
    let pending = MIGRATIONS
        .iter()
        .filter(|migration| migration.version > from && migration.version <= target);
    let started = Instant::now();
    let mut applied = 0;
    for migration in pending {
        let step_started = Instant::now();
        match migration.step {
            Step::Sql(sql) => tx.execute_batch(sql).map_err(anyhow::Error::from),
            Step::Rust(run) => run(&tx),
        }
        .with_context(|| format!("migration {} ({})", migration.version, migration.name))?;
        tx.pragma_update(None, "user_version", migration.version)?;
        applied += 1;
        let elapsed = step_started.elapsed();
        if elapsed.as_millis() >= 100 {
            crate::mlog!(
                crate::log::Level::Info,
                "store",
                "migration {} ({}) took {elapsed:?}",
                migration.version,
                migration.name
            );
        }
    }
    tx.commit()?;
    if applied > 0 && from > 0 {
        crate::mlog!(
            crate::log::Level::Info,
            "store",
            "schema upgraded from {from} to {} in {:?}",
            user_version(conn)?,
            started.elapsed()
        );
    }
    Ok(())
}
