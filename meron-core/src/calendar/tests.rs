use super::*;

fn mine(addrs: &[&str]) -> HashSet<String> {
    addrs.iter().map(|addr| addr.to_string()).collect()
}

/// What Outlook sends: a Windows zone name, defined by its own VTIMEZONE.
const OUTLOOK: &str = "BEGIN:VCALENDAR\r
METHOD:REQUEST\r
PRODID:Microsoft Exchange Server 2010\r
VERSION:2.0\r
BEGIN:VTIMEZONE\r
TZID:Pacific Standard Time\r
BEGIN:STANDARD\r
DTSTART:16010101T020000\r
TZOFFSETFROM:-0700\r
TZOFFSETTO:-0800\r
RRULE:FREQ=YEARLY;INTERVAL=1;BYDAY=1SU;BYMONTH=11\r
END:STANDARD\r
BEGIN:DAYLIGHT\r
DTSTART:16010101T020000\r
TZOFFSETFROM:-0800\r
TZOFFSETTO:-0700\r
RRULE:FREQ=YEARLY;INTERVAL=1;BYDAY=2SU;BYMONTH=3\r
END:DAYLIGHT\r
END:VTIMEZONE\r
BEGIN:VEVENT\r
ORGANIZER;CN=Maya Chen:mailto:maya@example.com\r
ATTENDEE;ROLE=REQ-PARTICIPANT;PARTSTAT=NEEDS-ACTION;RSVP=TRUE;CN=Sam Lee:mailto:sam@example.org\r
ATTENDEE;ROLE=OPT-PARTICIPANT;PARTSTAT=ACCEPTED;CN=Kai:mailto:kai@example.com\r
DESCRIPTION;LANGUAGE=en-US:Agenda to follow.\\nBring notes.\r
UID:040000008200E00074C5B7101A82E008\r
SUMMARY;LANGUAGE=en-US:Launch review\r
DTSTART;TZID=Pacific Standard Time:20261022T100000\r
DTEND;TZID=Pacific Standard Time:20261022T110000\r
SEQUENCE:2\r
LOCATION;LANGUAGE=en-US:Room 4\\, second floor\r
DTSTAMP:20261020T120000Z\r
BEGIN:VALARM\r
ACTION:DISPLAY\r
TRIGGER:-PT15M\r
END:VALARM\r
END:VEVENT\r
END:VCALENDAR\r
";

#[test]
fn reads_an_outlook_invite_in_its_windows_zone() {
    let invite = parse_invite(OUTLOOK).unwrap();
    assert_eq!(invite.method, "request");
    assert_eq!(invite.uid, "040000008200E00074C5B7101A82E008");
    assert_eq!(invite.sequence, 2);
    assert_eq!(invite.summary, "Launch review");
    assert_eq!(invite.location, "Room 4, second floor");
    assert_eq!(invite.description, "Agenda to follow.\nBring notes.");
    // 10:00 PDT (UTC-7) on 22 October 2026.
    assert_eq!(invite.start, 1_792_688_400);
    assert_eq!(invite.end, invite.start + 3600);
    assert!(!invite.all_day && !invite.recurring && !invite.cancelled);
    assert_eq!(
        invite.organizer,
        Person {
            name: "Maya Chen".into(),
            addr: "maya@example.com".into()
        }
    );
    assert_eq!(invite.attendees.len(), 2);
    assert_eq!(invite.attendees[0].addr, "sam@example.org");
    assert_eq!(invite.attendees[0].partstat, "needs-action");
    assert_eq!(invite.attendees[1].partstat, "accepted");
}

#[test]
fn reads_utc_and_all_day_events() {
    let google = "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\n\
DTSTART:20261022T170000Z\r\nDTEND:20261022T173000Z\r\nUID:abc@google.com\r\n\
ORGANIZER;CN=Maya:mailto:maya@example.com\r\nSUMMARY:Sync\r\n\
RRULE:FREQ=WEEKLY;BYDAY=TH\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let invite = parse_invite(google).unwrap();
    // The same instant as the Outlook invite's 10:00 PDT.
    assert_eq!(invite.start, 1_792_688_400);
    assert_eq!(invite.end - invite.start, 1800);
    assert!(invite.recurring);

    let holiday = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:h1\r\n\
DTSTART;VALUE=DATE:20261224\r\nDTEND;VALUE=DATE:20261226\r\nSUMMARY:Holiday\r\n\
END:VEVENT\r\nEND:VCALENDAR\r\n";
    let invite = parse_invite(holiday).unwrap();
    assert!(invite.all_day);
    assert_eq!(invite.method, "");
    // UTC midnights, end exclusive: two days.
    assert_eq!(invite.start % 86_400, 0);
    assert_eq!(invite.end - invite.start, 2 * 86_400);

    // A date with no end lasts the day.
    let one_day = holiday.replace("DTEND;VALUE=DATE:20261226\r\n", "");
    let invite = parse_invite(&one_day).unwrap();
    assert_eq!(invite.end - invite.start, 86_400);
}

