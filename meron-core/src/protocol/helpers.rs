use super::*;

pub(crate) fn req_account_id(params: &Value) -> Result<String, String> {
    req_str(params, "account_id").or_else(|_| req_str(params, "account"))
}

pub(crate) fn is_rss_account(
    conn: &rusqlite::Connection,
    account_id: &str,
) -> Result<bool, String> {
    Ok(store::account_engine(conn, account_id)
        .map_err(|err| err.to_string())?
        .as_deref()
        == Some("rss"))
}

pub(crate) fn account_needs_reconnect(creds: &Creds) -> bool {
    if creds.is_oauth() {
        creds
            .refresh_token
            .as_deref()
            .is_none_or(|token| token.trim().is_empty())
    } else {
        creds.password.trim().is_empty()
    }
}

/// Mobile secret persistence. Unlike desktop (OS keychain via `secrets`),
/// mobile has no keychain, so the per-account secret blob lives in the
/// app-private SQLite DB. Keep these the only secret read/write on the mobile
/// path so password/token persistence stays consistent.
pub(crate) fn store_mobile_secret(
    conn: &rusqlite::Connection,
    account_id: &str,
    creds: &Creds,
) -> Result<(), String> {
    let blob = serde_json::to_string(&Secrets::from_creds(creds)).map_err(|err| err.to_string())?;
    store::upsert_secret(conn, account_id, &blob).map_err(|err| err.to_string())
}

pub(crate) fn load_mobile_secret(conn: &rusqlite::Connection, account_id: &str) -> Secrets {
    store::load_secret(conn, account_id)
        .ok()
        .flatten()
        .and_then(|blob| serde_json::from_str::<Secrets>(&blob).ok())
        .unwrap_or_default()
}

pub(crate) fn load_mobile_account_creds(
    conn: &rusqlite::Connection,
    account_id: &str,
) -> Result<Creds, String> {
    let mut creds = store::load_accounts(conn)
        .map_err(|err| err.to_string())?
        .into_iter()
        .find(|(id, _)| id == account_id)
        .map(|(_, creds)| creds)
        .ok_or_else(|| format!("account not found: {account_id}"))?;
    load_mobile_secret(conn, account_id).apply_to(&mut creds);
    Ok(creds)
}

pub(crate) fn parse_mobile_attachments(params: &Value) -> Result<Vec<AttachmentInput>, String> {
    match params.get("attachments") {
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                serde_json::from_value::<AttachmentInput>(value.clone())
                    .map_err(|err| format!("invalid attachment: {err}"))
            })
            .collect(),
        Some(_) => Err("attachments must be an array".to_string()),
        None => Ok(Vec::new()),
    }
}

pub(crate) fn resolve_mobile_send_from(
    conn: &rusqlite::Connection,
    account_id: &str,
    creds: &Creds,
    requested_from: &str,
) -> (String, String) {
    let sender_name = conn
        .query_row(
            "SELECT sender_name FROM accounts WHERE id = ?1",
            rusqlite::params![account_id],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_default();
    let aliases = store::account_aliases(conn, account_id).unwrap_or_default();
    let requested = requested_from.trim().to_lowercase();
    if requested.is_empty() || requested == creds.user.trim().to_lowercase() {
        return (creds.user.clone(), sender_name);
    }
    if let Some(alias) = aliases
        .iter()
        .find(|alias| alias.email.trim().to_lowercase() == requested)
    {
        let name = if alias.name.trim().is_empty() {
            sender_name
        } else {
            alias.name.clone()
        };
        return (alias.email.trim().to_string(), name);
    }
    (creds.user.clone(), sender_name)
}

pub(crate) fn run_mobile_async<F, T>(future: F) -> Result<T, String>
where
    F: std::future::Future<Output = anyhow::Result<T>>,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("tokio runtime: {err}"))?
        .block_on(future)
        .map_err(|err| format!("{err:#}"))
}

pub(crate) fn with_mobile_db<F>(data_dir: &str, f: F) -> Result<Value, String>
where
    F: FnOnce(rusqlite::Connection) -> Result<Value, String>,
{
    let data_dir = data_dir.trim();
    if data_dir.is_empty() {
        return Err("mobile core is not initialized".to_string());
    }
    let db_path = Path::new(data_dir).join("meron.db");
    let conn = match mobile_db_key() {
        Some(key) => store::open_at_keyed(&db_path, &key),
        // No key set (tests/unkeyed init): plaintext, matching prior behavior.
        None => store::open_at(&db_path),
    }
    .map_err(|err| err.to_string())?;
    f(conn)
}

/// SQLCipher key for the mobile store. The platform host (Android Keystore /
/// iOS Keychain) owns the key and passes it through the keyed `init` FFI; we
/// keep it process-global so every `with_mobile_db` opens the encrypted store
/// without threading the key through every command handler.
static MOBILE_DB_KEY: Mutex<Option<String>> = Mutex::new(None);

/// Set (or clear, with an empty/`None` key) the mobile store's SQLCipher key.
pub fn set_mobile_db_key(key: Option<String>) {
    *MOBILE_DB_KEY.lock().unwrap() = key.filter(|k| !k.is_empty());
}

pub(crate) fn mobile_db_key() -> Option<String> {
    MOBILE_DB_KEY.lock().unwrap().clone()
}

pub(crate) fn with_mobile_db_mutex<F>(data_dir: &str, f: F) -> Result<Value, String>
where
    F: FnOnce(Mutex<rusqlite::Connection>) -> Result<Value, String>,
{
    with_mobile_db(data_dir, |conn| f(Mutex::new(conn)))
}

pub(crate) fn account_id(email: &str) -> String {
    email.trim().to_lowercase()
}

pub(crate) fn now_epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub(crate) fn tls_mode(tls: bool, port: u16) -> (bool, bool) {
    if !tls {
        return (false, false);
    }
    match port {
        3143 | 3587 => (false, false),
        143 | 25 | 587 => (false, true),
        _ => (true, false),
    }
}

pub(crate) fn req_str(params: &Value, key: &str) -> Result<String, String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("{key} required"))
}

pub(crate) fn req_str_array(params: &Value, key: &str) -> Result<Vec<String>, String> {
    let values = params
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{key} required"))?;
    Ok(values
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect())
}

pub(crate) fn opt_str(params: &Value, key: &str) -> String {
    params
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

pub(crate) fn req_u16(params: &Value, key: &str) -> Option<u16> {
    params.get(key)?.as_u64()?.try_into().ok()
}

pub(crate) fn opt_u32(params: &Value, key: &str) -> Option<u32> {
    params.get(key)?.as_u64()?.try_into().ok()
}

pub(crate) fn req_bool(params: &Value, key: &str) -> Result<bool, String> {
    params
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("{key} required"))
}
