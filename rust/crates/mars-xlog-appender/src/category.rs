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
    appender_flush, appender_flush_sync, appender_open, appender_set_console_log,
    appender_set_mode, appender_write, AppenderMode, LogLevel, XLogConfig, XLoggerInfo,
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
                info.maintid = info.tid;
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
}

fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        Mutex::new(Registry {
            next: DEFAULT_HANDLE + 1,
            categories: HashMap::new(),
            by_prefix: HashMap::new(),
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

    let mut registry = registry().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(handle) = registry.by_prefix.get(&config.nameprefix) {
        return *handle;
    }

    // The C++ creates the appender instance here and only opens it once; the
    // port's appender is a singleton, so this is the open call.
    if appender_open(config.clone()).is_err() {
        return DEFAULT_HANDLE;
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
    if let Some(handle) = registry.by_prefix.remove(nameprefix) {
        registry.categories.remove(&handle);
    }
}

fn with_category<R>(handle: XloggerHandle, f: impl FnOnce(&XloggerCategory) -> R) -> Option<R> {
    registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .categories
        .get(&handle)
        .map(f)
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
pub fn xlogger_write(handle: XloggerHandle, info: Option<&XLoggerInfo>, log: Option<&str>) -> bool {
    match with_category(handle, |category| category.write(info, log)) {
        Some(written) => written,
        // Handle 0 (or a stale one): the process-wide appender, no level filter.
        None => appender_write(info, log.unwrap_or("NULL == _log")),
    }
}

/// `mars::xlog::IsEnabledFor`.
pub fn is_enabled_for(handle: XloggerHandle, level: LogLevel) -> bool {
    with_category(handle, |category| category.is_enabled_for(level)).unwrap_or(true)
}

/// `mars::xlog::GetLevel`.
pub fn get_level(handle: XloggerHandle) -> LogLevel {
    with_category(handle, |category| category.level()).unwrap_or(LogLevel::Verbose)
}

/// `mars::xlog::SetLevel`.
pub fn set_level(handle: XloggerHandle, level: LogLevel) {
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

    fn config(prefix: &str, dir: &std::path::Path) -> XLogConfig {
        XLogConfig {
            logdir: dir.to_path_buf(),
            nameprefix: prefix.to_owned(),
            ..XLogConfig::default()
        }
    }

    #[test]
    fn instance_table_is_keyed_by_prefix() {
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
    fn level_filter_follows_the_cpp_rule() {
        let mut category = XloggerCategory::default();
        category.set_level(LogLevel::Warn);
        assert!(category.is_enabled_for(LogLevel::Error));
        assert!(category.is_enabled_for(LogLevel::Warn));
        assert!(!category.is_enabled_for(LogLevel::Info));
        assert!(!category.is_enabled_for(LogLevel::Verbose));
        assert_eq!(get_level(12345), LogLevel::Verbose, "unknown handle");
    }
}
