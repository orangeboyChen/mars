# Mars xlog — Rust port

This directory holds the Rust port of the Mars **xlog** pipeline. Every crate
here replaces one layer of the original C++ implementation, which has since been
removed from the repository: the `mars/...` paths in the table below are the
files these crates were ported *from*, and the wire format they had to match is
now pinned by the golden `.xlog` files of `crates/mars-xlog-compat/fixtures`.

## Layout

| crate                 | replaces (C++)                                                   |
|-----------------------|------------------------------------------------------------------|
| `mars-xlog-core`      | `mars/comm/ptrbuffer.{h,cc}`, `mars/comm/autobuffer.{h,cc}`      |
| `mars-xlog-crypt`     | `mars/xlog/crypt/log_crypt.{h,cc}`, `log_magic_num.h`            |
| `mars-xlog-buffer`    | `mars/xlog/src/log_base_buffer.*`, `log_zlib_buffer.cc`, `log_zstd_buffer.cc` |
| `mars-xlog-appender`  | `mars/xlog/src/appender.cc`, `formater.cc`, `xlogger_interface.cc` |
| `mars-xlog-ffi`       | C ABI seam behind `mars-xlog-jni` and the Swift API               |

`PORT-CONTRACT.md` in this directory is the authoritative API contract between
the crates — read it before changing a public signature.

## Building

```
cd rust
cargo build --workspace
cargo test  --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

`rust-toolchain.toml` pins stable + rustfmt + clippy. `unsafe` appears in
exactly two places: the C ABI shims in `mars-xlog-ffi` and the single
`memmap2::MmapOptions::map_mut` call in `mars-xlog-appender` (there is no safe
API for creating a mapping). Both are commented with SAFETY notes.

## CI

`.github/workflows/rust.yml` runs rustfmt, clippy, tests (Linux/macOS/Windows,
debug + release), a release build, `cargo doc`, and cross builds of
`mars-xlog-ffi` for `aarch64-linux-android`, `armv7-linux-androideabi`,
`aarch64-apple-ios` and `aarch64-apple-ios-sim`. It only triggers when `rust/**`
changes, so the existing C++ release workflows are unaffected.

## Compatibility

The on-disk `.xlog` format is **unchanged**: same 73-byte header
(magic / seq / begin hour / end hour / length / client pubkey), same 1-byte
tailer, same zlib (raw DEFLATE) and zstd streams, same TEA block encryption.
Logs written by the Rust implementation can be decoded by the existing
`decode_mars_nocrypt_log_file.py` / `decode_mars_crypt_log_file.py` scripts and
vice versa.

## Status

- [x] `mars-xlog-core`
- [x] `mars-xlog-crypt`
- [x] `mars-xlog-buffer`
- [x] `mars-xlog-appender`
- [x] `mars-xlog-ffi`
- [ ] Switch the Android/iOS glue over to `mars-xlog-ffi`
- [x] Retire the C++ implementation (xlog, STN, SDT and their vendored deps)
