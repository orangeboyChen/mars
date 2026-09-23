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
#include <sys/xattr.h>

#import <mars/comm/xlogger/xloggerbase.h>
#import <mars/xlog/appender.h>

using namespace mars::xlog;

static TLogLevel MarsXlogLevelToTLogLevel(MarsXlogLevel level) {
    // MarsXlogLevelAll and MarsXlogLevelVerbose share the same value (0)
    switch (level) {
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

static NSString* MarsXlogUTF8(NSString* _Nullable value) {
    return value ? value : @"";
}

@implementation MarsXlogOpenConfig

- (instancetype)init {
    self = [super init];
    if (self) {
        _logDir = @"";
        _cacheDir = @"";
        _namePrefix = @"";
        _pubKey = @"";
        _mode = MarsXlogAppenderModeAsync;
        _compressMode = MarsXlogCompressModeZlib;
        _level = MarsXlogLevelInfo;
        _consoleLogEnabled = NO;
    }
    return self;
}

@end

@implementation MarsXlog

+ (void)openWithConfig:(MarsXlogOpenConfig*)config {
    XLogConfig xlogConfig;
    xlogConfig.mode_ = (config.mode == MarsXlogAppenderModeSync) ? kAppenderSync : kAppenderAsync;
    xlogConfig.logdir_ = MarsXlogUTF8(config.logDir).UTF8String;
    xlogConfig.nameprefix_ = MarsXlogUTF8(config.namePrefix).UTF8String;
    xlogConfig.pub_key_ = MarsXlogUTF8(config.pubKey).UTF8String;
    xlogConfig.cachedir_ = MarsXlogUTF8(config.cacheDir).UTF8String;
    xlogConfig.compress_mode_ = (config.compressMode == MarsXlogCompressModeZstd) ? kZstd : kZlib;
    appender_open(xlogConfig);

    xlogger_SetLevel(MarsXlogLevelToTLogLevel(config.level));
    appender_set_console_log(config.consoleLogEnabled ? true : false);
}

+ (void)openWithLogDir:(NSString*)logDir namePrefix:(NSString*)namePrefix {
    MarsXlogOpenConfig* config = [[MarsXlogOpenConfig alloc] init];
    config.logDir = logDir;
    config.namePrefix = namePrefix;
    [self openWithConfig:config];
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

+ (BOOL)setExcludedFromBackup:(BOOL)exclude forPath:(NSString*)path {
    if (path.length == 0) {
        return NO;
    }
    static const char* attrName = "com.apple.MobileBackup";
    u_int8_t attrValue = exclude ? 1 : 0;
    return setxattr(path.fileSystemRepresentation, attrName, &attrValue, sizeof(attrValue), 0, 0) == 0;
}

+ (void)logWithLevel:(MarsXlogLevel)level
              module:(NSString*)module
                file:(const char*)file
                line:(int)line
            function:(const char*)function
             message:(NSString*)message {
    TLogLevel logLevel = MarsXlogLevelToTLogLevel(level);
    if (!xlogger_IsEnabledFor(logLevel)) {
        return;
    }

    XLoggerInfo info;
    memset(&info, 0, sizeof(info));
    info.level = logLevel;
    info.tag = MarsXlogUTF8(module).UTF8String;
    info.filename = file ? file : "";
    info.func_name = function ? function : "";
    info.line = line;
    info.pid = -1;
    info.tid = -1;
    info.maintid = -1;
    gettimeofday(&info.timeval, NULL);

    xlogger_Write(&info, MarsXlogUTF8(message).UTF8String);
}

+ (void)logWithLevel:(MarsXlogLevel)level
              module:(NSString*)module
            function:(NSString*)function
             message:(NSString*)message {
    [self logWithLevel:level
                module:module
                  file:nullptr
                  line:0
              function:function ? function.UTF8String : nullptr
               message:message];
}

+ (void)logWithLevel:(MarsXlogLevel)level module:(NSString*)module message:(NSString*)message {
    [self logWithLevel:level module:module function:nil message:message];
}

+ (void)infoWithModule:(NSString*)module function:(NSString*)function message:(NSString*)message {
    [self logWithLevel:MarsXlogLevelInfo module:module function:function message:message];
}

+ (void)warningWithModule:(NSString*)module function:(NSString*)function message:(NSString*)message {
    [self logWithLevel:MarsXlogLevelWarning module:module function:function message:message];
}

+ (void)errorWithModule:(NSString*)module function:(NSString*)function message:(NSString*)message {
    [self logWithLevel:MarsXlogLevelError module:module function:function message:message];
}

+ (void)fatalWithModule:(NSString*)module function:(NSString*)function message:(NSString*)message {
    [self logWithLevel:MarsXlogLevelFatal module:module function:function message:message];
}

@end
