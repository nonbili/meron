//! C ABI entry points for mobile hosts.
//!
//! This is intentionally small while the full engine is still being extracted
//! from the desktop sidecar binary. Android/JNI and iOS can use these functions
//! to assert protocol compatibility before calling the future command API.

use std::collections::HashMap;
use std::ffi::CStr;
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::path::Path;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use jni::JNIEnv;
use jni::JavaVM;
use jni::objects::{GlobalRef, JClass, JObject, JString, JValue};
use jni::sys::{jboolean, jstring};
use serde_json::{Value, json};

use crate::engine::{Engine, OAuthDefaults, set_oauth_defaults};
use crate::protocol::{
    MobileHost, PROTOCOL_VERSION, Request, account_needs_reconnect, canon_folder,
    invoke_mobile_protocol_json, is_rss_account, load_mobile_account_creds, mobile_db_key,
    ping_response, ready_event, req_account_id, req_str, sync_mobile_mail,
};

type EventCallback = unsafe extern "C" fn(event_json: *const c_char, user_data: *mut c_void);

#[derive(Clone, Copy)]
struct EventSink {
    callback: EventCallback,
    user_data: usize,
}

static EVENT_SINK: Mutex<Option<EventSink>> = Mutex::new(None);
static MOBILE_CONFIG: Mutex<Option<MobileConfig>> = Mutex::new(None);
static ANDROID_EVENT_DISPATCHER: OnceLock<AndroidEventDispatcher> = OnceLock::new();
static MOBILE_IDLE_WATCHES: Mutex<Option<HashMap<String, Arc<AtomicBool>>>> = Mutex::new(None);

/// The shared mail [`Engine`], hosted while the app is foreground so the request
/// path reuses warm IMAP sessions (and, later, drives foreground IDLE) instead
/// of reconnecting per FFI call. Built on `engine.foreground`, kept across calls,
/// and parked (pool cleared) on `engine.background`. `None` until the first
/// foreground transition.
static ENGINE: Mutex<Option<Arc<Engine>>> = Mutex::new(None);

/// Long-lived multi-thread tokio runtime that owns the Engine's async work
/// (pooled session ops and, later, IDLE watcher tasks). Built once, lazily, so
/// individual FFI calls can `block_on` engine ops without spinning up a runtime
/// each time.
static ENGINE_RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// The Engine's runtime, built on first use. Returns `None` only if the runtime
/// failed to build (host out of threads/memory), which the caller surfaces.
fn engine_runtime() -> Option<&'static tokio::runtime::Runtime> {
    if ENGINE_RT.get().is_none() {
        match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => {
                let _ = ENGINE_RT.set(rt);
            }
            Err(err) => {
                eprintln!("meron-core: engine runtime build failed: {err}");
                return None;
            }
        }
    }
    ENGINE_RT.get()
}

/// A clone of the live Engine handle, if one is hosted (app is foreground).
pub(crate) fn current_engine() -> Option<Arc<Engine>> {
    ENGINE.lock().unwrap().clone()
}

/// Evict `account_id` from the hosted Engine's in-memory creds cache, if an
/// Engine is live. Account mutations (add/reconnect, removal, platform token
/// push via `account.updateOAuthToken`) go through the stateless DB path, and
/// `ensure_valid_creds` only hydrates accounts *missing* from its cache — so
/// without eviction a warm foreground Engine keeps authenticating with the
/// stale creds it loaded before the mutation.
pub(crate) fn evict_engine_account(account_id: &str) {
    if let Some(engine) = current_engine() {
        engine.accounts.blocking_lock().remove(account_id);
    }
}

/// The Engine to run a mobile command against: the warm hosted one while
/// foreground, or a transient one (own connection, no pooling) built on demand
/// for background/one-off calls (e.g. a `BGAppRefreshTask` sync before any
/// foreground transition). Either way the command path is identical, so there is
/// only one implementation to maintain.
pub(crate) fn engine_for(data_dir: &str) -> Result<Arc<Engine>, String> {
    if let Some(engine) = current_engine() {
        return Ok(engine);
    }
    let host = Box::new(MobileHost {
        data_dir: data_dir.to_string(),
    });
    Engine::new(host)
        .map(Arc::new)
        .map_err(|err| format!("{err:#}"))
}

