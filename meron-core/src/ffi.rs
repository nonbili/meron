//! C ABI entry points for mobile hosts.
//!
//! This is intentionally small while the full engine is still being extracted
//! from the desktop sidecar binary. Android/JNI and iOS can use these functions
//! to assert protocol compatibility before calling the future command API.

use std::ffi::CStr;
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use std::sync::{Mutex, OnceLock};

use jni::JNIEnv;
use jni::JavaVM;
use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::{jboolean, jstring};

use crate::protocol::{PROTOCOL_VERSION, invoke_mobile_protocol_json, ping_response, ready_event};

type EventCallback = unsafe extern "C" fn(event_json: *const c_char, user_data: *mut c_void);

#[derive(Clone, Copy)]
struct EventSink {
    callback: EventCallback,
    user_data: usize,
}

static EVENT_SINK: Mutex<Option<EventSink>> = Mutex::new(None);
static MOBILE_CONFIG: Mutex<Option<MobileConfig>> = Mutex::new(None);
static ANDROID_EVENT_DISPATCHER: OnceLock<AndroidEventDispatcher> = OnceLock::new();

#[derive(Clone, Debug, PartialEq, Eq)]
struct MobileConfig {
    data_dir: String,
}

struct AndroidEventDispatcher {
    vm: JavaVM,
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
    java_string(&env, init_mobile_core(&data_dir).to_string())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_jp_nonbili_meron_MeronCoreNative_meronCoreRegisterEventCallback(
    env: JNIEnv,
    _class: JClass,
) {
    let Ok(vm) = env.get_java_vm() else {
        return;
    };
    let _ = ANDROID_EVENT_DISPATCHER.set(AndroidEventDispatcher { vm });
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
    json_to_c_string(init_mobile_core(&data_dir))
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

fn init_mobile_core(data_dir: &str) -> serde_json::Value {
    let data_dir = data_dir.trim();
    if data_dir.is_empty() {
        return serde_json::json!({ "error": { "message": "data_dir is required" } });
    }
    let mut config = MOBILE_CONFIG.lock().unwrap();
    *config = Some(MobileConfig {
        data_dir: data_dir.to_string(),
    });
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
    invoke_mobile_protocol_json(request, data_dir.as_deref())
}

fn event_envelope(name: &str, detail: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "event": name, "detail": detail })
}

fn emit_event(name: &str, detail: serde_json::Value) -> bool {
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
    let _ = env.call_static_method(
        "jp/nonbili/meron/MeronCoreNative",
        "dispatchCoreEventFromNative",
        "(Ljava/lang/String;)V",
        &[JValue::Object(&event)],
    );
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
