use super::*;
use sha2::{Digest, Sha256};

const CHECKSUMS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/store/db/migrations/checksums.txt"
);

fn sha256_hex(text: &str) -> String {
    // Line endings are a checkout setting, not part of the step.
    let digest = Sha256::digest(text.replace("\r\n", "\n").as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn checksum_line(migration: &Migration) -> String {
    format!(
        "{:04} {} {}",
        migration.version,
        sha256_hex(migration.source),
        migration.name
    )
}

#[test]
fn versions_run_from_one_without_gaps() {
    for (index, migration) in MIGRATIONS.iter().enumerate() {
        assert_eq!(
            migration.version,
            index as i64 + 1,
            "{} is out of sequence: two branches may have taken the same number",
            migration.name
        );
    }
    assert_eq!(SCHEMA_VERSION, MIGRATIONS.len() as i64);
}

#[test]
fn file_names_carry_their_version() {
    for migration in MIGRATIONS {
        let number = migration.name.trim_start_matches('m');
        assert!(
            number.starts_with(&format!("{:04}_", migration.version)),
            "{} should start with {:04}_",
            migration.name,
            migration.version
        );
    }
}

/// Shipped steps never change. `MERON_BLESS=1` records steps that have no
/// checksum yet; it never rewrites one that has.
#[test]
fn shipped_migrations_are_unchanged() {
    let recorded = std::fs::read_to_string(CHECKSUMS)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    let recorded: Vec<&str> = recorded.lines().filter(|line| !line.is_empty()).collect();
    assert!(
        recorded.len() <= MIGRATIONS.len(),
        "checksums.txt lists {} migrations, but only {} exist: a shipped step was removed",
        recorded.len(),
        MIGRATIONS.len()
    );
    for (line, migration) in recorded.iter().zip(MIGRATIONS) {
        assert_eq!(
            *line,
            checksum_line(migration),
            "migration {} ({}) changed after it was recorded. Shipped steps are frozen: \
             revert the edit and add a new step instead",
            migration.version,
            migration.name
        );
    }
    let missing = &MIGRATIONS[recorded.len()..];
    if missing.is_empty() {
        return;
    }
    if std::env::var_os("MERON_BLESS").is_some() {
        let mut text = recorded.join("\n");
        for migration in missing {
            text.push('\n');
            text.push_str(&checksum_line(migration));
        }
        text.push('\n');
        std::fs::write(CHECKSUMS, text.trim_start()).unwrap();
        return;
    }
    panic!(
        "no checksum for migration {}: run `MERON_BLESS=1 cargo test store::db` to record it",
        missing[0].version
    );
}

#[test]
fn migrate_to_stops_at_the_target() {
    let conn = Connection::open_in_memory().unwrap();
    migrate_to(&conn, 8).unwrap();
    assert_eq!(user_version(&conn).unwrap(), 8);
    run_migrations(&conn).unwrap();
    assert_eq!(user_version(&conn).unwrap(), SCHEMA_VERSION);
}

/// A step that fails rolls back every step before it in the same upgrade,
/// writes and `user_version` alike.
#[test]
fn a_failing_step_rolls_back_the_whole_upgrade() {
    let conn = Connection::open_in_memory().unwrap();
    migrate_to(&conn, 8).unwrap();
    conn.execute(
        "INSERT INTO messages(account, folder, msg_id, uid, json)
         VALUES ('acc', 'INBOX', '1', 1, '{\"message_id\":\"<m@example.test>\"}')",
        [],
    )
    .unwrap();
    // v9 backfills observed identities and v10 adds tasks, then v11 trips
    // over a column that is already there.
    conn.execute_batch("ALTER TABLE messages ADD COLUMN files TEXT;")
        .unwrap();
    let error = run_migrations(&conn).unwrap_err();
    assert!(format!("{error:#}").contains("migration 11"), "{error:#}");
    assert_eq!(user_version(&conn).unwrap(), 8);
    let tasks: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name = 'task_lists'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(tasks, 0, "v10's table should have rolled back");
    let observed: i64 = conn
        .query_row("SELECT count(*) FROM observed_mail_identities", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(observed, 0, "v9's backfill should have rolled back");
}

#[test]
fn a_newer_store_is_refused_untouched() {
    let conn = Connection::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();
    conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .unwrap();
    let error = run_migrations(&conn).unwrap_err();
    assert_eq!(
        error.downcast_ref::<NewerSchema>(),
        Some(&NewerSchema {
            found: SCHEMA_VERSION + 1,
            supported: SCHEMA_VERSION
        })
    );
    assert_eq!(user_version(&conn).unwrap(), SCHEMA_VERSION + 1);
}
