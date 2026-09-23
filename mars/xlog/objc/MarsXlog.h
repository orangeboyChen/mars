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
//  Objective-C interface of mars xlog, used by the Swift Package (binary target)
//  so that Swift code can `import MarsXlog` without C++ interop.
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

/// Objective-C wrapper of `mars::xlog`, the high performance logging module of Mars.
@interface MarsXlog : NSObject

/// Open the log appender. Must be called before writing any log.
/// @param logDir    directory where the mmap log files are stored, must exist.
/// @param namePrefix  prefix of the log file name, e.g. "Test".
/// @param mode      async (recommended) or sync.
/// @param compressMode zlib (default) or zstd, only used when the log file is converted.
+ (void)openWithLogDir:(NSString*)logDir
            namePrefix:(NSString*)namePrefix
                  mode:(MarsXlogAppenderMode)mode
          compressMode:(MarsXlogCompressMode)compressMode NS_SWIFT_NAME(open(logDir:namePrefix:mode:compressMode:));

/// Open with the default configuration: async mode + zlib compression.
+ (void)openWithLogDir:(NSString*)logDir namePrefix:(NSString*)namePrefix NS_SWIFT_NAME(open(logDir:namePrefix:));

/// Flush and close the appender.
+ (void)close;

/// Flush asynchronously.
+ (void)flush;

/// Flush synchronously.
+ (void)flushSync;

/// Filter logs below the given level. Debug level in debug build, info level in release build is a common choice.
+ (void)setLevel:(MarsXlogLevel)level;

/// Mirror the log to the console (Xcode console / os_log).
+ (void)setConsoleLogEnabled:(BOOL)enabled;

/// Split log files once a single file is larger than maxByteSize, 0 means never split.
+ (void)setMaxFileSize:(uint64_t)maxByteSize;

/// Max alive duration of a single log file in seconds, default is 10 days.
+ (void)setMaxAliveDuration:(long)maxAliveDuration;

/// Write one log record.
+ (void)logWithLevel:(MarsXlogLevel)level
                 tag:(NSString*)tag
                file:(const char* _Nullable)file
            function:(const char* _Nullable)function
                line:(int)line
             message:(NSString*)message NS_SWIFT_NAME(log(level:tag:file:function:line:message:));

/// Write one log record without file / function / line information.
+ (void)logWithLevel:(MarsXlogLevel)level tag:(NSString*)tag message:(NSString*)message NS_SWIFT_NAME(log(level:tag:message:));

@end

NS_ASSUME_NONNULL_END
