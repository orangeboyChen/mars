//! `mars::comm::XloggerCategory` plus the instance table in
//! `mars/xlog/src/xlogger_interface.cc`.
//!
//! | C++                                            | Rust                     |
//! |------------------------------------------------|--------------------------|
//! | `XloggerCategory`                              | [`XloggerCategory`]      |
//! | `NewXloggerInstance` / `GetXloggerInstance` /  | [`new_xlogger_instance`] |
//! | `ReleaseXloggerInstance`                       | …                        |
//! | `XloggerWrite` / `IsEnabledFor` / `GetLevel` / | the free functions below |
//! | `SetLevel` / `Flush` / `SetConsoleLogOpen`     |                          |
//!
//! # Handles instead of pointers
//!
//! The C++ hands out `XloggerCategory*` as a `uintptr_t` and casts it back on
//! every call — any stale pointer is undefined behaviour. The port hands out an
//! opaque [`XloggerHandle`] that is looked up in the instance table, so a stale
//! handle is a no-op instead of a wild write. Handle `0` keeps its C++ meaning:
//! "the default logger", i.e. the process-wide appender.
//!
//! # Known limitation
//!
//! The C++ gives every instance its own `XloggerAppender` (so two prefixes can
//! write to two directories); the port's appender is still a process-wide
//! singleton, so every category writes through it. Multi-instance appenders are
//! a prerequisite for deleting the C++ and come in a follow-up.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::{
    appender_flush, appender_flush_sync, appender_set_console_log, appender_set_mode,
    appender_write, AppenderMode, LogLevel, XLogConfig, XLoggerInfo,
};

/// Opaque id of a [`XloggerCategory`]; `0` is the default logger.
pub type XloggerHandle = u64;

/// The default logger, i.e. "no instance" — calls go straight to the
/// process-wide appender.
pub const DEFAULT_HANDLE: XloggerHandle = 0;

/// `mars::comm::XloggerCategory`.
///
/// The C++ also stores an appender and a write callback; the port only needs
/// the level, because writing always goes through the process-wide appender
/// (see the module note).
#[derive(Debug, Clone, Copy)]
pub struct XloggerCategory {
    level: LogLevel,
}

impl Default for XloggerCategory {
    fn default() -> Self {
        Self {
            level: LogLevel::Verbose,
        }
    }
}

impl XloggerCategory {
    /// `XloggerCategory::GetLevel`.
    pub fn level(&self) -> LogLevel {
        self.level
    }

    /// `XloggerCategory::SetLevel`.
    pub fn set_level(&mut self, level: LogLevel) {
        self.level = level;
    }

    /// `XloggerCategory::IsEnabledFor` — `level_ <= _level`.
    pub fn is_enabled_for(&self, level: LogLevel) -> bool {
        (self.level as i32) <= (level as i32)
    }

    /// `XloggerCategory::Write` — the level filter and the pid/tid fix-up of
    /// `__WriteImpl`.
    ///
    /// `log` of `None` mirrors the C++ `NULL == _log`: the record is written
    /// anyway, promoted to `Fatal` with a fixed message.
    pub fn write(&self, info: Option<&XLoggerInfo>, log: Option<&str>) -> bool {
        let mut info = info.cloned();

        if let Some(info) = info.as_ref() {
            if (info.level as i32) < (self.level as i32) {
                return false;
            }
        }

        // `-1 == pid && -1 == tid && -1 == maintid` means "fill these in".
        if let Some(info) = info.as_mut() {
            if info.pid == -1 && info.tid == -1 && info.maintid == -1 {
                info.pid = std::process::id() as i64;
                info.tid = crate::sys::thread_id();
                info.maintid = crate::sys::main_thread_id();
            }
        }

        match log {
            Some(log) => appender_write(info.as_ref(), log),
            None => {
                if let Some(info) = info.as_mut() {
                    info.level = LogLevel::Fatal;
                }
                appender_write(info.as_ref(), "NULL == _log")
            }
        }
    }
}

struct Registry {
    next: XloggerHandle,
    categories: HashMap<XloggerHandle, XloggerCategory>,
    by_prefix: HashMap<String, XloggerHandle>,
    /// The logger handle `0` selects: `SetLevel(0, ..)` in the C++ configures
    /// the process-wide level, so it has to be reachable.
    default: XloggerCategory,
    /// Whether the registry itself opened the shared appender. It may only
    /// close what it opened — a caller can hold the very same singleton through
    /// `appender_open()`, and closing it out from under them silently kills
    /// every later `appender_write`.
    opened_appender: bool,
}

fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        Mutex::new(Registry {
            next: DEFAULT_HANDLE + 1,
            categories: HashMap::new(),
            by_prefix: HashMap::new(),
            default: XloggerCategory::default(),
            opened_appender: false,
        })
    })
}