/// Run an async engine op on the shared runtime, blocking the calling FFI thread
/// until it completes. Used by the mobile command handlers routed through the
/// Engine.
pub(crate) fn engine_block_on<F, T>(future: F) -> Result<T, String>
where
    F: std::future::Future<Output = anyhow::Result<T>>,
{
    let rt = engine_runtime().ok_or_else(|| "engine runtime unavailable".to_string())?;
    rt.block_on(future).map_err(|err| format!("{err:#}"))
}

/// INBOX IDLE watches started by `engine.foreground` (as opposed to the opt-in
/// live-mail-push feature). Tracked separately so `engine.background` tears down
/// only its own watches and leaves any background live-push watches running.
static ENGINE_OWNED_WATCHES: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// Build and host the Engine for the foreground session (idempotent), then start
/// foreground IMAP IDLE on each active account's INBOX so new mail lands in the
/// open view live. The Engine holds its own DB connection and warm session pool
/// for the lifetime of the foreground state.
fn engine_foreground() -> Result<Value, String> {
    let data_dir = MOBILE_CONFIG
        .lock()
        .unwrap()
        .as_ref()
        .map(|config| config.data_dir.clone())
        .ok_or_else(|| "mobile core is not initialized".to_string())?;
    // Make sure the runtime is ready so the first routed op doesn't pay for it.
    engine_runtime().ok_or_else(|| "engine runtime unavailable".to_string())?;
    {
        let mut slot = ENGINE.lock().unwrap();
        if slot.is_none() {
            let host = Box::new(MobileHost {
                data_dir: data_dir.clone(),
            });
            let engine = Engine::new(host).map_err(|err| format!("{err:#}"))?;
            *slot = Some(Arc::new(engine));
        }
    }
    start_foreground_idle(&data_dir);
    Ok(json!({ "ok": true }))
}

/// Park the Engine for the background state: stop the foreground IDLE watches and
/// drop warm pooled sockets (the OS freezes/reclaims them anyway). The Engine
/// itself (DB + creds) is kept so the next foreground transition resumes fast.
fn engine_background() -> Result<Value, String> {
    let owned: Vec<(String, String)> = ENGINE_OWNED_WATCHES.lock().unwrap().drain(..).collect();
    for (account, folder) in owned {
        let _ = stop_mobile_idle_watch(&account, &folder);
    }
    if let Some(engine) = ENGINE.lock().unwrap().as_ref() {
        engine.clear_all_pools();
    }
    Ok(json!({ "ok": true }))
}

/// Start IDLE for each active (non-RSS, non-paused) account on both INBOX and the
/// account's Sent folder — INBOX so received mail appears live, Sent so mail sent
/// from another client (each IDLE connection watches a single mailbox) also shows
/// up live in the cross-folder conversation view. Records the watches we actually
/// started so `engine_background` stops exactly those, leaving any opt-in
/// live-push watches alone.
fn start_foreground_idle(data_dir: &str) {
    let accounts = match mobile_db(data_dir) {
        Ok(conn) => crate::store::load_accounts(&conn).unwrap_or_default(),
        Err(_) => return,
    };
    for (id, _creds) in accounts {
        let (skip, sent) = match mobile_db(data_dir) {
            Ok(conn) => {
                let skip = is_rss_account(&conn, &id).unwrap_or(false)
                    || crate::store::account_paused(&conn, &id).unwrap_or(false);
                let sent = crate::store::get_folders(&conn, &id)
                    .ok()
                    .and_then(|folders| {
                        folders
                            .into_iter()
                            .map(|folder| folder.name)
                            .find(|name| crate::imap::looks_like_sent(name))
                    });
                (skip, sent)
            }
            Err(_) => (true, None),
        };
        if skip {
            continue;
        }
        start_owned_watch(data_dir, &id, "INBOX");
        if let Some(sent) = sent
            && !sent.eq_ignore_ascii_case("INBOX")
        {
            start_owned_watch(data_dir, &id, &sent);
        }
    }
}

