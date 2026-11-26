use anyhow::Result;
use calcard::icalendar::{ICalendarComponentType, ICalendarProperty, ICalendarValue};
use rusqlite::Connection;

use super::recurrence::{
    OCCURRENCE_RESPONSE, carry_exceptions, cut_from, excludes_occurrence, master_index, overrides,
    own_sequence, recurrence_instant, remove_overrides, settle_start, with_occurrence,
    with_series_sequence, without_occurrence,
};
use super::set_all_day;
use crate::calendar::{self, Invite, Response};
use crate::store::{self, CalendarEvent};

/// File the answer to an invitation in the calendar: an accepted or tentative
/// event is put there (or brought up to date), a declined one taken out.
pub fn record_invite_event(
    conn: &Connection,
    account: &str,
    thread_id: &str,
    message_id: &str,
    invite: &Invite,
    response: Response,
) -> Result<()> {
    let series = store::invite_event(conn, account, &invite.uid)?;
    if invite.recurrence_id != 0 {
        let alone =
            store::invite_occurrence_event(conn, account, &invite.uid, invite.recurrence_id)?;
        // An answer about one occurrence of a series already in the calendar
        // changes that occurrence, not the series.
        if let Some(mut series) = series.filter(|event| event.recurring && !event.ics.is_empty()) {
            series.ics = with_occurrence(&series.ics, invite, Some(response))?;
            settle_start(&mut series);
            store::save_event(conn, &series)?;
            if let Some(alone) = alone {
                store::delete_event(conn, &alone.id)?;
            }
            return Ok(());
        }
        // Otherwise the occurrence stands alone, beside any other occurrence
        // of the same event answered the same way.
        if response == Response::Declined {
            if let Some(alone) = alone {
                store::delete_event(conn, &alone.id)?;
            }
            // Declined or cancelled from that date on: the later ones too.
            if invite.this_and_future {
                for later in store::invite_occurrence_events(conn, account, &invite.uid)? {
                    if later.recurrence_id >= invite.recurrence_id {
                        store::delete_event(conn, &later.id)?;
                    }
                }
            }
            return Ok(());
        }
        file_invite(
            conn, alone, account, thread_id, message_id, invite, response,
        )?;
        return Ok(());
    }

    let alone = store::invite_occurrence_events(conn, account, &invite.uid)?;
    if response == Response::Declined {
        for event in series.into_iter().chain(alone) {
            store::delete_event(conn, &event.id)?;
        }
        return Ok(());
    }
    let mut series = file_invite(
        conn, series, account, thread_id, message_id, invite, response,
    )?;
    // Occurrences answered before the series fold into it: accepted ones as
    // its overrides, declined ones (which left nothing filed) as exclusions.
    if series.recurring && !series.ics.is_empty() {
        for (recurrence_id, this_and_future) in
            store::declined_occurrences(conn, account, &invite.uid)?
        {
            series.ics = if this_and_future {
                cut_from(&series.ics, recurrence_id)?
            } else {
                without_occurrence(&series.ics, recurrence_id, series.all_day)?
            };
        }
        for occurrence in alone {
            let answer = Response::parse(&occurrence.response).unwrap_or(Response::Accepted);
            let update = Invite {
                ics: occurrence.ics.clone(),
                recurrence_id: occurrence.recurrence_id,
                ..Default::default()
            };
            series.ics = with_occurrence(&series.ics, &update, Some(answer))?;
            // The series has one set of notes; the user's on this occurrence
            // join them rather than go with its row.
            let notes = occurrence.notes.trim();
            if !notes.is_empty() && !series.notes.contains(notes) {
                series.notes = if series.notes.is_empty() {
                    notes.to_string()
                } else {
                    format!("{}\n\n{notes}", series.notes)
                };
            }
            store::delete_event(conn, &occurrence.id)?;
        }
        settle_start(&mut series);
        store::save_event(conn, &series)?;
    }
    Ok(())
}

