use serde_json::{Value, json};
use std::sync::Arc;

use meron_core::calendar::events;
use meron_core::engine::Engine;
use meron_core::protocol::Request;

use crate::sidecar::params::*;
use crate::{Writer, emit};

/// Handle the local calendar: `calendar.*`. Local SQLite only, like Tasks.
pub(crate) async fn dispatch(
    engine: &Arc<Engine>,
    req: &Request,
    out: &Writer,
) -> anyhow::Result<Value> {
    let p = &req.params;
    let result = match req.method.as_str() {
        "calendar.events" => {
            let from = p.get("from").and_then(Value::as_i64).unwrap_or(0);
            let to = p.get("to").and_then(Value::as_i64).unwrap_or(0);
            let include_tasks = p
                .get("include_tasks")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            return events::events_payload(&engine.db.lock().unwrap(), from, to, include_tasks);
        }

        "calendar.get" => {
            let event_id = req_str(p, "event_id")?;
            return events::event_payload(&engine.db.lock().unwrap(), &event_id);
        }

        "calendar.create" => events::create_event(
            &engine.db.lock().unwrap(),
            &events::EventInput::from_params(p),
        )?,

        "calendar.update" => {
            let event_id = req_str(p, "event_id")?;
            events::update_event(
                &engine.db.lock().unwrap(),
                &event_id,
                &events::EventInput::from_params(p),
            )?
        }

        "calendar.delete" => {
            let event_id = req_str(p, "event_id")?;
            events::delete_event(&engine.db.lock().unwrap(), &event_id)?
        }

        "invite.addToCalendar" => {
            let message = req_str(p, "message")?;
            let thread_id = req_str(p, "thread_id").unwrap_or_default();
            let event_id = meron_core::calendar::add_to_calendar(
                &engine.db.lock().unwrap(),
                &message,
                &thread_id,
                &meron_core::parse::media_root(),
            )?;
            json!({ "ok": true, "event_id": event_id })
        }

        "invite.removeFromCalendar" => {
            let message = req_str(p, "message")?;
            meron_core::calendar::remove_from_calendar(
                &engine.db.lock().unwrap(),
                &message,
                &meron_core::parse::media_root(),
            )?;
            json!({ "ok": true })
        }

        "calendar.restore" => {
            let restore = p.get("restore").cloned().unwrap_or(Value::Null);
            events::restore_event(&engine.db.lock().unwrap(), &restore)?
        }

        other => return Err(anyhow::anyhow!("unknown method: {other}")),
    };

    // Only mutations reach here; any surface showing the calendar re-reads.
    emit(out, "calendar.changed", json!({ "method": req.method })).await;
    // An edit to a synced event goes to its server straight away.
    let synced = [
        &result["event"]["calendar"],
        &result["restore"]["calendar"],
        &result["moved_from"],
    ]
    .iter()
    .any(|calendar| calendar.as_str().is_some_and(|c| !c.is_empty()));
    if synced {
        super::caldav::spawn_sync(engine.clone(), out.clone(), None);
    }
    Ok(result)
}
