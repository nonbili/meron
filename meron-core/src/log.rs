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

/// Whether a record at [level] should be produced by this binary.
///
/// Debug builds keep verbose diagnostics. Release builds keep warnings and
/// errors only, so mobile does not spend work formatting and forwarding routine
/// trace data across FFI.
pub fn enabled(level: Level) -> bool {
    cfg!(debug_assertions) || matches!(level, Level::Warn | Level::Error)
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

#[cfg(test)]
mod tests {
    use super::{Level, enabled};

    #[test]
    fn release_policy_keeps_only_warning_and_error() {
        assert_eq!(enabled(Level::Debug), cfg!(debug_assertions));
        assert_eq!(enabled(Level::Info), cfg!(debug_assertions));
        assert!(enabled(Level::Warn));
        assert!(enabled(Level::Error));
    }
}
