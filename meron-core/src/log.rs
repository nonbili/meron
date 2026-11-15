//! Tiny leveled logger with a pluggable sink.
//!
//! Defaults to stderr — the desktop sidecar's stderr is captured by the bridge,
//! so existing behavior is preserved. Mobile hosts install a sink (see
//! `ffi::init`) that forwards records over the event channel to os_log / Logcat,
//! where the core's `eprintln!` output would otherwise be lost.

use std::sync::RwLock;

/// Diagnostic guard for selected database operations. Release the mutex before
/// logging so a slow log sink does not extend the database critical section.
pub struct TimedDbGuard<'a> {
    guard: Option<std::sync::MutexGuard<'a, rusqlite::Connection>>,
    acquired: std::time::Instant,
    wait: std::time::Duration,
    operation: &'static str,
}

pub fn timed_db_lock<'a>(
    db: &'a std::sync::Mutex<rusqlite::Connection>,
    operation: &'static str,
) -> TimedDbGuard<'a> {
    let started = std::time::Instant::now();
    let guard = db.lock().unwrap();
    TimedDbGuard {
        guard: Some(guard),
        acquired: std::time::Instant::now(),
        wait: started.elapsed(),
        operation,
    }
}

impl std::ops::Deref for TimedDbGuard<'_> {
    type Target = rusqlite::Connection;

    fn deref(&self) -> &Self::Target {
        self.guard.as_deref().unwrap()
    }
}

impl std::ops::DerefMut for TimedDbGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.as_deref_mut().unwrap()
    }
}

impl Drop for TimedDbGuard<'_> {
    fn drop(&mut self) {
        let held = self.acquired.elapsed();
        drop(self.guard.take());
        if let Some(level) = slow_level(held.max(self.wait)) {
            crate::mlog!(
                level,
                "db.timing",
                "operation={} lock_wait_ms={} held_ms={}",
                self.operation,
                self.wait.as_millis(),
                held.as_millis()
            );
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        }
    }
}

type Sink = Box<dyn Fn(Level, &str, &str) + Send + Sync>;

/// Process-global log sink. `None` falls back to stderr.
static SINK: RwLock<Option<Sink>> = RwLock::new(None);

/// Level for a timing diagnostic: none under 100ms, Debug (development builds
/// only) up to a second, Warn past that, where a user would notice the delay.
pub fn slow_level(elapsed: std::time::Duration) -> Option<Level> {
    match elapsed.as_millis() {
        0..100 => None,
        100..1000 => Some(Level::Debug),
        _ => Some(Level::Warn),
    }
}

/// Whether a record at [level] should be produced by this binary.
///
/// Debug builds keep verbose diagnostics. Release builds drop Debug, so mobile
/// does not spend work formatting and forwarding routine trace data across FFI,
/// but keep Info: the low-volume lifecycle records (a mail watcher connecting,
/// new mail arriving) a sync problem can only be diagnosed from after the fact.
pub fn enabled(level: Level) -> bool {
    cfg!(debug_assertions) || !matches!(level, Level::Debug)
}

/// Install the log sink (replaces any previous one). Mobile hosts call this on
/// init so core logs reach the platform logger.
pub fn set_sink(sink: Sink) {
    *SINK.write().unwrap() = Some(sink);
}

/// Emit one record. Routes to the installed sink, or stderr when none is set.
pub fn emit(level: Level, tag: &str, message: &str) {
    if !enabled(level) {
        return;
    }
    if let Some(sink) = SINK.read().unwrap().as_ref() {
        sink(level, tag, message);
    } else {
        eprintln!("meron-core [{}] {tag}: {message}", level.as_str());
    }
}

/// `mlog!(Level::Warn, "tag", "text {}", value)` — formats lazily and routes
/// through [`emit`]. Prefer this over `eprintln!` in core so mobile sees it too.
#[macro_export]
macro_rules! mlog {
    ($level:expr, $tag:expr, $($arg:tt)*) => {
        {
            let level = $level;
            if $crate::log::enabled(level) {
                $crate::log::emit(level, $tag, &format!($($arg)*));
            }
        }
    };
}

/// Route panics through [`emit`] so they survive where stderr does not: on
/// mobile the host forwards them to Logcat / os_log and the shareable
/// diagnostic log, on desktop the sidecar's stderr already lands in meron.log.
/// The previous hook still runs afterwards, keeping the default backtrace.
///
/// Idempotent: repeated calls (each mobile `ffi::init`) install the hook once.
pub fn install_panic_hook() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let location = info
                .location()
                .map(|l| format!("{}:{}", l.file(), l.line()))
                .unwrap_or_else(|| "unknown location".to_string());
            let message = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panic".to_string());
            let thread = std::thread::current();
            let thread = thread.name().unwrap_or("unnamed").to_string();
            emit(
                Level::Error,
                "panic",
                &format!("core panic in thread '{thread}' at {location}: {message}"),
            );
            previous(info);
        }));
    });
}

#[cfg(test)]
mod tests {
    use super::{Level, enabled};

    #[test]
    fn release_policy_drops_only_debug() {
        assert_eq!(enabled(Level::Debug), cfg!(debug_assertions));
        assert!(enabled(Level::Info));
        assert!(enabled(Level::Warn));
        assert!(enabled(Level::Error));
    }

    #[test]
    fn timing_is_only_a_warning_once_a_user_would_notice() {
        use super::slow_level;
        use std::time::Duration;
        assert_eq!(slow_level(Duration::from_millis(99)), None);
        assert_eq!(slow_level(Duration::from_millis(700)), Some(Level::Debug));
        assert_eq!(slow_level(Duration::from_millis(1000)), Some(Level::Warn));
    }
}