/// Bring a filed invitation up to date with the organizer's newer revision of
/// it (a later SEQUENCE: moved, renamed, ...), as soon as that message is read.
/// The calendar would otherwise keep the old time until the user answered
/// again. The answer they gave carries over; the card asks for a new one.
/// `sender` is the message's From address.
pub fn apply_invite_update(
    conn: &Connection,
    account: &str,
    sender: &str,
    message_id: &str,
    invite: &Invite,
) -> Result<()> {
    if invite.method != "request" || invite.cancelled || invite.uid.is_empty() {
        return Ok(());
    }
    let alone = if invite.recurrence_id == 0 {
        None
    } else {
        store::invite_occurrence_event(conn, account, &invite.uid, invite.recurrence_id)?
    };
    let Some(filed) = alone.or(store::invite_event(conn, account, &invite.uid)?) else {
        return Ok(());
    };
    // An occurrence the calendar leaves out stays out.
    if invite.recurrence_id != 0
        && filed.recurrence_id == 0
        && filed.recurring
        && excludes_occurrence(&filed, invite.recurrence_id)
    {
        return Ok(());
    }
    // Only the organizer revises an event, and only in mail from them: anyone
    // else who knows its UID (another attendee, say) must not move it on the
    // user's calendar unasked. Answering their copy still can.
    let organizer = calendar::parse_invite(&filed.ics)
        .map(|filed| filed.organizer.addr.trim().to_ascii_lowercase())
        .unwrap_or_default();
    if organizer.is_empty()
        || organizer != invite.organizer.addr.trim().to_ascii_lowercase()
        || organizer != sender.trim().to_ascii_lowercase()
    {
        return Ok(());
    }
    // Something in it is newer than what is filed, and the series it brings
    // (if it brings one) is not older: that replaces the filed one whole.
    let revisions = revisions(invite);
    let newer = revisions
        .iter()
        .any(|&(recurrence_id, sequence)| sequence > filed_sequence(&filed, recurrence_id));
    let older_series = invite.recurrence_id == 0
        && revisions.iter().any(|&(recurrence_id, sequence)| {
            recurrence_id == 0 && sequence < filed_sequence(&filed, 0)
        });
    if !newer || older_series {
        return Ok(());
    }
    let recorded = store::invite_response(conn, account, &invite.uid, invite.recurrence_id)?;
    let Some(response) = recorded
        .and_then(|recorded| Response::parse(&recorded.response))
        .or_else(|| Response::parse(&filed.response))
        .filter(|response| *response != Response::Declined)
    else {
        return Ok(());
    };
    // An automatic update is no answer: the dates the user answered one by
    // one keep those answers, and the others stay with the series'.
    if invite.recurrence_id != 0
        && filed.recurrence_id == 0
        && filed.recurring
        && !filed.ics.is_empty()
    {
        let mut series = filed;
        let answer = occurrence_answer(&series.ics, invite.recurrence_id);
        series.ics = with_occurrence(&series.ics, invite, answer)?;
        settle_start(&mut series);
        store::save_event(conn, &series)?;
        return Ok(());
    }
    let invite = if invite.recurrence_id == 0 && filed.recurring {
        &with_occurrence_answers(conn, account, invite, &filed)?
    } else {
        invite
    };
    record_invite_event(
        conn,
        account,
        &filed.thread_id,
        message_id,
        invite,
        response,
    )
}

/// The answer the user gave the date at `recurrence_id` on its own, if any.
fn occurrence_answer(series_ics: &str, recurrence_id: i64) -> Option<Response> {
    let series = calendar::parse_ical(series_ics)?;
    overrides(&series)
        .filter(|component| recurrence_instant(&series, component) == Some(recurrence_id))
        .find_map(|component| {
            Response::parse(&calendar::text_of(
                component,
                ICalendarProperty::Other(OCCURRENCE_RESPONSE.to_string()),
            ))
        })
}

/// [`apply_invite_update`] for the revisions of `uid` already downloaded,
/// oldest first: an update can arrive before the user answers the invitation
/// it revises, and then had nothing filed to change.
pub fn apply_cached_updates(conn: &Connection, account: &str, uid: &str) -> Result<()> {
    let mut cached = store::cached_invites(conn, account, uid)?;
    cached.sort_by_key(|(_, _, invite)| invite.sequence);
    for (sender, message_id, invite) in cached {
        apply_invite_update(conn, account, &sender, &message_id, &invite)?;
    }
    Ok(())
}