#[test]
fn a_cancellation_or_a_calendar_without_an_event_is_not_answerable() {
    let cancel = OUTLOOK
        .replace("METHOD:REQUEST", "METHOD:CANCEL")
        .replace("SEQUENCE:2", "SEQUENCE:3\r\nSTATUS:CANCELLED");
    let invite = parse_invite(&cancel).unwrap();
    assert!(invite.cancelled);
    assert!(!can_respond(&invite, &mine(&["sam@example.org"])));

    let todo =
        "BEGIN:VCALENDAR\r\nBEGIN:VTODO\r\nUID:t\r\nSUMMARY:x\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";
    assert_eq!(parse_invite(todo), None);
    assert_eq!(parse_invite("not a calendar"), None);
}

#[test]
fn only_someone_elses_open_request_takes_an_answer() {
    let invite = parse_invite(OUTLOOK).unwrap();
    assert!(can_respond(&invite, &mine(&["sam@example.org"])));
    // The user organized it: nobody to reply to.
    assert!(!can_respond(&invite, &mine(&["maya@example.com"])));
    let published = parse_invite(&OUTLOOK.replace("METHOD:REQUEST", "METHOD:PUBLISH")).unwrap();
    assert!(!can_respond(&published, &mine(&["sam@example.org"])));
}

#[test]
fn a_recorded_answer_stands_until_the_organizer_revises_the_event() {
    let me = mine(&["sam@example.org"]);
    let mut invite = parse_invite(OUTLOOK).unwrap();
    assert_eq!(my_response(&invite, &me, None), "");

    let recorded = crate::store::InviteResponse {
        response: "tentative".into(),
        sequence: 2,
        responded_at: 0,
    };
    assert_eq!(my_response(&invite, &me, Some(&recorded)), "tentative");

    // A newer revision: the organizer's view of the user is all there is.
    invite.sequence = 3;
    invite.attendees[0].partstat = "accepted".into();
    assert_eq!(my_response(&invite, &me, Some(&recorded)), "accepted");
    invite.attendees[0].partstat = "needs-action".into();
    assert_eq!(my_response(&invite, &me, Some(&recorded)), "");
}

#[test]
fn the_reply_carries_the_one_answering_attendee_and_the_zone() {
    let invite = parse_invite(OUTLOOK).unwrap();
    let me = Person {
        name: "Sam Lee".into(),
        addr: "sam@example.org".into(),
    };
    let ics = reply_ics(&invite, &me, Response::Accepted, 1_792_000_000, false).unwrap();
    assert!(ics.contains("METHOD:REPLY"), "{ics}");
    assert!(ics.contains("UID:040000008200E00074C5B7101A82E008"));
    assert!(ics.contains("SEQUENCE:2"));
    assert!(ics.contains("BEGIN:VTIMEZONE"));
    assert!(ics.contains("DTSTAMP:"));
    assert!(!ics.contains("VALARM"));
    assert!(
        !ics.contains("kai@example.com"),
        "only the replying attendee"
    );
    assert!(!ics.contains("DESCRIPTION"));

    // What the organizer's client reads back out of it.
    let back = parse_invite(&ics).unwrap();
    assert_eq!(back.method, "reply");
    assert_eq!(back.start, invite.start);
    assert_eq!(back.attendees.len(), 1);
    assert_eq!(back.attendees[0].addr, "sam@example.org");
    assert_eq!(back.attendees[0].name, "Sam Lee");
    assert_eq!(back.attendees[0].partstat, "accepted");
    assert_eq!(back.organizer.addr, "maya@example.com");

    let mail = reply_mail(&invite, &me, Response::Declined, 1_792_000_000, false).unwrap();
    assert_eq!(mail.to, "\"Maya Chen\" <maya@example.com>");
    assert_eq!(mail.subject, "Declined: Launch review");
    assert!(mail.body.contains("Sam Lee has declined"));
    assert!(mail.ics.contains("PARTSTAT=DECLINED"));
}

// An encrypted invitation is answered in the clear: the answer leaves out its
// title and place, keeping what an organizer's client needs to match it.
#[test]
fn the_reply_to_an_encrypted_invitation_leaves_its_details_out() {
    let invite = parse_invite(OUTLOOK).unwrap();
    let me = Person {
        name: "Sam Lee".into(),
        addr: "sam@example.org".into(),
    };
    let mail = reply_mail(&invite, &me, Response::Accepted, 1_792_000_000, true).unwrap();
    assert_eq!(mail.subject, "Accepted");
    assert_eq!(mail.body, "Sam Lee has accepted this invitation.\r\n");
    let ics = &mail.ics;
    assert!(!ics.contains("Launch review"), "{ics}");
    assert!(!ics.contains("Room 4"), "{ics}");
    for property in ["SUMMARY", "LOCATION"] {
        assert!(!ics.contains(property), "{property} in {ics}");
    }
    assert!(ics.contains("UID:040000008200E00074C5B7101A82E008"));
    assert!(ics.contains("SEQUENCE:2"));
    assert!(ics.contains("PARTSTAT=ACCEPTED"));
    let back = parse_invite(ics).unwrap();
    assert_eq!(back.start, invite.start);
    assert_eq!(back.organizer.addr, "maya@example.com");
    assert_eq!(back.attendees[0].partstat, "accepted");
}

