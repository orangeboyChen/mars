// Tencent is pleased to support the open source community by making Mars available.
// Copyright (C) 2016 THL A29 Limited, a Tencent company. All rights reserved.
//
// Licensed under the MIT License (the "License"); you may not use this file except
// compliance with the License. You may obtain a copy of the License at
// http://opensource.org/licenses/MIT

import Foundation

// The XCFramework slice is the Rust static library `libmars_xlog_ffi.a`; its
// umbrella header declares the module `MarsXlogC`. There is no Objective-C
// (and no C++) between Swift and the Rust implementation any more.
import MarsXlogC

/// Log levels, the counterpart of `TLogLevel` / `MarsLogLevel`.
public enum MarsXlogLevel: Int32 {
    case verbose = 0
    case debug = 1
    case info = 2
    case warning = 3
    case error = 4
    case fatal = 5
    /// Filter out every record.
    case none = 6

    /// `kLevelAll == kLevelVerbose == 0` in the C++ enum.
    public static var all: MarsXlogLevel { .verbose }
}

/// Async (default) or sync appender.
public enum MarsXlogAppenderMode: Int32 {
    case async = 0
    case sync = 1
}

/// Compression used when a log file is converted.
public enum MarsXlogCompressMode: Int32 {
    case zlib = 0
    case zstd = 1
}

/// Configuration of `MarsXlog.open(_:)`, the counterpart of `mars::xlog::XLogConfig`.
public final class MarsXlogOpenConfig {
    /// Directory the mmap log files are written to. Must exist.
    public var logDir: String
    /// Cache directory used by the appender. Optional.
    public var cacheDir: String?
    /// Prefix of the log file name, e.g. "Ham".
    public var namePrefix: String?
    /// Public key used to encrypt the log files. Optional.
    public var pubKey: String?
    /// Async (default) or sync appender.
    public var mode: MarsXlogAppenderMode
    /// Compression used when a log file is converted, zlib (default) or zstd.
    public var compressMode: MarsXlogCompressMode
    /// Level applied right after open, `.info` by default.
    public var level: MarsXlogLevel
    /// Mirror the log to the console (Xcode console).
    public var consoleLogEnabled: Bool

    public init(logDir: String,
                cacheDir: String? = nil,
                namePrefix: String? = nil,
                pubKey: String? = nil,
                mode: MarsXlogAppenderMode = .async,
                compressMode: MarsXlogCompressMode = .zlib,
                level: MarsXlogLevel = .info,
                consoleLogEnabled: Bool = false) {
        self.logDir = logDir
        self.cacheDir = cacheDir
        self.namePrefix = namePrefix
        self.pubKey = pubKey
        self.mode = mode
        self.compressMode = compressMode
        self.level = level
        self.consoleLogEnabled = consoleLogEnabled
    }
}

/// Swift wrapper of the Rust xlog implementation.
public enum MarsXlog {

    /// Open the appender. Has to be called before any log is written.
    ///
    /// - Returns: `false` when the Rust appender rejected the configuration.
    @discardableResult
    public static func open(_ config: MarsXlogOpenConfig) -> Bool {
        // `withCString` keeps every pointer alive across the call; `nil` is the
        // "empty" of the C ABI, which the header documents per field.
        let opened = logDirPtr(config.logDir) { logDir in
            cacheDirPtr(config.cacheDir) { cacheDir in
                namePrefixPtr(config.namePrefix) { namePrefix in
                    pubKeyPtr(config.pubKey) { pubKey in
                        var cConfig = MarsXLogConfig(mode: config.mode.rawValue,
                                                     log_dir: logDir,
                                                     name_prefix: namePrefix,
                                                     pub_key: pubKey,
                                                     compress_mode: config.compressMode.rawValue,
                                                     compress_level: 0,
                                                     cache_dir: cacheDir,
                                                     cache_days: 0)
                        return withUnsafePointer(to: &cConfig) { mars_xlog_open($0) }
                    }
                }
            }
        }
        guard opened == MARS_XLOG_OK else { return false }
        // `mars_xlog_open` carries no level, exactly like the C++
        // `appender_open`: it has to be applied afterwards.
        mars_xlog_set_level(config.level.rawValue)
        mars_xlog_set_console_log(config.consoleLogEnabled ? 1 : 0)
        return true
    }

    /// Open with the defaults: async appender, zlib, info level, console off.
    @discardableResult
    public static func open(logDir: String, namePrefix: String) -> Bool {
        open(MarsXlogOpenConfig(logDir: logDir, namePrefix: namePrefix))
    }

