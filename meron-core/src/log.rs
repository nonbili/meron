//! Tiny leveled logger with a pluggable sink.
//!
//! Defaults to stderr — the desktop sidecar's stderr is captured by the bridge,
//! so existing behavior is preserved. Mobile hosts install a sink (see
//! `ffi::init`) that forwards records over the event channel to os_log / Logcat,
//! where the core's `eprintln!` output would otherwise be lost.

use std::sync::RwLock;

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

/// Install the log sink (replaces any previous one). Mobile hosts call this on
/// init so core logs reach the platform logger.
pub fn set_sink(sink: Sink) {
    *SINK.write().unwrap() = Some(sink);
}

/// Emit one record. Routes to the installed sink, or stderr when none is set.
pub fn emit(level: Level, tag: &str, message: &str) {
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
        $crate::log::emit($level, $tag, &format!($($arg)*))
    };
}