/// Start one IDLE watch and, if it was newly started (not already running and not
/// skipped as rss/paused), record it as engine-owned for teardown on background.
fn start_owned_watch(data_dir: &str, account: &str, folder: &str) {
    if let Ok(result) = start_mobile_idle_watch(data_dir, account.to_string(), folder.to_string())
        && result.get("already").and_then(Value::as_bool) != Some(true)
        && result.get("rss").is_none()
        && result.get("paused").is_none()
    {
        ENGINE_OWNED_WATCHES
            .lock()
            .unwrap()
            .push((account.to_string(), folder.to_string()));
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MobileConfig {
    data_dir: String,
    /// SQLCipher key (64 hex chars), or empty for an unencrypted store.
    db_key: String,
}

struct AndroidEventDispatcher {
    vm: JavaVM,
    class: GlobalRef,
}

#[unsafe(no_mangle)]
pub extern "C" fn meron_core_protocol_version() -> u32 {
    PROTOCOL_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_jp_nonbili_meron_MeronCoreNative_meronCoreProtocolVersion(
    _env: *mut c_void,
    _class: *mut c_void,
) -> c_int {
    PROTOCOL_VERSION as c_int
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_jp_nonbili_meron_MeronCoreNative_meronCoreConfigureOAuthDefaults(
    mut env: JNIEnv,
    _class: JClass,
    google_client_id: JString,
    google_token_url: JString,
    outlook_client_id: JString,
) {
    let google_client_id = env
        .get_string(&google_client_id)
        .map(|value| value.to_string_lossy().trim().to_string())
        .unwrap_or_default();
    let google_token_url = env
        .get_string(&google_token_url)
        .map(|value| value.to_string_lossy().trim().to_string())
        .unwrap_or_default();
    let outlook_client_id = env
        .get_string(&outlook_client_id)
        .map(|value| value.to_string_lossy().trim().to_string())
        .unwrap_or_default();
    set_oauth_defaults(OAuthDefaults {
        google_client_id,
        google_token_url,
        outlook_client_id,
        ..OAuthDefaults::default()
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_jp_nonbili_meron_MeronCoreNative_meronCoreInvokeJson(
    mut env: JNIEnv,
    _class: JClass,
    request: JString,
) -> jstring {
    let request = match env.get_string(&request) {
        Ok(value) => value.to_string_lossy().into_owned(),
        Err(err) => {
            return java_string(
                &env,
                serde_json::json!({ "error": { "message": format!("bad JNI string: {err}") } })
                    .to_string(),
            );
        }
    };
    java_string(&env, invoke_mobile_request_json(&request).to_string())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_jp_nonbili_meron_MeronCoreNative_meronCoreInitJson(
    mut env: JNIEnv,
    _class: JClass,
    data_dir: JString,
) -> jstring {
    let data_dir = match env.get_string(&data_dir) {
        Ok(value) => value.to_string_lossy().into_owned(),
        Err(err) => {
            return java_string(
                &env,
                serde_json::json!({ "error": { "message": format!("bad JNI string: {err}") } })
                    .to_string(),
            );
        }
    };
    java_string(&env, init_mobile_core(&data_dir, None).to_string())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_jp_nonbili_meron_MeronCoreNative_meronCoreInitJsonKeyed(
    mut env: JNIEnv,
    _class: JClass,
    data_dir: JString,
    db_key: JString,
) -> jstring {
    let data_dir = match env.get_string(&data_dir) {
        Ok(value) => value.to_string_lossy().into_owned(),
        Err(err) => {
            return java_string(
                &env,
                serde_json::json!({ "error": { "message": format!("bad JNI string: {err}") } })
                    .to_string(),
            );
        }
    };
    let db_key = match env.get_string(&db_key) {
        Ok(value) => value.to_string_lossy().into_owned(),
        Err(err) => {
            return java_string(
                &env,
                serde_json::json!({ "error": { "message": format!("bad JNI string: {err}") } })
                    .to_string(),
            );
        }
    };
    java_string(&env, init_mobile_core(&data_dir, Some(&db_key)).to_string())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_jp_nonbili_meron_MeronCoreNative_meronCoreRegisterEventCallback(
    env: JNIEnv,
    class: JClass,
) {
    let Ok(vm) = env.get_java_vm() else {
        return;
    };
    let Ok(class) = env.new_global_ref(class) else {
        return;
    };
    let _ = ANDROID_EVENT_DISPATCHER.set(AndroidEventDispatcher { vm, class });
    meron_core_register_event_callback(Some(android_event_callback), ptr::null_mut());
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_jp_nonbili_meron_MeronCoreNative_meronCoreUnregisterEventCallback(
    _env: JNIEnv,
    _class: JClass,
) {
    meron_core_register_event_callback(None, ptr::null_mut());
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_jp_nonbili_meron_MeronCoreNative_meronCoreEmitReadyEvent(
    _env: JNIEnv,
    _class: JClass,
) -> jboolean {
    meron_core_emit_ready_event().into()
}

#[unsafe(no_mangle)]
pub extern "C" fn meron_core_ready_json() -> *mut c_char {
    json_to_c_string(ready_event())
}

#[unsafe(no_mangle)]
pub extern "C" fn meron_core_ping_json() -> *mut c_char {
    json_to_c_string(ping_response())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn meron_core_invoke_json(request: *const c_char) -> *mut c_char {
    if request.is_null() {
        return json_to_c_string(serde_json::json!({
            "error": { "message": "request pointer is null" }
        }));
    }
    let request = unsafe { CStr::from_ptr(request) }.to_string_lossy();
    json_to_c_string(invoke_mobile_request_json(&request))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn meron_core_init_json(data_dir: *const c_char) -> *mut c_char {
    if data_dir.is_null() {
        return json_to_c_string(serde_json::json!({
            "error": { "message": "data_dir pointer is null" }
        }));
    }
    let data_dir = unsafe { CStr::from_ptr(data_dir) }.to_string_lossy();
    json_to_c_string(init_mobile_core(&data_dir, None))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn meron_core_init_json_keyed(
    data_dir: *const c_char,
    db_key: *const c_char,
) -> *mut c_char {
    if data_dir.is_null() || db_key.is_null() {
        return json_to_c_string(serde_json::json!({
            "error": { "message": "data_dir and db_key pointers are required" }
        }));
    }
    let data_dir = unsafe { CStr::from_ptr(data_dir) }.to_string_lossy();
    let db_key = unsafe { CStr::from_ptr(db_key) }.to_string_lossy();
    json_to_c_string(init_mobile_core(&data_dir, Some(&db_key)))
}

/// Registers the mobile host event callback.
///
/// Passing `None` unregisters the callback. The callback receives a pointer
/// that is valid only for the duration of the call, so platform hosts must copy
/// the JSON string before returning.
#[unsafe(no_mangle)]
pub extern "C" fn meron_core_register_event_callback(
    callback: Option<EventCallback>,
    user_data: *mut c_void,
) {
    let mut sink = EVENT_SINK.lock().unwrap();
    *sink = callback.map(|callback| EventSink {
        callback,
        user_data: user_data as usize,
    });
}

/// Emits the initial ready event through the registered callback.
///
/// Mobile hosts call this after registering their callback to verify event
/// delivery before issuing commands. Later extraction work will route all core
/// events through the same sink.
#[unsafe(no_mangle)]
pub extern "C" fn meron_core_emit_ready_event() -> bool {
    emit_event("ready", ready_event())
}

/// Frees strings returned from Meron core FFI functions.
///
/// # Safety
///
/// `ptr` must be either null or a pointer returned by this library from a
/// function documented as returning an owned C string. Passing any other pointer
/// is undefined behavior.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn meron_core_string_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}

fn json_to_c_string(value: serde_json::Value) -> *mut c_char {
    match CString::new(value.to_string()) {
        Ok(s) => s.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

fn init_mobile_core(data_dir: &str, db_key: Option<&str>) -> serde_json::Value {
    let data_dir = data_dir.trim();
    if data_dir.is_empty() {
        return serde_json::json!({ "error": { "message": "data_dir is required" } });
    }
    let db_key = db_key.map(str::trim).unwrap_or_default();
    let mut config = MOBILE_CONFIG.lock().unwrap();
    *config = Some(MobileConfig {
        data_dir: data_dir.to_string(),
        db_key: db_key.to_string(),
    });
    // Publish the key so `with_mobile_db` opens the encrypted store. An empty
    // key (unkeyed init) leaves the store plaintext.
    crate::protocol::set_mobile_db_key(if db_key.is_empty() {
        None
    } else {
        Some(db_key.to_string())
    });
    // Route core logs to the platform over the event channel; the host forwards
    // `log` events to os_log / Logcat. Without this the core's logs (which use
    // `eprintln!` on desktop) are invisible on device.
    crate::log::set_sink(Box::new(|level, tag, message| {
        let _ = emit_event(
            "log",
            json!({ "level": level.as_str(), "tag": tag, "message": message }),
        );
    }));
    serde_json::json!({
        "ok": true,
        "protocol": PROTOCOL_VERSION,
        "data_dir": data_dir,
    })
}

fn invoke_mobile_request_json(request: &str) -> serde_json::Value {
    let data_dir = MOBILE_CONFIG
        .lock()
        .unwrap()
        .as_ref()
        .map(|config| config.data_dir.clone());
    if let Some(data_dir) = data_dir.as_deref()
        && let Ok(req) = serde_json::from_str::<Request>(request)
        && matches!(
            req.method.as_str(),
            "watch.start" | "watch.stop" | "engine.foreground" | "engine.background"
        )
    {
        let result = match req.method.as_str() {
            "engine.foreground" => engine_foreground(),
            "engine.background" => engine_background(),
            _ => dispatch_mobile_watch_request(data_dir, &req),
        };
        return match result {
            Ok(result) => json!({ "id": req.id, "result": result }),
            Err(message) => json!({ "id": req.id, "error": { "message": message } }),
        };
    }
    invoke_mobile_protocol_json(request, data_dir.as_deref())
}

fn event_envelope(name: &str, detail: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "event": name, "detail": detail })
}

pub(crate) fn emit_event(name: &str, detail: serde_json::Value) -> bool {
    let sink = *EVENT_SINK.lock().unwrap();
    let Some(sink) = sink else {
        return false;
    };
    let Ok(event_json) = CString::new(event_envelope(name, detail).to_string()) else {
        return false;
    };
    unsafe {
        (sink.callback)(event_json.as_ptr(), sink.user_data as *mut c_void);
    }
    true
}

fn dispatch_mobile_watch_request(data_dir: &str, req: &Request) -> Result<Value, String> {
    let account = req_account_id(&req.params)?;
    let folder =
        canon_folder(&req_str(&req.params, "folder").unwrap_or_else(|_| "INBOX".to_string()));
    match req.method.as_str() {
        "watch.start" => start_mobile_idle_watch(data_dir, account, folder),
        "watch.stop" => stop_mobile_idle_watch(&account, &folder),
        _ => Err(format!("unknown watch method: {}", req.method)),
    }
}

fn watch_key(account: &str, folder: &str) -> String {
    format!("{account}\n{folder}")
}

fn mobile_db(data_dir: &str) -> Result<rusqlite::Connection, String> {
    let db_path = Path::new(data_dir.trim()).join("meron.db");
    match mobile_db_key() {
        Some(key) => crate::store::open_at_keyed(&db_path, &key),
        None => crate::store::open_at(&db_path),
    }
    .map_err(|err| format!("{err:#}"))
}

fn start_mobile_idle_watch(
    data_dir: &str,
    account: String,
    folder: String,
) -> Result<Value, String> {
    {
        let conn = mobile_db(data_dir)?;
        if is_rss_account(&conn, &account)? {
            return Ok(json!({ "ok": true, "rss": true }));
        }
        if crate::store::account_paused(&conn, &account).map_err(|err| format!("{err:#}"))? {
            return Ok(json!({ "ok": true, "paused": true }));
        }
        let creds = load_mobile_account_creds(&conn, &account)?;
        if account_needs_reconnect(&creds) {
            return Err(format!("account needs reconnect: {account}"));
        }
    }

    let key = watch_key(&account, &folder);
    let mut watches = MOBILE_IDLE_WATCHES.lock().unwrap();
    let watches = watches.get_or_insert_with(HashMap::new);
    if watches.contains_key(&key) {
        return Ok(json!({ "ok": true, "already": true }));
    }
    let stop = Arc::new(AtomicBool::new(false));
    watches.insert(key, stop.clone());
    let data_dir = data_dir.to_string();
    std::thread::Builder::new()
        .name(format!("meron-idle-{account}-{folder}"))
        .spawn({
            let account = account.clone();
            let folder = folder.clone();
            move || mobile_idle_thread(data_dir, account, folder, stop)
        })
        .map_err(|err| format!("spawn idle watcher: {err}"))?;
    Ok(json!({ "ok": true, "already": false }))
}

fn stop_mobile_idle_watch(account: &str, folder: &str) -> Result<Value, String> {
    let key = watch_key(account, folder);
    let removed = MOBILE_IDLE_WATCHES
        .lock()
        .unwrap()
        .as_mut()
        .and_then(|watches| watches.remove(&key));
    if let Some(stop) = removed.as_ref() {
        stop.store(true, Ordering::SeqCst);
    }
    Ok(json!({ "ok": true, "stopped": removed.is_some() }))
}

fn mobile_idle_thread(data_dir: String, account: String, folder: String, stop: Arc<AtomicBool>) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            emit_event(
                "error",
                json!({ "message": format!("idle runtime: {err}") }),
            );
            return;
        }
    };
    runtime.block_on(async {
        while !stop.load(Ordering::SeqCst) {
            match mobile_idle_once(&data_dir, &account, &folder, &stop).await {
                Ok(()) => {}
                Err(err) => {
                    emit_event(
                        "error",
                        json!({ "message": format!("idle {account}/{folder}: {err:#}") }),
                    );
                    tokio::time::sleep(Duration::from_secs(15)).await;
                }
            }
        }
        let _ = MOBILE_IDLE_WATCHES
            .lock()
            .unwrap()
            .as_mut()
            .map(|watches| watches.remove(&watch_key(&account, &folder)));
    });
}

async fn mobile_idle_once(
    data_dir: &str,
    account: &str,
    folder: &str,
    stop: &AtomicBool,
) -> anyhow::Result<()> {
    {
        let conn = mobile_db(data_dir).map_err(|err| anyhow::anyhow!(err))?;
        if crate::store::account_paused(&conn, account).unwrap_or(false) {
            return Ok(());
        }
    }
    // IDLE used to load the stored credentials directly and authenticate before
    // entering the normal Engine sync path. An expired browser-flow OAuth token
    // therefore failed the connection before Engine::ensure_valid_creds could
    // refresh it. Use the same credential path as every other live IMAP op.
    // Platform-managed Android Gmail is refreshed by the host before it invokes
    // engine.foreground/watch.start; ensure_valid_creds then picks up that token.
    let engine = engine_for(data_dir).map_err(|err| anyhow::anyhow!(err))?;
    let creds = engine.ensure_valid_creds(account).await?;
    if account_needs_reconnect(&creds) {
        anyhow::bail!("account needs reconnect: {account}");
    }
    let mut session = crate::imap::connect(&creds).await?;
    session.select(folder).await?;
    mobile_sync_and_notify(data_dir, account, folder).await?;

    while !stop.load(Ordering::SeqCst) {
        let mut handle = session.idle();
        handle.init().await?;
        let response = {
            let (idle_fut, _stop) = handle.wait_with_timeout(Duration::from_secs(15 * 60));
            idle_fut.await
        };
        session = handle.done().await?;
        if let async_imap::extensions::idle::IdleResponse::NewData(_) = response? {
            mobile_sync_and_notify(data_dir, account, folder).await?;
        }
    }
    Ok(())
}

async fn mobile_sync_and_notify(data_dir: &str, account: &str, folder: &str) -> anyhow::Result<()> {
    let data_dir_owned = data_dir.to_string();
    let account_owned = account.to_string();
    let folder_owned = folder.to_string();
    let sync = tokio::task::spawn_blocking(move || {
        sync_mobile_mail(
            &data_dir_owned,
            &json!({
                "account_id": account_owned,
                "folder_id": folder_owned,
                "limit": 50,
                "folders": false,
            }),
        )
    })
    .await?
    .map_err(|err| anyhow::anyhow!(err))?;
    let synced = sync.get("synced").and_then(Value::as_u64).unwrap_or(0);

    if let Some(detail) = sync.get("new_messages").filter(|value| value.is_object()) {
        emit_event("mail.newMessages", detail.clone());
    } else {
        emit_event(
            "mail.synced",
            json!({ "account": account, "folder": folder, "synced": synced }),
        );
    }
    Ok(())
}

/// `mail.newMessages` detail for inbox messages that arrived between the two
/// `uid_next` snapshots, or None when nothing new and unread did. Shared by the
/// IDLE watch event and the `mail.sync` response so background workers — which
/// have no live event listener — can notify from the response alone.
pub(crate) fn mobile_new_messages_detail(
    data_dir: &str,
    account: &str,
    uid_next_before: u32,
    uid_next_after: u32,
    synced_messages: &[crate::imap::MessageHeader],
) -> Option<Value> {
    let (count, latest) = mobile_new_unread_summary(
        data_dir,
        account,
        uid_next_before,
        uid_next_after,
        synced_messages,
    )?;
    Some(json!({
        "account": account,
        "accountName": mobile_account_label(data_dir, account),
        "folder": "inbox",
        "count": count,
        "muted": mobile_account_muted(data_dir, account),
        "from": display_from(&latest),
        "subject": latest.subject,
        // Branch-aware card key so a notification tap opens the exact list
        // card the grouping produced.
        "threadKey": crate::store::card_thread_key(&latest),
    }))
}

pub(crate) fn mobile_inbox_uid_next(data_dir: &str, account: &str) -> Option<u32> {
    let conn = mobile_db(data_dir).ok()?;
    crate::store::get_folder_state(&conn, account, "INBOX")
        .ok()
        .flatten()
        .map(|(_, uid_next)| uid_next)
}

fn mobile_new_unread_summary(
    data_dir: &str,
    account: &str,
    uid_next_before: u32,
    uid_next_after: u32,
    synced_messages: &[crate::imap::MessageHeader],
) -> Option<(u32, crate::imap::MessageHeader)> {
    let conn = mobile_db(data_dir).ok()?;
    crate::store::new_unread_inbox_summary(
        &conn,
        account,
        uid_next_before,
        uid_next_after,
        synced_messages,
    )
    .ok()
    .flatten()
}

fn mobile_account_label(data_dir: &str, account: &str) -> String {
    let Ok(conn) = mobile_db(data_dir) else {
        return account.to_string();
    };
    conn.query_row(
        "SELECT display_name, email FROM accounts WHERE id = ?1",
        rusqlite::params![account],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )
    .map(|(display_name, email)| {
        if !email.trim().is_empty() {
            email
        } else if !display_name.trim().is_empty() {
            display_name
        } else {
            account.to_string()
        }
    })
    .unwrap_or_else(|_| account.to_string())
}

fn mobile_account_muted(data_dir: &str, account: &str) -> bool {
    mobile_db(data_dir)
        .ok()
        .and_then(|conn| crate::store::account_muted(&conn, account).ok())
        .unwrap_or(false)
}

fn display_from(header: &crate::imap::MessageHeader) -> String {
    if !header.from_name.trim().is_empty() {
        header.from_name.trim().to_string()
    } else {
        header.from_addr.trim().to_string()
    }
}

unsafe extern "C" fn android_event_callback(event_json: *const c_char, _user_data: *mut c_void) {
    if event_json.is_null() {
        return;
    }
    let Some(dispatcher) = ANDROID_EVENT_DISPATCHER.get() else {
        return;
    };
    let event_json = unsafe { CStr::from_ptr(event_json) }.to_string_lossy();
    let Ok(mut env) = dispatcher.vm.attach_current_thread() else {
        return;
    };
    let Ok(event) = env.new_string(event_json.as_ref()) else {
        return;
    };
    let event = JObject::from(event);
    let class = unsafe { JClass::from_raw(dispatcher.class.as_raw() as jni::sys::jclass) };
    let _ = env.call_static_method(
        class,
        "dispatchCoreEventFromNative",
        "(Ljava/lang/String;)V",
        &[JValue::Object(&event)],
    );
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
}

fn java_string(env: &JNIEnv, value: String) -> jstring {
    match env.new_string(value) {
        Ok(output) => output.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;
    use std::sync::Mutex;

    static CAPTURED_EVENTS: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static EVENT_CALLBACK_TEST_LOCK: Mutex<()> = Mutex::new(());

    unsafe extern "C" fn capture_event(event_json: *const c_char, _user_data: *mut c_void) {
        assert!(!event_json.is_null());
        let value = unsafe { CStr::from_ptr(event_json) }
            .to_string_lossy()
            .into_owned();
        CAPTURED_EVENTS.lock().unwrap().push(value);
    }

    unsafe fn take_owned_string(ptr: *mut c_char) -> String {
        assert!(!ptr.is_null());
        let value = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        unsafe { meron_core_string_free(ptr) };
        value
    }

    #[test]
    fn ffi_exposes_protocol_version() {
        assert_eq!(meron_core_protocol_version(), PROTOCOL_VERSION);
    }

    #[test]
    fn ffi_ready_json_contains_protocol() {
        let raw = unsafe { take_owned_string(meron_core_ready_json()) };
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(value["protocol"], PROTOCOL_VERSION);
    }

    #[test]
    fn ffi_ping_json_contains_pong() {
        let raw = unsafe { take_owned_string(meron_core_ping_json()) };
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(value["pong"], true);
        assert_eq!(value["protocol"], PROTOCOL_VERSION);
    }

    #[test]
    fn ffi_invoke_json_wraps_ping() {
        let request = CString::new(r#"{"id":5,"method":"ping"}"#).unwrap();
        let raw = unsafe { take_owned_string(meron_core_invoke_json(request.as_ptr())) };
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(value["id"], 5);
        assert_eq!(value["result"]["pong"], true);
        assert_eq!(value["result"]["protocol"], PROTOCOL_VERSION);
    }

    #[test]
    fn ffi_invoke_json_handles_null() {
        let raw = unsafe { take_owned_string(meron_core_invoke_json(ptr::null())) };
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(value["error"]["message"].as_str().unwrap().contains("null"));
    }

    #[test]
    fn ffi_init_json_records_mobile_data_dir() {
        let data_dir = CString::new("/tmp/meron-mobile-test").unwrap();
        let raw = unsafe { take_owned_string(meron_core_init_json(data_dir.as_ptr())) };
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(value["ok"], true);
        assert_eq!(value["protocol"], PROTOCOL_VERSION);
        assert_eq!(value["data_dir"], "/tmp/meron-mobile-test");
        assert_eq!(
            *MOBILE_CONFIG.lock().unwrap(),
            Some(MobileConfig {
                data_dir: "/tmp/meron-mobile-test".to_string(),
                db_key: String::new(),
            })
        );
    }

    #[test]
    fn ffi_init_json_rejects_missing_data_dir() {
        let data_dir = CString::new("  ").unwrap();
        let raw = unsafe { take_owned_string(meron_core_init_json(data_dir.as_ptr())) };
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            value["error"]["message"]
                .as_str()
                .unwrap()
                .contains("data_dir")
        );
    }

    #[test]
    fn ffi_init_json_handles_null() {
        let raw = unsafe { take_owned_string(meron_core_init_json(ptr::null())) };
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(value["error"]["message"].as_str().unwrap().contains("null"));
    }

    #[test]
    fn ffi_free_accepts_null() {
        unsafe { meron_core_string_free(ptr::null_mut()) };
    }

    #[test]
    fn ffi_ready_event_reaches_registered_callback() {
        let _guard = EVENT_CALLBACK_TEST_LOCK.lock().unwrap();
        CAPTURED_EVENTS.lock().unwrap().clear();
        meron_core_register_event_callback(None, ptr::null_mut());
        meron_core_register_event_callback(Some(capture_event), ptr::null_mut());
        assert!(meron_core_emit_ready_event());
        meron_core_register_event_callback(None, ptr::null_mut());

        let events = CAPTURED_EVENTS.lock().unwrap();
        assert_eq!(events.len(), 1);
        let value: serde_json::Value = serde_json::from_str(&events[0]).unwrap();
        assert_eq!(value["event"], "ready");
        assert_eq!(value["detail"]["protocol"], PROTOCOL_VERSION);
    }

    #[test]
    fn ffi_ready_event_reports_false_without_callback() {
        let _guard = EVENT_CALLBACK_TEST_LOCK.lock().unwrap();
        meron_core_register_event_callback(None, ptr::null_mut());
        CAPTURED_EVENTS.lock().unwrap().clear();
        assert!(!meron_core_emit_ready_event());
    }
}