/// `mars::xlog::NewXloggerInstance`.
///
/// Registers a category for `config.nameprefix` (an existing prefix returns the
/// existing handle, like the C++) and opens the appender for `config`. Returns
/// [`DEFAULT_HANDLE`] when the config has no log dir or prefix, matching the
/// C++ `nullptr`.
pub fn new_xlogger_instance(config: &XLogConfig, level: LogLevel) -> XloggerHandle {
    if config.logdir.as_os_str().is_empty() || config.nameprefix.is_empty() {
        return DEFAULT_HANDLE;
    }

    // Fast path: already registered.
    if let Some(handle) = registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .by_prefix
        .get(&config.nameprefix)
        .copied()
    {
        return handle;
    }

    // The appender is a process-wide singleton: the first caller opens it and
    // every later prefix shares it (see the module note). Opening it does
    // `create_dir_all`, an mmap and several writes, so it happens outside the
    // registry lock — otherwise every other logger in the process stalls for
    // the whole of that.
    let opened = if crate::appender_get_current_log_path().is_none() {
        match crate::appender_open(config.clone()) {
            Ok(()) => true,
            Err(_) => return DEFAULT_HANDLE,
        }
    } else {
        false
    };

    let mut registry = registry().lock().unwrap_or_else(|e| e.into_inner());
    // Record the ownership first: another thread may have registered the
    // prefix while the lock was free, and returning early must not lose the
    // fact that *we* opened the appender — otherwise nothing ever closes it.
    if opened {
        registry.opened_appender = true;
    }
    if let Some(handle) = registry.by_prefix.get(&config.nameprefix).copied() {
        return handle;
    }
    let handle = registry.next;
    registry.next += 1;
    let mut category = XloggerCategory::default();
    category.set_level(level);
    registry.categories.insert(handle, category);
    registry.by_prefix.insert(config.nameprefix.clone(), handle);
    handle
}

/// `mars::xlog::GetXloggerInstance` — the handle registered for `_nameprefix`.
pub fn get_xlogger_instance(nameprefix: &str) -> XloggerHandle {
    registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .by_prefix
        .get(nameprefix)
        .copied()
        .unwrap_or(DEFAULT_HANDLE)
}

/// `mars::xlog::ReleaseXloggerInstance`.
pub fn release_xlogger_instance(nameprefix: &str) {
    let mut registry = registry().lock().unwrap_or_else(|e| e.into_inner());
    let Some(handle) = registry.by_prefix.remove(nameprefix) else {
        return;
    };
    registry.categories.remove(&handle);

    // Only the last instance closes the appender, and only if this registry is
    // the one that opened it. The close happens with the lock still held so
    // that a concurrent `new_xlogger_instance` cannot slip in between the
    // decision and the close and end up with an appender that is gone.
    if registry.categories.is_empty() && registry.opened_appender {
        registry.opened_appender = false;
        crate::appender_close();
    }
}

/// Looks a category up and returns a copy, so the caller never runs with the
/// registry lock held — writing a record does file I/O.
fn lookup(handle: XloggerHandle) -> Option<XloggerCategory> {
    registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .categories
        .get(&handle)
        .copied()
}

/// A copy of the default logger (handle `0`).
fn default_category() -> XloggerCategory {
    registry().lock().unwrap_or_else(|e| e.into_inner()).default
}

fn with_category_mut(handle: XloggerHandle, f: impl FnOnce(&mut XloggerCategory)) {
    if let Some(category) = registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .categories
        .get_mut(&handle)
    {
        f(category);
    }
}

/// `mars::xlog::XloggerWrite`.
///
/// Handle `0` uses the default logger. An unknown non-zero handle (one whose
/// instance was released) writes nothing: the module promises that a stale
/// handle is a no-op, not a fall back to the default logger.
pub fn xlogger_write(handle: XloggerHandle, info: Option<&XLoggerInfo>, log: Option<&str>) -> bool {
    let category = if handle == DEFAULT_HANDLE {
        default_category()
    } else {
        match lookup(handle) {
            Some(category) => category,
            None => return false,
        }
    };
    category.write(info, log)
}

/// `mars::xlog::IsEnabledFor`.
///
/// `false` for an unknown non-zero handle, so nothing is written through it.
pub fn is_enabled_for(handle: XloggerHandle, level: LogLevel) -> bool {
    let category = if handle == DEFAULT_HANDLE {
        default_category()
    } else {
        match lookup(handle) {
            Some(category) => category,
            None => return false,
        }
    };
    category.is_enabled_for(level)
}

/// `mars::xlog::GetLevel`.
///
/// `None` for an unknown non-zero handle.
pub fn get_level(handle: XloggerHandle) -> Option<LogLevel> {
    if handle == DEFAULT_HANDLE {
        return Some(default_category().level());
    }
    lookup(handle).map(|category| category.level())
}

/// `mars::xlog::SetLevel`.
pub fn set_level(handle: XloggerHandle, level: LogLevel) {
    if handle == DEFAULT_HANDLE {
        registry()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .default
            .set_level(level);
        return;
    }
    with_category_mut(handle, |category| category.set_level(level));
}

