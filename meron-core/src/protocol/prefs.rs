use super::*;

/// Mobile `app.prefsGet`. The mobile hosts keep a platform-side cache
/// (SharedPreferences / NSUserDefaults) so the first frame can paint before the
/// keyed store is open, but the `settings` table is the source of truth — this
/// is what the cache reconciles against once the core is up.
pub(crate) fn get_mobile_prefs(data_dir: &str, params: &Value) -> Result<Value, String> {
    let keys = params
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
    with_mobile_db(data_dir, |conn| {
        let prefs = store::settings_get(&conn, &keys).map_err(|err| format!("{err:#}"))?;
        Ok(json!({ "prefs": prefs }))
    })
}

/// Mobile `app.prefsSet`. Writes a single setting; the host has already updated
/// its own cache, so this is the durable half of a write-through.
pub(crate) fn set_mobile_pref(data_dir: &str, params: &Value) -> Result<Value, String> {
    let key = req_str(params, "key")?;
    let value = params.get("value").cloned().unwrap_or(Value::Null);
    with_mobile_db(data_dir, |conn| {
        store::setting_set(&conn, &key, &value).map_err(|err| format!("{err:#}"))?;
        // The proxy lives in a process-global slot that socket code reads
        // without a DB handle, so republish it as soon as it changes.
        if key == crate::proxy::SETTING_KEY {
            crate::proxy::set_global(crate::proxy::parse_global(&value));
        }
        // Mobile hosts reschedule reminders on calendar changes.
        if key == crate::reminders::LEAD_SETTING_KEY || key == crate::reminders::ALL_DAY_SETTING_KEY
        {
            crate::ffi::emit_event("calendar.changed", json!({ "method": "app.prefsSet" }));
        }
        Ok(json!({ "ok": true }))
    })
}
