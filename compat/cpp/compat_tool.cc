// The C++ half of the differential format test (see compat/README.md).
//
// `encode` drives the real LogZlibBuffer / LogZstdBuffer over a heap region the
// way XloggerAppender drives them over its mmap'd cache file, and dumps the
// flushed bytes: that is exactly the content of an .xlog file.
//
// `decode` is not reimplemented here: it calls the repo's own decoder,
// mars/xlog/crypt/decode_log_file_c_impl/decode_log_file.c, after pointing its
// PRIV_KEY / PUB_KEY globals at the test key.
//
// usage:
//   compat_tool encode --mode=zlib|zstd [--compress=1] [--sync=0]
//                      [--pubkey=HEX] --records=PATH --out=PATH
//                      [--region=N] [--flush-every=N] [--level=6]
//   compat_tool decode --privkey=HEX --in=PATH --out=PATH

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <map>

#include <string>
#include <vector>

#include "mars/comm/autobuffer.h"
#include "xlog/src/log_zlib_buffer.h"
#include "xlog/src/log_zstd_buffer.h"

extern "C" {
// Defined (non-static) in decode_log_file.c so the keys can be injected
// without patching the vendored source.
extern const char* PRIV_KEY;
extern const char* PUB_KEY;
void parseFile(const char* path, const char* outPath);
}