#[test]
fn the_reply_message_is_an_imip_alternative_threaded_on_the_invite() {
    let invite = parse_invite(OUTLOOK).unwrap();
    let me = Person {
        name: "Sam Lee".into(),
        addr: "sam@example.org".into(),
    };
    let mail = reply_mail(&invite, &me, Response::Tentative, 1_792_000_000, false).unwrap();
    let raw =
        crate::smtp::build_invite_reply("Sam Lee", "sam@example.org", &mail, "<inv@x>", "root@x")
            .unwrap();
    let text = String::from_utf8_lossy(&raw);
    assert!(text.contains("multipart/alternative"), "{text}");
    assert!(text.contains("In-Reply-To: <inv@x>"));
    assert!(text.contains("<root@x> <inv@x>"), "{text}");

    let parsed = crate::parse::parse_message(&raw, None);
    assert_eq!(parsed.subject, "Tentative: Launch review");
    let reply = parsed
        .invite
        .expect("the reply's own calendar part is read");
    assert_eq!(reply.method, "reply");
    assert_eq!(reply.attendees[0].partstat, "tentative");
}

#[test]
fn a_message_reads_its_invite_from_the_inline_calendar_part() {
    // Google's shape: a method-bearing text/calendar alternative, plus the same
    // event as an application/ics file.
    let raw = format!(
        "From: Maya <maya@example.com>\r\nSubject: Invitation\r\nMessage-ID: <inv@x>\r\n\
Content-Type: multipart/mixed; boundary=outer\r\n\r\n\
--outer\r\nContent-Type: multipart/alternative; boundary=inner\r\n\r\n\
--inner\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nYou are invited\r\n\
--inner\r\nContent-Type: text/calendar; charset=utf-8; method=REQUEST\r\n\
Content-Transfer-Encoding: 7bit\r\n\r\n{OUTLOOK}\r\n--inner--\r\n\
--outer\r\nContent-Type: application/ics; name=invite.ics\r\n\
Content-Disposition: attachment; filename=invite.ics\r\n\r\n\
{published}\r\n--outer--\r\n",
        published = OUTLOOK.replace("METHOD:REQUEST", "METHOD:PUBLISH")
    );
    let message = crate::parse::parse_message(raw.as_bytes(), None);
    let invite = message.invite.clone().expect("invite");
    assert_eq!(invite.method, "request", "the part naming its method wins");
    assert_eq!(invite.summary, "Launch review");
    assert_eq!(message.file_attachments().count(), 1);
}

#[test]
fn invite_json_hides_the_calendar_text() {
    let invite = parse_invite(OUTLOOK).unwrap();
    let value = invite_json(&invite, &mine(&["sam@example.org"]), None, None);
    assert_eq!(value["method"], "request");
    assert_eq!(value["can_respond"], true);
    assert_eq!(value["response"], "");
    assert_eq!(value["organizer"]["name"], "Maya Chen");
    assert!(value.get("ics").is_none());
}

#[test]
fn message_refs_split_on_the_first_and_last_hash() {
    assert_eq!(
        parse_message_ref("acct#inbox#42"),
        Some(("acct".into(), "INBOX".into(), 42))
    );
    assert_eq!(
        parse_message_ref("acct#Work#Q3#7"),
        Some(("acct".into(), "Work#Q3".into(), 7))
    );
    assert_eq!(parse_message_ref("acct#INBOX#t.abc"), None);
    assert_eq!(parse_message_ref("nohash"), None);
}

#[test]
fn reminder_schedule_keeps_timezone_rules_with_their_timezone_ids() {
    let zone = |id, offset| {
        format!(
            "BEGIN:VTIMEZONE\r\nTZID:{id}\r\nBEGIN:STANDARD\r\nDTSTART:19700101T000000\r\nTZOFFSETFROM:{offset}\r\nTZOFFSETTO:{offset}\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\n"
        )
    };
    let a = zone("A", "+0100");
    let b = zone("B", "+0200");
    let calendar = |zones| {
        format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\n{zones}BEGIN:VEVENT\r\nUID:one\r\nDTSTART;TZID=A:20261026T100000\r\nSUMMARY:Original\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
        )
    };
    let original = calendar(format!("{a}{b}"));
    assert!(same_reminder_schedule(
        &original,
        &calendar(format!("{b}{a}"))
    ));
    assert!(same_reminder_schedule(
        &original,
        &original.replace("SUMMARY:Original", "SUMMARY:Changed")
    ));
    let changed = calendar(format!("{}{}", zone("A", "+0200"), zone("B", "+0100")));
    assert!(!same_reminder_schedule(&original, &changed));
}
