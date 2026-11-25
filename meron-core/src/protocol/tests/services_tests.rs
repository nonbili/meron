use super::*;

#[test]
fn mobile_protocol_serves_the_tasks_surface() {
    let data_dir = unique_data_dir("tasks");
    let dir = data_dir.to_str().unwrap().to_string();

    // Reading lists on a fresh install bootstraps one, so the screen is never
    // an empty shell with nowhere to type.
    let lists = invoke_mobile_protocol_json(
        r#"{"id":300,"method":"tasks.lists","params":{}}"#,
        Some(&dir),
    );
    assert_eq!(lists["id"], 300);
    let list_id = lists["result"]["default_list_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(!list_id.is_empty(), "{lists}");
    assert_eq!(lists["result"]["lists"].as_array().unwrap().len(), 1);

    let created = invoke_mobile_protocol_json(
        &format!(
            r#"{{"id":301,"method":"tasks.create","params":{{"list_id":"{list_id}","title":"Book the flight","thread_id":"acct#t1","account":"acct"}}}}"#
        ),
        Some(&dir),
    );
    let task_id = created["result"]["task"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(created["result"]["task"]["title"], "Book the flight");
    assert_eq!(created["result"]["task"]["done"], false);

    let linked = invoke_mobile_protocol_json(
        r#"{"id":302,"method":"tasks.forThread","params":{"thread_id":"acct#t1"}}"#,
        Some(&dir),
    );
    assert_eq!(linked["result"]["tasks"][0]["id"], task_id.as_str());

    let done = invoke_mobile_protocol_json(
        &format!(
            r#"{{"id":303,"method":"tasks.setDone","params":{{"task_id":"{task_id}","done":true}}}}"#
        ),
        Some(&dir),
    );
    assert_eq!(done["result"]["task"]["done"], true);

    // Completed tasks are hidden by default and visible on request.
    let active = invoke_mobile_protocol_json(
        &format!(r#"{{"id":304,"method":"tasks.items","params":{{"list_id":"{list_id}"}}}}"#),
        Some(&dir),
    );
    assert!(
        active["result"]["tasks"].as_array().unwrap().is_empty(),
        "{active}"
    );
    let all = invoke_mobile_protocol_json(
        &format!(
            r#"{{"id":305,"method":"tasks.items","params":{{"list_id":"{list_id}","include_completed":true}}}}"#
        ),
        Some(&dir),
    );
    assert_eq!(all["result"]["tasks"].as_array().unwrap().len(), 1);

    let cleared = invoke_mobile_protocol_json(
        &format!(
            r#"{{"id":306,"method":"tasks.clearCompleted","params":{{"list_id":"{list_id}"}}}}"#
        ),
        Some(&dir),
    );
    assert_eq!(cleared["result"]["removed"], 1);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_reports_missing_task_parameters() {
    let data_dir = unique_data_dir("tasks-invalid");
    let dir = data_dir.to_str().unwrap().to_string();
    let value = invoke_mobile_protocol_json(
        r#"{"id":310,"method":"tasks.items","params":{}}"#,
        Some(&dir),
    );
    assert_eq!(value["error"]["message"], "list_id required", "{value}");
    let _ = std::fs::remove_dir_all(data_dir);
}

/// Before a data dir is bound the stub table answers, so a host that polls the
/// core during startup gets an empty surface rather than "unknown method".
#[test]
fn protocol_stub_answers_tasks_without_a_data_dir() {
    let value = invoke_protocol_json(r#"{"id":311,"method":"tasks.lists","params":{}}"#);
    assert_eq!(
        value["result"],
        json!({ "lists": [], "default_list_id": "" })
    );
    let unknown = invoke_protocol_json(r#"{"id":312,"method":"tasks.nope","params":{}}"#);
    assert_eq!(unknown["error"]["message"], "unknown method: tasks.nope");
}

#[test]
fn mobile_protocol_serves_the_sync_services() {
    let data_dir = unique_data_dir("services");
    let dir = data_dir.to_str().unwrap().to_string();
    let call = |id: u64, method: &str, params: &str| {
        invoke_mobile_protocol_json(
            &format!(r#"{{"id":{id},"method":"{method}","params":{params}}}"#),
            Some(&dir),
        )
    };

    // Each service answers on mobile, not only CalDAV.
    for (id, method) in [
        (400, "caldav.accounts"),
        (401, "carddav.accounts"),
        (402, "googleTasks.accounts"),
    ] {
        let answer = call(id, method, "{}");
        assert_eq!(
            answer["result"]["accounts"],
            json!([]),
            "{method}: {answer}"
        );
    }

    let poll = call(403, "googleTasks.pollState", "{}");
    assert_eq!(poll["result"]["enabled"], true, "{poll}");
    assert_eq!(poll["result"]["pending"], json!([]));
    assert_eq!(poll["result"]["accounts"], json!([]));

    // An automatic sync rests while Tasks is off.
    call(
        404,
        "app.prefsSet",
        r#"{"key":"tasks_enabled","value":false}"#,
    );
    let poll = call(405, "googleTasks.pollState", "{}");
    assert_eq!(poll["result"]["enabled"], false, "{poll}");
    let synced = call(406, "googleTasks.sync", r#"{"automatic":true}"#);
    assert_eq!(synced["result"]["changed"], false, "{synced}");
    // A background refresh syncs only the calendar accounts that are due.
    let due = call(408, "caldav.sync", r#"{"automatic":true,"due":true}"#);
    assert_eq!(due["result"]["pulled"], 0, "{due}");

    // Before a data dir is bound, the read-only ones still answer.
    let stub = invoke_mobile_protocol_json(
        r#"{"id":407,"method":"carddav.accounts","params":{}}"#,
        None,
    );
    assert_eq!(stub["result"]["accounts"], json!([]), "{stub}");

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_imports_and_exports_icalendar() {
    let data_dir = unique_data_dir("ics");
    let dir = data_dir.to_str().unwrap().to_string();
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Example//EN\r\nBEGIN:VEVENT\r\nUID:launch@example.test\r\nDTSTAMP:20260101T000000Z\r\nDTSTART:20260310T090000Z\r\nDTEND:20260310T100000Z\r\nSUMMARY:Launch review\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let call = |id: u64, method: &str| {
        invoke_mobile_protocol_json(
            &json!({ "id": id, "method": method, "params": { "ics": ics } }).to_string(),
            Some(&dir),
        )
    };

    let preview = call(410, "calendar.previewImport");
    assert_eq!(preview["result"]["imported"], 1, "{preview}");
    let imported = call(411, "calendar.importIcs");
    assert_eq!(imported["result"]["imported"], 1, "{imported}");
    // The same file again brings in nothing new.
    let again = call(412, "calendar.previewImport");
    assert_eq!(again["result"]["imported"], 0, "{again}");
    assert_eq!(again["result"]["duplicates"], 1, "{again}");

    let exported = invoke_mobile_protocol_json(
        r#"{"id":413,"method":"calendar.exportIcs","params":{}}"#,
        Some(&dir),
    );
    let document = exported["result"]["ics"].as_str().unwrap_or_default();
    assert!(document.contains("SUMMARY:Launch review"), "{exported}");

    let _ = std::fs::remove_dir_all(data_dir);
}