/// `mars::xlog::SetAppenderMode`.
pub fn set_appender_mode(handle: XloggerHandle, mode: AppenderMode) {
    // The C++ reaches into the instance's appender; the port has one appender.
    let _ = handle;
    appender_set_mode(mode);
}

/// `mars::xlog::Flush`.
pub fn flush(handle: XloggerHandle, sync: bool) {
    let _ = handle;
    if sync {
        appender_flush_sync();
    } else {
        appender_flush();
    }
}

/// `mars::xlog::FlushAll`.
pub fn flush_all(sync: bool) {
    flush(DEFAULT_HANDLE, sync);
}

/// `mars::xlog::SetConsoleLogOpen`.
pub fn set_console_log_open(handle: XloggerHandle, open: bool) {
    let _ = handle;
    appender_set_console_log(open);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_lock::serial;

    fn config(prefix: &str, dir: &std::path::Path) -> XLogConfig {
        XLogConfig {
            logdir: dir.to_path_buf(),
            nameprefix: prefix.to_owned(),
            ..XLogConfig::default()
        }
    }

    #[test]
    fn instance_table_is_keyed_by_prefix() {
        let _guard = serial();
        let dir = tempfile::tempdir().unwrap();
        let first = new_xlogger_instance(&config("p", dir.path()), LogLevel::Info);
        assert_ne!(first, DEFAULT_HANDLE);
        assert_eq!(get_xlogger_instance("p"), first);
        // A second call with the same prefix returns the existing handle.
        assert_eq!(
            new_xlogger_instance(&config("p", dir.path()), LogLevel::Info),
            first
        );
        assert_eq!(get_xlogger_instance("missing"), DEFAULT_HANDLE);

        release_xlogger_instance("p");
        assert_eq!(get_xlogger_instance("p"), DEFAULT_HANDLE);

        // The appender is a process-wide singleton and the other tests in this
        // crate assume it is closed, so put it back.
        crate::appender_close();
    }

    #[test]
    fn empty_logdir_or_prefix_is_rejected() {
        let _guard = serial();
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            new_xlogger_instance(&config("", dir.path()), LogLevel::Info),
            DEFAULT_HANDLE
        );
        assert_eq!(
            new_xlogger_instance(&config("prefix", std::path::Path::new("")), LogLevel::Info),
            DEFAULT_HANDLE
        );
    }

    #[test]
    fn a_second_prefix_shares_the_open_appender() {
        let _guard = serial();
        let dir = tempfile::tempdir().unwrap();
        let first = new_xlogger_instance(&config("one", dir.path()), LogLevel::Info);
        let second = new_xlogger_instance(&config("two", dir.path()), LogLevel::Warn);
        assert_ne!(first, DEFAULT_HANDLE);
        assert_ne!(second, DEFAULT_HANDLE);
        assert_ne!(first, second);
        assert_eq!(get_level(first), Some(LogLevel::Info));
        assert_eq!(get_level(second), Some(LogLevel::Warn));

        release_xlogger_instance("one");
        release_xlogger_instance("two");
        // The last release closes the shared appender, so the lifecycle can be
        // repeated.
        let again = new_xlogger_instance(&config("one", dir.path()), LogLevel::Info);
        assert_ne!(again, DEFAULT_HANDLE);
        crate::appender_close();
    }

    #[test]
    fn a_stale_handle_writes_nothing() {
        let _guard = serial();
        // A handle that was never registered behaves exactly like one whose
        // instance has been released: no write, no level, no fallback.
        const STALE: XloggerHandle = 999;
        assert!(!xlogger_write(STALE, None, Some("dropped")));
        assert!(!is_enabled_for(STALE, LogLevel::Fatal));
        assert_eq!(get_level(STALE), None);
    }

    #[test]
    fn the_default_handle_has_its_own_level() {
        let _guard = serial();
        set_level(DEFAULT_HANDLE, LogLevel::Error);
        assert_eq!(get_level(DEFAULT_HANDLE), Some(LogLevel::Error));
        assert!(!is_enabled_for(DEFAULT_HANDLE, LogLevel::Warn));
        assert!(is_enabled_for(DEFAULT_HANDLE, LogLevel::Error));
        set_level(DEFAULT_HANDLE, LogLevel::Verbose);
        assert!(is_enabled_for(DEFAULT_HANDLE, LogLevel::Verbose));
    }

    #[test]
    fn level_filter_follows_the_cpp_rule() {
        let mut category = XloggerCategory::default();
        category.set_level(LogLevel::Warn);
        assert!(category.is_enabled_for(LogLevel::Error));
        assert!(category.is_enabled_for(LogLevel::Warn));
        assert!(!category.is_enabled_for(LogLevel::Info));
        assert!(!category.is_enabled_for(LogLevel::Verbose));
        assert_eq!(get_level(12345), None, "unknown handle");
    }
}