/// The revised series `invite` with the user's own answers about its dates:
/// the organizer's override of a date they declined goes, so the decline (put
/// back by [`record_invite_event`]) still holds, and an override of a date
/// they answered otherwise than the series takes that answer.
fn with_occurrence_answers(
    conn: &Connection,
    account: &str,
    invite: &Invite,
    filed: &CalendarEvent,
) -> Result<Invite> {
    let (Some(mut incoming), Some(stored)) = (
        calendar::parse_ical(&invite.ics),
        calendar::parse_ical(&filed.ics),
    ) else {
        return Ok(invite.clone());
    };
    for (recurrence_id, _) in store::declined_occurrences(conn, account, &invite.uid)? {
        remove_overrides(&mut incoming, recurrence_id);
    }
    let private = ICalendarProperty::Other(OCCURRENCE_RESPONSE.to_string());
    let answers: Vec<(i64, String)> = overrides(&stored)
        .filter_map(|component| {
            let answer = calendar::text_of(component, private.clone());
            (!answer.is_empty())
                .then(|| Some((recurrence_instant(&stored, component)?, answer)))
                .flatten()
        })
        .collect();
    let at: Vec<(usize, i64)> = incoming
        .components
        .iter()
        .enumerate()
        .filter(|(_, component)| {
            component.component_type == ICalendarComponentType::VEvent
                && component.is_recurrence_override()
        })
        .filter_map(|(index, component)| Some((index, recurrence_instant(&incoming, component)?)))
        .collect();
    for (index, recurrence_id) in at {
        if let Some((_, answer)) = answers.iter().find(|(rid, _)| *rid == recurrence_id) {
            let component = &mut incoming.components[index];
            component.entries.retain(|entry| entry.name != private);
            component.add_property(private.clone(), ICalendarValue::Text(answer.clone()));
        }
    }
    Ok(Invite {
        ics: incoming.to_string(),
        ..invite.clone()
    })
}

/// Each event `invite` carries, as (RECURRENCE-ID, SEQUENCE): a series may
/// bring occurrences the organizer revised on their own, at their own count.
fn revisions(invite: &Invite) -> Vec<(i64, i64)> {
    if invite.recurrence_id != 0 {
        return vec![(invite.recurrence_id, invite.sequence)];
    }
    let Some(ical) = calendar::parse_ical(&invite.ics) else {
        return vec![(0, invite.sequence)];
    };
    ical.components
        .iter()
        .filter(|component| component.component_type == ICalendarComponentType::VEvent)
        .filter_map(|component| {
            let recurrence_id = if component.is_recurrence_override() {
                recurrence_instant(&ical, component)?
            } else {
                0
            };
            Some((recurrence_id, own_sequence(component)))
        })
        .collect()
}

/// The revision of `filed` an update about `recurrence_id` must be newer than.
/// An occurrence moved within a series keeps its own (see [`super::recurrence::OWN_SEQUENCE`]);
/// the series' counts too, as an organizer's SEQUENCE only grows.
fn filed_sequence(filed: &CalendarEvent, recurrence_id: i64) -> i64 {
    if recurrence_id == 0 || filed.recurrence_id != 0 {
        return calendar::parse_invite(&filed.ics).map_or(0, |filed| filed.sequence);
    }
    let Some(ical) = calendar::parse_ical(&filed.ics) else {
        return 0;
    };
    ical.components
        .iter()
        .enumerate()
        .filter(|(index, component)| {
            Some(*index) == master_index(&ical)
                || (component.component_type == ICalendarComponentType::VEvent
                    && component.is_recurrence_override()
                    && recurrence_instant(&ical, component) == Some(recurrence_id))
        })
        .map(|(_, component)| own_sequence(component))
        .max()
        .unwrap_or(0)
}

/// Put the event `invite` describes in the calendar, over `existing` (the row
/// an earlier answer filed) when there is one.
fn file_invite(
    conn: &Connection,
    existing: Option<CalendarEvent>,
    account: &str,
    thread_id: &str,
    message_id: &str,
    invite: &Invite,
    response: Response,
) -> Result<CalendarEvent> {
    let mut event = existing.unwrap_or_else(|| CalendarEvent {
        id: crate::tasks::new_id("event"),
        ..Default::default()
    });
    event.title = invite.summary.clone();
    // The user's own notes on an earlier revision outlive the organizer's.
    if event.notes.is_empty() {
        event.notes = invite.description.clone();
    }
    event.location = invite.location.clone();
    event.start_at = invite.start;
    event.end_at = invite.end;
    set_all_day(&mut event, invite.all_day);
    event.recurring = invite.recurring;
    event.account = account.to_string();
    if !thread_id.is_empty() {
        event.thread_id = thread_id.to_string();
    }
    event.message_id = message_id.to_string();
    event.invite_uid = invite.uid.clone();
    event.recurrence_id = invite.recurrence_id;
    event.response = response.as_str().to_string();
    // A series answered again keeps the dates the user already dealt with.
    let incoming = if invite.recurrence_id == 0 {
        with_series_sequence(&invite.ics)?
    } else {
        invite.ics.clone()
    };
    event.ics = if invite.recurrence_id == 0 && event.recurring && !event.ics.is_empty() {
        carry_exceptions(&event.ics, &incoming)?
    } else {
        incoming
    };
    settle_start(&mut event);
    store::save_event(conn, &event)
}