namespace {

using mars::xlog::LogBaseBuffer;
using mars::xlog::LogZlibBuffer;
using mars::xlog::LogZstdBuffer;

const size_t kDefaultRegion = 150 * 1024;  // kBufferBlockLength in appender.cc
const int kDefaultLevel = 6;

typedef std::map<std::string, std::string> Opts;

Opts ParseOpts(int argc, char** argv, int first) {
    Opts opts;
    for (int i = first; i < argc; ++i) {
        const char* arg = argv[i];
        if (strncmp(arg, "--", 2) != 0) {
            fprintf(stderr, "compat_tool: ignored argument `%s`\n", arg);
            continue;
        }
        const char* eq = strchr(arg + 2, '=');
        if (NULL == eq) {
            fprintf(stderr, "compat_tool: ignored argument `%s`\n", arg);
            continue;
        }
        opts[std::string(arg + 2, eq - arg - 2)] = std::string(eq + 1);
    }
    return opts;
}

std::string Get(const Opts& opts, const char* key, const char* fallback) {
    Opts::const_iterator it = opts.find(key);
    return it == opts.end() ? std::string(fallback) : it->second;
}

bool Flag(const Opts& opts, const char* key, bool fallback) {
    Opts::const_iterator it = opts.find(key);
    if (it == opts.end()) {
        return fallback;
    }
    return it->second == "1" || it->second == "true";
}

long Number(const Opts& opts, const char* key, long fallback) {
    Opts::const_iterator it = opts.find(key);
    return it == opts.end() ? fallback : atol(it->second.c_str());
}

// One record per line; a trailing newline does not start a record.
std::vector<std::string> ReadRecords(const char* path) {
    FILE* file = fopen(path, "rb");
    if (NULL == file) {
        fprintf(stderr, "compat_tool: cannot open %s\n", path);
        exit(1);
    }
    std::vector<std::string> records;
    std::string current;
    int c;
    while (EOF != (c = fgetc(file))) {
        if (c == '\n') {
            records.push_back(current);
            current.clear();
        } else {
            current.push_back((char)c);
        }
    }
    fclose(file);
    if (!current.empty()) {
        records.push_back(current);
    }
    return records;
}

void WriteFile(const char* path, const void* data, size_t len) {
    FILE* file = fopen(path, "wb");
    if (NULL == file) {
        fprintf(stderr, "compat_tool: cannot write %s\n", path);
        exit(1);
    }
    if (len != fwrite(data, 1, len, file)) {
        fprintf(stderr, "compat_tool: short write to %s\n", path);
        exit(1);
    }
    fclose(file);
}

int Encode(const Opts& opts) {
    const std::string mode = Get(opts, "mode", "zlib");
    const bool zstd = (mode == "zstd");
    if (!zstd && mode != "zlib") {
        fprintf(stderr, "compat_tool: --mode must be zlib or zstd\n");
        return 1;
    }
    const bool is_compress = Flag(opts, "compress", true);
    const bool sync = Flag(opts, "sync", false);
    const std::string pubkey = Get(opts, "pubkey", "");
    const std::string records_path = Get(opts, "records", "");
    const std::string out_path = Get(opts, "out", "");
    const size_t region_len = (size_t)Number(opts, "region", (long)kDefaultRegion);
    const size_t flush_every = (size_t)Number(opts, "flush-every", 0);
    const int level = (int)Number(opts, "level", kDefaultLevel);
    if (records_path.empty() || out_path.empty()) {
        fprintf(stderr, "compat_tool: --records and --out are required\n");
        return 1;
    }

    const std::vector<std::string> records = ReadRecords(records_path.c_str());

    std::vector<char> region(region_len, 0);
    LogBaseBuffer* buffer = zstd
        ? (LogBaseBuffer*)new LogZstdBuffer(&region[0],
                                           region_len,
                                           is_compress,
                                           pubkey.empty() ? NULL : pubkey.c_str(),
                                           level)
        : (LogBaseBuffer*)new LogZlibBuffer(&region[0],
                                            region_len,
                                            is_compress,
                                            pubkey.empty() ? NULL : pubkey.c_str());

    AutoBuffer out;
    if (sync) {
        // XloggerAppender::__WriteFile passes a fresh AutoBuffer per record.
        for (size_t i = 0; i < records.size(); ++i) {
            AutoBuffer block;
            buffer->Write(records[i].data(), records[i].size(), block);
            out.Write(block.Ptr(), block.Length());
        }
    } else {
        for (size_t i = 0; i < records.size(); ++i) {
            if (flush_every > 0 && i > 0 && i % flush_every == 0) {
                AutoBuffer block;
                buffer->Flush(block);
                out.Write(block.Ptr(), block.Length());
            }
            if (!buffer->Write(records[i].data(), records[i].size())) {
                fprintf(stderr, "compat_tool: record %zu did not fit in the region\n", i);
                return 1;
            }
        }
        AutoBuffer block;
        buffer->Flush(block);
        out.Write(block.Ptr(), block.Length());
    }

    delete buffer;
    WriteFile(out_path.c_str(), out.Ptr(), out.Length());
    printf("encode: %zu records -> %zu bytes\n", records.size(), out.Length());
    return 0;
}

int Decode(const Opts& opts) {
    const std::string privkey = Get(opts, "privkey", "");
    const std::string in_path = Get(opts, "in", "");
    const std::string out_path = Get(opts, "out", "");
    if (privkey.size() != 64 || in_path.empty() || out_path.empty()) {
        fprintf(stderr, "compat_tool: --privkey (64 hex), --in and --out are required\n");
        return 1;
    }

    // decode_log_file.c reads these globals; they are `const char*` (pointer to
    // const, mutable pointer), so they can be repointed from here.
    PRIV_KEY = privkey.c_str();
    PUB_KEY = "";
    parseFile(in_path.c_str(), out_path.c_str());
    printf("decode: %s -> %s\n", in_path.c_str(), out_path.c_str());
    return 0;
}

}  // namespace

int main(int argc, char** argv) {
    if (argc < 2) {
        fprintf(stderr,
                "usage: compat_tool encode --mode=zlib|zstd [--compress=1] [--sync=0] "
                "[--pubkey=HEX] --records=PATH --out=PATH\n"
                "       compat_tool decode --privkey=HEX --in=PATH --out=PATH\n");
        return 1;
    }

    const Opts opts = ParseOpts(argc, argv, 2);
    if (0 == strcmp(argv[1], "encode")) {
        return Encode(opts);
    }
    if (0 == strcmp(argv[1], "decode")) {
        return Decode(opts);
    }
    fprintf(stderr, "compat_tool: unknown subcommand `%s`\n", argv[1]);
    return 1;
}
