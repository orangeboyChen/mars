// Tencent is pleased to support the open source community by making Mars available.
// Copyright (C) 2016 THL A29 Limited, a Tencent company. All rights reserved.

// Licensed under the MIT License (the "License"); you may not use this file except in
// compliance with the License. You may obtain a copy of the License at
// http://opensource.org/licenses/MIT

// Unless required by applicable law or agreed to in writing, software distributed under the License is
// distributed on an "AS IS" basis, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND,
// either express or implied. See the License for the specific language governing permissions and
// limitations under the License.

//
//  MarsXlog.h
//  Objective-C interface of mars xlog.
//
//  It is the public API of the MarsXlog Swift Package (binary target) and mirrors
//  the xlog API used by app code (XLogConfig + appender_open/close/flush +
//  xlogger_SetLevel + appender_set_console_log), so Swift can log without C++
//  interop:
//
//      import MarsXlog
//
//      let config = MarsXlogOpenConfig()
//      config.logDir = logDir
//      config.cacheDir = cacheDir
//      config.namePrefix = "Ham"
//      config.pubKey = "..."
//      MarsXlog.open(config)
//
//      MarsXlog.info(module: "Net", function: #function, message: "hello")
//      MarsXlog.flush()
//

#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

typedef NS_ENUM(NSUInteger, MarsXlogLevel) {
    MarsXlogLevelAll = 0,
    MarsXlogLevelVerbose = 0,
    MarsXlogLevelDebug,
    MarsXlogLevelInfo,
    MarsXlogLevelWarning,
    MarsXlogLevelError,
    MarsXlogLevelFatal,
    MarsXlogLevelNone,
};

typedef NS_ENUM(NSUInteger, MarsXlogAppenderMode) {
    MarsXlogAppenderModeAsync = 0,
    MarsXlogAppenderModeSync,
};

typedef NS_ENUM(NSUInteger, MarsXlogCompressMode) {
    MarsXlogCompressModeZlib = 0,
    MarsXlogCompressModeZstd,
};

/// Configuration of `+[MarsXlog openWithConfig:]`, the counterpart of `mars::xlog::XLogConfig`.
@interface MarsXlogOpenConfig : NSObject

/// Directory the mmap log files are written to. Must exist.
@property (nonatomic, copy) NSString* logDir;
/// Cache directory used by the appender. Optional.
@property (nonatomic, copy) NSString* cacheDir;
/// Prefix of the log file name, e.g. "Ham".
@property (nonatomic, copy) NSString* namePrefix;
/// Public key used to encrypt the log files. Optional.
@property (nonatomic, copy) NSString* pubKey;
/// Async (default) or sync appender.
@property (nonatomic, assign) MarsXlogAppenderMode mode;
/// Compression used when a log file is converted, zlib (default) or zstd.
@property (nonatomic, assign) MarsXlogCompressMode compressMode;
/// Level applied right after open, `MarsXlogLevelInfo` by default.
@property (nonatomic, assign) MarsXlogLevel level;
/// Mirror the log to the console (Xcode console / os_log).
@property (nonatomic, assign) BOOL consoleLogEnabled;

@end

/// Objective-C wrapper of `mars::xlog`.
@interface MarsXlog : NSObject

/// Open the appender. Has to be called before any log is written.
+ (void)openWithConfig:(MarsXlogOpenConfig*)config NS_SWIFT_NAME(open(_:));

/// Open with the defaults: async appender, zlib, info level, console off.
+ (void)openWithLogDir:(NSString*)logDir namePrefix:(NSString*)namePrefix NS_SWIFT_NAME(open(logDir:namePrefix:));

/// Flush and close the appender.
+ (void)close;

/// Flush asynchronously.
+ (void)flush;

/// Flush synchronously.
+ (void)flushSync;

/// Filter out every log below `level`.
+ (void)setLevel:(MarsXlogLevel)level;

+ (void)setConsoleLogEnabled:(BOOL)enabled;

/// Split the log file once a single file grows past `maxByteSize`, 0 means never split.
+ (void)setMaxFileSize:(uint64_t)maxByteSize;

/// Max alive duration of a single log file in seconds, 10 days by default.
+ (void)setMaxAliveDuration:(long)maxAliveDuration;

/// Mark `path` as "do not back up" (`com.apple.MobileBackup`), which is what
/// apps do with their log and cache directories. Returns NO if the attribute
/// could not be set.
+ (BOOL)setExcludedFromBackup:(BOOL)exclude forPath:(NSString*)path NS_SWIFT_NAME(setExcludedFromBackup(_:forPath:));

/// Write one log record with full source location.
+ (void)logWithLevel:(MarsXlogLevel)level
              module:(NSString*)module
                file:(const char* _Nullable)file
                line:(int)line
            function:(const char* _Nullable)function
             message:(NSString*)message NS_SWIFT_NAME(log(level:module:file:line:function:message:));

/// Write one log record.
+ (void)logWithLevel:(MarsXlogLevel)level
              module:(NSString*)module
            function:(NSString* _Nullable)function
             message:(NSString*)message NS_SWIFT_NAME(log(level:module:function:message:));

/// Write one log record without source location.
+ (void)logWithLevel:(MarsXlogLevel)level
              module:(NSString*)module
             message:(NSString*)message NS_SWIFT_NAME(log(level:module:message:));

+ (void)infoWithModule:(NSString*)module
              function:(NSString* _Nullable)function
               message:(NSString*)message NS_SWIFT_NAME(info(module:function:message:));
+ (void)warningWithModule:(NSString*)module
                 function:(NSString* _Nullable)function
                  message:(NSString*)message NS_SWIFT_NAME(warning(module:function:message:));
+ (void)errorWithModule:(NSString*)module
               function:(NSString* _Nullable)function
                message:(NSString*)message NS_SWIFT_NAME(error(module:function:message:));
+ (void)fatalWithModule:(NSString*)module
               function:(NSString* _Nullable)function
                message:(NSString*)message NS_SWIFT_NAME(fatal(module:function:message:));

@end

NS_ASSUME_NONNULL_END
