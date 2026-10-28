//! One reminder plan shared by desktop and native mobile notification schedulers.
//! Events alert at their own alerts (at most two) or the app-wide default:
//! timed events ten minutes before starting, all-day events at 09:00 local time
//! on the day, unless changed. Date-only tasks alert at 09:00. A selected task time is an instant, including explicit midnight.
use anyhow::Result;
use chrono::{Datelike, TimeZone};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{calendar, store};
const DAY: i64 = 86_400;
const LEAD: i64 = 600;
/// Settings row holding the default event alert, in minutes before start, or
/// [`NO_ALERT`]. Shared by desktop and mobile, since the core plans reminders
/// for both.
pub const LEAD_SETTING_KEY: &str = "reminder_lead_minutes";
/// Settings row holding the default all-day event alert, in days before the
/// event (at 09:00), or [`NO_ALERT`].
pub const ALL_DAY_SETTING_KEY: &str = "reminder_all_day_days";
/// The default alert setting that turns event alerts off.
pub const NO_ALERT: i64 = -1;
const START_KEY: &str = "__reminders_started__";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reminder {
    pub key: String,
    pub kind: String,
    pub item_id: String,
    pub title: String,
    pub body: String,
    pub at: i64,
    #[serde(skip)]
    eligible_after: i64,
    #[serde(skip)]
    expires_at: i64,
}

fn day_time(at: i64, utc_date: bool, hour: u32) -> Option<i64> {
    let tz = calendar::local_tz();
    let date = if utc_date {
        chrono::DateTime::from_timestamp(at, 0)?.date_naive()
    } else {
        tz.timestamp_opt(at, 0).single()?.date_naive()
    };
    tz.with_ymd_and_hms(date.year(), date.month(), date.day(), hour, 0, 0)
        .earliest()
        .map(|at| at.timestamp())
}

fn morning(at: i64, utc_date: bool) -> Option<i64> {
    day_time(at, utc_date, 9)
}

// A missing midnight at a DST boundary starts at the first valid local hour.
fn local_day_end(at: i64) -> Option<i64> {
    (0..24).find_map(|hour| day_time(at, true, hour))
}

fn started_at(conn: &Connection, now: i64) -> Result<i64> {
    conn.execute(
        "INSERT OR IGNORE INTO reminder_deliveries(key, delivered_at) VALUES(?1, ?2)",
        params![START_KEY, now],
    )?;
    Ok(conn.query_row(
        "SELECT delivered_at FROM reminder_deliveries WHERE key = ?1",
        params![START_KEY],
        |row| row.get(0),
    )?)
}

fn reset_start(conn: &Connection, now: i64) -> Result<()> {
    conn.execute(
        "INSERT INTO reminder_deliveries(key, delivered_at) VALUES(?1, ?2)
                  ON CONFLICT(key) DO UPDATE SET delivered_at = excluded.delivered_at",
        params![START_KEY, now],
    )?;
    Ok(())
}

/// The default alerts for events without their own, as (timed minutes,
/// all-day days): none, or one lead each. A missing or out-of-range setting
/// falls back to [LEAD] and the day itself.
fn default_alerts(conn: &Connection) -> Result<(Vec<i64>, Vec<i64>)> {
    let stored = store::settings_get(
        conn,
        &[
            LEAD_SETTING_KEY.to_string(),
            ALL_DAY_SETTING_KEY.to_string(),
        ],
    )?;
    let read = |key: &str, max: i64, fallback: i64| match stored[key].as_i64() {
        Some(NO_ALERT) => vec![],
        Some(lead) if (0..=max).contains(&lead) => vec![lead],
        _ => vec![fallback],
    };
    Ok((
        read(
            LEAD_SETTING_KEY,
            calendar::events::MAX_ALERT_MINUTES,
            LEAD / 60,
        ),
        read(
            ALL_DAY_SETTING_KEY,
            calendar::events::MAX_ALL_DAY_ALERT_DAYS,
            0,
        ),
    ))
}