    /// Flush and close the appender.
    public static func close() {
        mars_xlog_close()
    }

    /// Flush asynchronously.
    public static func flush() {
        mars_xlog_flush()
    }

    /// Flush synchronously.
    public static func flushSync() {
        mars_xlog_flush_sync()
    }

    /// Filter out every log below `level`.
    public static func setLevel(_ level: MarsXlogLevel) {
        mars_xlog_set_level(level.rawValue)
    }

    public static func setConsoleLogEnabled(_ enabled: Bool) {
        mars_xlog_set_console_log(enabled ? 1 : 0)
    }

    /// Split the log file once a single file grows past `maxByteSize`, 0 means never split.
    public static func setMaxFileSize(_ maxByteSize: UInt64) {
        mars_xlog_set_max_file_size(maxByteSize)
    }

    /// Max alive duration of a single log file in seconds, 10 days by default.
    public static func setMaxAliveDuration(_ maxAliveDuration: Int) {
        mars_xlog_set_max_alive_duration(Int64(maxAliveDuration))
    }

    /// Mark `path` as "do not back up" (`com.apple.MobileBackup`), which is what
    /// apps do with their log and cache directories.
    ///
    /// - Returns: `false` if the attribute could not be set.
    @discardableResult
    public static func setExcludedFromBackup(_ exclude: Bool, forPath path: String) -> Bool {
        var url = URL(fileURLWithPath: path, isDirectory: true)
        do {
            var values = URLResourceValues()
            values.isExcludedFromBackup = exclude
            try url.setResourceValues(values)
            return true
        } catch {
            return false
        }
    }

    /// Write one log record with full source location.
    public static func log(level: MarsXlogLevel,
                           module: String,
                           file: String? = nil,
                           line: Int = 0,
                           function: String? = nil,
                           message: String) {
        write(level: level, module: module, file: file, line: line, function: function, message: message)
    }

    /// Write one log record.
    public static func log(level: MarsXlogLevel,
                           module: String,
                           function: String? = nil,
                           message: String) {
        write(level: level, module: module, file: nil, line: 0, function: function, message: message)
    }

    /// Write one log record without source location.
    public static func log(level: MarsXlogLevel, module: String, message: String) {
        write(level: level, module: module, file: nil, line: 0, function: nil, message: message)
    }

    public static func info(module: String, function: String? = nil, message: String) {
        write(level: .info, module: module, file: nil, line: 0, function: function, message: message)
    }

    public static func warning(module: String, function: String? = nil, message: String) {
        write(level: .warning, module: module, file: nil, line: 0, function: function, message: message)
    }

    public static func error(module: String, function: String? = nil, message: String) {
        write(level: .error, module: module, file: nil, line: 0, function: function, message: message)
    }

    public static func fatal(module: String, function: String? = nil, message: String) {
        write(level: .fatal, module: module, file: nil, line: 0, function: function, message: message)
    }

    // MARK: - C bridging

    private static func write(level: MarsXlogLevel,
                              module: String,
                              file: String?,
                              line: Int,
                              function: String?,
                              message: String) {
        module.withCString { tag in
            filePtr(file) { file in
                functionPtr(function) { function in
                    message.withCString { message in
                        mars_xlog_write(level.rawValue, tag, file, function, Int32(line), message)
                    }
                }
            }
        }
    }

    private static func logDirPtr<T>(_ value: String, _ body: (UnsafePointer<CChar>) -> T) -> T {
        value.withCString(body)
    }

    private static func cacheDirPtr<T>(_ value: String?, _ body: (UnsafePointer<CChar>?) -> T) -> T {
        optionalCString(value, body)
    }

    private static func namePrefixPtr<T>(_ value: String?, _ body: (UnsafePointer<CChar>?) -> T) -> T {
        optionalCString(value, body)
    }

    private static func pubKeyPtr<T>(_ value: String?, _ body: (UnsafePointer<CChar>?) -> T) -> T {
        optionalCString(value, body)
    }

    private static func filePtr<T>(_ value: String?, _ body: (UnsafePointer<CChar>?) -> T) -> T {
        optionalCString(value, body)
    }

    private static func functionPtr<T>(_ value: String?, _ body: (UnsafePointer<CChar>?) -> T) -> T {
        optionalCString(value, body)
    }

    private static func optionalCString<T>(_ value: String?,
                                           _ body: (UnsafePointer<CChar>?) -> T) -> T {
        guard let value else { return body(nil) }
        return value.withCString { body($0) }
    }
}
