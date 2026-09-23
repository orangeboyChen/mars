// Tencent is pleased to support the open source community by making Mars available.
// Copyright (C) 2016 THL A29 Limited, a Tencent company. All rights reserved.

// Licensed under the MIT License (the "License"); you may not use this file except in
// compliance with the License. You may obtain a copy of the License at
// http://opensource.org/licenses/MIT

// Unless required by applicable law or agreed to in writing, software distributed under the License is
// distributed on an "AS IS" basis, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND,
// either express or implied. See the License for the specific language governing permissions and
// limitations under the License.

#import "MarsXlog.h"

#include <sys/time.h>

#import <mars/comm/xlogger/xloggerbase.h>
#import <mars/xlog/appender.h>

using namespace mars::xlog;

static TLogLevel MarsXlogLevelToTLogLevel(MarsXlogLevel level) {
    switch (level) {
        // MarsXlogLevelAll and MarsXlogLevelVerbose share the same value (0)
        case MarsXlogLevelVerbose:
            return kLevelAll;
        case MarsXlogLevelDebug:
            return kLevelDebug;
        case MarsXlogLevelInfo:
            return kLevelInfo;
        case MarsXlogLevelWarning:
            return kLevelWarn;
        case MarsXlogLevelError:
            return kLevelError;
        case MarsXlogLevelFatal:
            return kLevelFatal;
        case MarsXlogLevelNone:
            return kLevelNone;
    }
    return kLevelInfo;
}

@implementation MarsXlog

+ (void)openWithLogDir:(NSString*)logDir
            namePrefix:(NSString*)namePrefix
                  mode:(MarsXlogAppenderMode)mode
          compressMode:(MarsXlogCompressMode)compressMode {
    XLogConfig config;
    config.mode_ = (mode == MarsXlogAppenderModeSync) ? kAppenderSync : kAppenderAsync;
    config.logdir_ = logDir ? [logDir UTF8String] : "";
    config.nameprefix_ = namePrefix ? [namePrefix UTF8String] : "";
    config.compress_mode_ = (compressMode == MarsXlogCompressModeZstd) ? kZstd : kZlib;
    appender_open(config);
}

+ (void)openWithLogDir:(NSString*)logDir namePrefix:(NSString*)namePrefix {
    [self openWithLogDir:logDir namePrefix:namePrefix mode:MarsXlogAppenderModeAsync compressMode:MarsXlogCompressModeZlib];
}

+ (void)close {
    appender_close();
}

+ (void)flush {
    appender_flush();
}

+ (void)flushSync {
    appender_flush_sync();
}

+ (void)setLevel:(MarsXlogLevel)level {
    xlogger_SetLevel(MarsXlogLevelToTLogLevel(level));
}

+ (void)setConsoleLogEnabled:(BOOL)enabled {
    appender_set_console_log(enabled ? true : false);
}

+ (void)setMaxFileSize:(uint64_t)maxByteSize {
    appender_set_max_file_size(maxByteSize);
}

+ (void)setMaxAliveDuration:(long)maxAliveDuration {
    appender_set_max_alive_duration(maxAliveDuration);
}

+ (void)logWithLevel:(MarsXlogLevel)level
                 tag:(NSString*)tag
                file:(const char*)file
            function:(const char*)function
                line:(int)line
             message:(NSString*)message {
    if (!xlogger_IsEnabledFor(MarsXlogLevelToTLogLevel(level))) {
        return;
    }

    XLoggerInfo info;
    memset(&info, 0, sizeof(info));
    info.level = MarsXlogLevelToTLogLevel(level);
    info.tag = tag ? [tag UTF8String] : "";
    info.filename = file ? file : "";
    info.func_name = function ? function : "";
    info.line = line;
    info.pid = -1;
    info.tid = -1;
    info.maintid = -1;
    gettimeofday(&info.timeval, NULL);

    xlogger_Write(&info, message ? [message UTF8String] : "");
}

+ (void)logWithLevel:(MarsXlogLevel)level tag:(NSString*)tag message:(NSString*)message {
    [self logWithLevel:level tag:tag file:nullptr function:nullptr line:0 message:message];
}

@end