/// When each alert of an all-day occurrence fires: 09:00 local time, the
/// given number of days before its (UTC calendar) date.
fn all_day_alert_times(start: i64, alerts: &[i64]) -> Vec<(usize, i64)> {
    let mut seen = std::collections::HashSet::new();
    alerts
        .iter()
        .enumerate()
        .filter_map(|(slot, days)| morning(start - days * DAY, true).map(|at| (slot, at)))
        .filter(|(_, at)| seen.insert(*at))
        .collect()
}

/// When each alert of a timed occurrence fires, keyed by its slot. An event
/// created or rescheduled after every alert's time alerts once, at its start.
fn alert_times(start: i64, changed: i64, alerts: &[i64]) -> Vec<(usize, i64)> {
    let times: Vec<(usize, i64)> = alerts
        .iter()
        .enumerate()
        .map(|(slot, minutes)| (slot, start - minutes * 60))
        .collect();
    if !times.is_empty() && changed < start && times.iter().all(|(_, at)| *at < changed) {
        return vec![(0, start)];
    }
    let mut seen = std::collections::HashSet::new();
    times
        .into_iter()
        .filter(|(_, at)| seen.insert(*at))
        .collect()
}

fn plan(conn: &Connection, from: i64, to: i64, now: i64) -> Result<Vec<Reminder>> {
    let started = started_at(conn, now)?;
    Ok(list(conn, from, to)?
        .into_iter()
        .filter(|reminder| {
            reminder.at >= started
                && reminder.at >= reminder.eligible_after
                && (reminder.expires_at == 0 || reminder.expires_at > now)
        })
        .collect())
}

pub fn list(conn: &Connection, from: i64, to: i64) -> Result<Vec<Reminder>> {
    let mut reminders = Vec::new();
    let (timed_defaults, all_day_defaults) = default_alerts(conn)?;
    let reach = calendar::events::MAX_ALERT_MINUTES * 60;
    let payload = calendar::events::events_payload(conn, from - DAY, to + DAY + reach, false)?;
    for event in payload["events"].as_array().into_iter().flatten() {
        if event["response"] == "declined" {
            continue;
        }
        let start = event["start"].as_i64().unwrap_or(0);
        let all_day = event["all_day"] == true;
        let created = event["created_at"].as_i64().unwrap_or(0);
        let changed = event["reminder_after"].as_i64().unwrap_or(created);
        // An occurrence the organizer made all-day (or timed) on its own cannot
        // use the series' alerts, which count in the other unit.
        let own: Option<Vec<i64>> = serde_json::from_value(event["alerts"].clone())
            .ok()
            .filter(|_| event["alerts_all_day"] == event["all_day"]);
        let times = if all_day {
            all_day_alert_times(start, own.as_deref().unwrap_or(&all_day_defaults))
        } else {
            alert_times(
                start,
                created.max(changed),
                own.as_deref().unwrap_or(&timed_defaults),
            )
        };
        let id = event["id"].as_str().unwrap_or_default();
        for (slot, at) in times {
            if at < from || at >= to {
                continue;
            }
            // Slot 0 keeps the key format receipts were first stored in.
            let occurrence = if slot == 0 {
                format!("event:{id}:{start}")
            } else {
                format!("event:{id}:{start}.{slot}")
            };
            reminders.push(Reminder {
                key: format!("{occurrence}:{at}"),
                kind: "calendar".into(),
                item_id: id.into(),
                title: event["title"].as_str().unwrap_or_default().into(),
                body: event["location"].as_str().unwrap_or_default().into(),
                at,
                eligible_after: created.max(changed),
                expires_at: if all_day {
                    local_day_end(event["end"].as_i64().unwrap_or(start + DAY))
                        .unwrap_or(start + DAY)
                } else {
                    event["end"].as_i64().unwrap_or(start).max(start + 1)
                },
            });
        }
    }
    for task in store::tasks_due_between(conn, from - DAY, to + DAY)? {
        let at = if task.due_has_time {
            Some(task.due_at)
        } else {
            morning(task.due_at, false)
        };
        let Some(at) = at.filter(|at| *at >= from && *at < to) else {
            continue;
        };
        reminders.push(Reminder {
            key: format!("task:{}:{at}", task.id),
            kind: "tasks".into(),
            item_id: task.id,
            title: task.title,
            body: String::new(),
            at,
            eligible_after: task.created_at.max(task.reminder_after),
            expires_at: 0,
        });
    }
    let mut unseen = Vec::new();
    let mut check =
        conn.prepare("SELECT EXISTS(SELECT 1 FROM reminder_deliveries WHERE key = ?1)")?;
    let mut occurrence_check = conn.prepare(
        "SELECT EXISTS(SELECT 1 FROM reminder_deliveries WHERE key >= ?1 AND key < ?2
            AND instr(substr(key, length(?1) + 1), ':') = 0)",
    )?;
    let mut keys = std::collections::HashSet::new();
    for reminder in reminders {
        if !keys.insert(reminder.key.clone())
            || check.query_row(params![reminder.key], |r| r.get::<_, bool>(0))?
        {
            continue;
        }
        // A lead-time change or timezone change can change an alert's trigger
        // without making it a new one. Match existing receipts by the event id
        // + occurrence start (+ alert slot), retaining the persisted key format.
        if reminder.kind == "calendar" {
            if let Some((occurrence, _)) = reminder.key.rsplit_once(':') {
                if occurrence_check.query_row(
                    params![format!("{occurrence}:"), format!("{occurrence};")],
                    |r| r.get::<_, bool>(0),
                )? {
                    continue;
                }
            }
        }
        unseen.push(reminder);
    }
    unseen.sort_by(|a, b| a.at.cmp(&b.at).then(a.key.cmp(&b.key)));
    Ok(unseen)
}

