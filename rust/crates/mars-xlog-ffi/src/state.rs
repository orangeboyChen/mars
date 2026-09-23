//! Process-wide knobs that have no counterpart in `mars-xlog-appender`.
//!
//! Two groups of state live here:
//!
//! 1. the **level filter** ([`set_min_level`] / [`level_enabled`]), which is the
//!    port of `xlogger_SetLevel` / `xlogger_IsEnabledFor` from
//!    `mars/comm/xlogger/xlogger.cc`. The appender itself is level-agnostic
//!    (it formats whatever it is handed), so the gate belongs to the seam.
//! 2. the **identity fields** of `XLoggerInfo` (`pid` / `tid` / `maintid` /
//!    `timeval`) that the C++ filled in from `xlogger_pid()` and friends.
//!
//! Everything is lock-free and `Send + Sync`; there is no `unsafe` here.

use std::sync::atomic::{AtomicI32, AtomicI64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Level below which records are dropped. Mirrors the C++ global in
/// `xlogger.cc`; `MarsLevelVerbose` (0) means "log everything".
static MIN_LEVEL: AtomicI32 = AtomicI32::new(0);

/// Source of the thread ids reported in `XLoggerInfo::tid`. Rust has no portable
/// numeric thread id, so ids are handed out in "first thread to log wins" order,
/// starting at 1.
static NEXT_TID: AtomicI64 = AtomicI64::new(1);

/// Lazily becomes the id of the first thread that logs, mirroring
/// `xlogger_maintid()`.
static MAIN_TID: OnceLock<i64> = OnceLock::new();

thread_local! {
    /// Id of the calling thread, assigned on first use.
    static TID: i64 = {
        let tid = NEXT_TID.fetch_add(1, Ordering::Relaxed);
        // Best-effort: the first thread to log is treated as the main thread.
        let _ = MAIN_TID.set(tid);
        tid
    };
}

/// Sets the minimum level that [`level_enabled`] lets through.
///
/// Levels below `kLevelVerbose` (i.e. negative) clamp to "log everything"; any
/// value above `kLevelNone` (6) disables logging altogether, exactly like
/// `xlogger_SetLevel(kLevelNone)`.
pub fn set_min_level(level: i32) {
    MIN_LEVEL.store(level.max(0), Ordering::Relaxed);
}

/// The currently configured minimum level.
pub fn min_level() -> i32 {
    MIN_LEVEL.load(Ordering::Relaxed)
}

/// Port of `xlogger_IsEnabledFor(TLogLevel)`: `true` when `level` is at or above
/// the configured minimum.
pub fn level_enabled(level: i32) -> bool {
    level >= min_level()
}

/// `xlogger_pid()` — the OS process id.
pub fn pid() -> i64 {
    i64::from(std::process::id() as i32)
}

/// `xlogger_tid()` — a stable per-thread number, assigned on first use.
pub fn tid() -> i64 {
    TID.with(|tid| *tid)
}

/// `xlogger_maintid()` — the id of the first thread that logged. Falls back to
/// the calling thread's id when nothing has been logged yet, so the field is
/// never 0.
pub fn main_tid() -> i64 {
    *MAIN_TID.get_or_init(tid)
}

/// `gettimeofday(&info.timeval, NULL)` — seconds + microseconds since the epoch.
pub fn now_timeval() -> (i64, i64) {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => (d.as_secs() as i64, d.subsec_micros() as i64),
        // The system clock is before 1970 (or absurdly skewed); report the epoch
        // rather than failing the write.
        Err(_) => (0, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_logs_everything() {
        set_min_level(0);
        for level in 0..=5 {
            assert!(
                level_enabled(level),
                "level {level} should pass the default filter"
            );
        }
    }

    #[test]
    fn higher_level_gates_lower_records() {
        set_min_level(3);
        assert!(!level_enabled(0));
        assert!(!level_enabled(2));
        assert!(level_enabled(3));
        assert!(level_enabled(5));
        set_min_level(0);
    }

    #[test]
    fn level_none_disables_everything() {
        set_min_level(6);
        for level in 0..=5 {
            assert!(
                !level_enabled(level),
                "level {level} should be filtered out"
            );
        }
        set_min_level(0);
    }

    #[test]
    fn negative_level_clamps_to_verbose() {
        set_min_level(-7);
        assert_eq!(min_level(), 0);
        assert!(level_enabled(0));
    }

    #[test]
    fn identity_fields_are_sane() {
        assert!(pid() > 0);
        let this_tid = tid();
        assert!(this_tid >= 1);
        assert_eq!(
            this_tid,
            tid(),
            "the thread id must be stable within a thread"
        );
        assert!(main_tid() >= 1);

        let (sec, usec) = now_timeval();
        assert!(sec > 1_600_000_000, "clock looks wrong: {sec}");
        assert!(usec < 1_000_000);
    }

    #[test]
    fn tid_is_per_thread() {
        let first = tid();
        let other = std::thread::spawn(tid).join().unwrap();
        assert_ne!(first, other);
    }
}