/// Claim only a currently valid, due reminder. Changes and completions between
/// planning and delivery cannot produce stale alerts; INSERT OR IGNORE also
/// serializes competing Android receiver / foreground refresh runs.
pub fn claim(conn: &Connection, key: &str, now: i64) -> Result<Option<Reminder>> {
    let tx = conn.unchecked_transaction()?;
    let reminder = plan(&tx, now - DAY, now + 1, now)?
        .into_iter()
        .find(|r| r.key == key);
    if let Some(ref reminder) = reminder {
        if tx.execute(
            "INSERT OR IGNORE INTO reminder_deliveries(key, delivered_at) VALUES(?1, ?2)",
            params![reminder.key, now],
        )? == 0
        {
            return Ok(None);
        }
    }
    tx.execute(
        "DELETE FROM reminder_deliveries WHERE delivered_at < ?1 AND key <> ?2",
        params![now - 60 * DAY, START_KEY],
    )?;
    tx.commit()?;
    Ok(reminder)
}

pub fn dispatch(conn: &Connection, method: &str, p: &Value) -> Result<Value> {
    let now = store::now_unix();
    if method == "reminders.skipPast" {
        reset_start(conn, now)?;
        return Ok(json!({"ok": true}));
    }
    if method == "reminders.acknowledge" {
        let keys: Vec<String> =
            serde_json::from_value(p.get("keys").cloned().unwrap_or(json!([])))?;
        let tx = conn.unchecked_transaction()?;
        for key in keys {
            anyhow::ensure!(
                key.starts_with("task:") || key.starts_with("event:"),
                "invalid reminder key"
            );
            tx.execute(
                "INSERT OR IGNORE INTO reminder_deliveries(key, delivered_at) VALUES(?1, ?2)",
                params![key, now],
            )?;
        }
        tx.execute(
            "DELETE FROM reminder_deliveries WHERE delivered_at < ?1 AND key <> ?2",
            params![now - 60 * DAY, START_KEY],
        )?;
        tx.commit()?;
        return Ok(json!({"ok": true}));
    }
    if method == "reminders.claim" {
        return Ok(json!({"reminder": claim(conn, p["key"].as_str().unwrap_or_default(), now)?}));
    }
    let from = if p["upcoming"] == true {
        now + 1
    } else {
        now - DAY
    };
    let to = if p["due_only"] == true {
        now + 1
    } else {
        now + 30 * DAY
    };
    let mut reminders = plan(conn, from, to, now)?;
    // Catch-up must not crowd future alarms out of a mobile scheduler's plan.
    let (mut due, mut future) = (0, 0);
    reminders.retain(|reminder| {
        let count = if reminder.at <= now {
            &mut due
        } else {
            &mut future
        };
        *count += 1;
        *count <= 64
    });
    Ok(json!({"reminders": reminders}))
}

#[cfg(test)]
mod tests;
