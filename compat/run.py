#!/usr/bin/env python3
"""Differential format test: C++ xlog <-> Rust xlog.

For every combination of (compression backend, compression on/off, crypt
on/off, sync/async, single block / block per record) it

  1. encodes the same records with the C++ ``LogZlibBuffer``/``LogZstdBuffer``
     and with the Rust ``LogBuffer``,
  2. decodes each of the two files with *both* decoders (the repo's own
     ``decode_log_file.c`` for C++, ``xlog-compat decode`` for Rust),
  3. asserts that all four decoded texts equal the original records,
  4. compares the two encoded files byte for byte where that is deterministic.

Any format drift between the two implementations fails this script.

usage: compat/run.py [--work-dir DIR] [--keep] [--filter SUBSTR]
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
COMPAT = ROOT / "compat"
KEYS = json.loads((COMPAT / "testdata" / "keys.json").read_text())

# Records are newline-separated, so they must not contain 0x0a. They cover the
# cases that matter: ASCII, UTF-8, every byte value, a payload larger than one
# TEA block, lengths around the 8-byte TEA block boundary and a long line.
RECORDS = [
    b"hello xlog",
    b"[2026-09-23 09:12:33][I][mars] first record",
    "中文日志 compatibility 测试".encode("utf-8"),
    bytes(b for b in range(1, 256) if b != 0x0A),
    b"A" * 4096,
    b"x",                       # 1 byte  -> shorter than a TEA block
    b"1234567",                 # 7 bytes -> one byte short of a TEA block
    b"12345678",                # 8 bytes -> exactly one TEA block
    b'{"level":"warn","msg":"json payload","n":42}',
    "emoji 🚀🦀".encode("utf-8"),
]

EXPECTED = b"".join(RECORDS)

# (mode, compress, crypt, sync, flush_every)
CASES = [
    (mode, compress, crypt, sync, flush_every)
    for mode in ("zlib", "zstd")
    for compress in (1, 0)
    for crypt in (1, 0)
    for sync in (0, 1)
    for flush_every in (0, 1)
]


def case_name(case: tuple) -> str:
    mode, compress, crypt, sync, flush_every = case
    return (
        f"{mode}-compress{compress}-crypt{crypt}-"
        f"{'sync' if sync else 'async'}-flush{flush_every}"
    )


def run(cmd: list[str]) -> None:
    proc = subprocess.run(cmd, capture_output=True)
    if proc.returncode != 0:
        sys.stderr.write(proc.stdout.decode("utf-8", "replace"))
        sys.stderr.write(proc.stderr.decode("utf-8", "replace"))
        raise SystemExit(f"command failed ({proc.returncode}): {' '.join(cmd)}")


def build_tools(rust_bin: Path, cpp_bin: Path) -> None:
    if not rust_bin.exists():
        print("building xlog-compat ...")
        run(["cargo", "build", "-p", "mars-xlog-compat", "--manifest-path", str(ROOT / "rust" / "Cargo.toml")])
    if not cpp_bin.exists():
        print("building compat_tool ...")
        run([str(COMPAT / "cpp" / "build.sh"), str(cpp_bin)])


def encode(tool: list[str], case: tuple, records: Path, out: Path) -> None:
    mode, compress, crypt, sync, flush_every = case
    run(tool + [
        "encode",
        f"--mode={mode}",
        f"--compress={compress}",
        f"--sync={sync}",
        f"--pubkey={KEYS['pubkey'] if crypt else ''}",
        f"--records={records}",
        f"--out={out}",
        f"--flush-every={flush_every}",
    ])


def decode(tool: list[str], src: Path, out: Path) -> None:
    run(tool + ["decode", f"--privkey={KEYS['privkey']}", f"--in={src}", f"--out={out}"])


def normalize(data: bytes, mask_seq: bool, mask_pubkey: bool) -> bytes:
    """Masks the fields that legitimately differ between two encoders.

    * begin/end hour: both encoders stamp the wall-clock hour, so a run that
      crosses an hour boundary would produce a spurious diff.
    * seq, only for `is_compress == false` on the async path: the C++ passes
      `is_compress_` where `__GetSeq(bool _is_async)` expects the sync/async
      flag, so an uncompressed async buffer gets seq 0. The Rust port passes
      `true` on purpose (documented in `LogBuffer::reset`). The appender always
      compresses, so the two agree everywhere it matters.
    * the 64-byte client public key slot whenever crypt is off: `LogCrypt`
      leaves `client_pubkey_` uninitialised when no server key is configured,
      and `SetHeaderInfo` copies it into every header anyway, so the C++ writes
      whatever was on the heap there. The Rust port writes zeros. No decoder
      reads the field on the no-crypt magics, but the bytes differ.
    """
    out = bytearray(data)
    header = 73
    tailer = 1
    offset = 0
    while offset + header + tailer <= len(out):
        out[offset + 3] = 0
        out[offset + 4] = 0
        if mask_seq:
            out[offset + 1] = 0
            out[offset + 2] = 0
        if mask_pubkey:
            out[offset + 9:offset + 73] = bytes(64)
        length = int.from_bytes(out[offset + 5:offset + 9], "little")
        offset += header + length + tailer
    return bytes(out)


def describe_diff(a: bytes, b: bytes) -> str:
    """Explains *where* two encodings differ, so a CI failure is actionable."""
    fields = []
    offset = 0
    header = 73
    tailer = 1
    while offset + header + tailer <= len(a) and offset + header + tailer <= len(b):
        for name, start, size in (
            ("magic", 0, 1),
            ("seq", 1, 2),
            ("hour", 3, 2),
            ("length", 5, 4),
            ("pubkey", 9, 64),
        ):
            if a[offset + start:offset + start + size] != b[offset + start:offset + start + size]:
                fields.append(f"{name}@{offset}")
        length = int.from_bytes(a[offset + 5:offset + 9], "little")
        body_a = a[offset + header:offset + header + length]
        body_b = b[offset + header:offset + header + length]
        if body_a != body_b:
            n = sum(1 for x, y in zip(body_a, body_b) if x != y)
            fields.append(f"payload@{offset} ({n}/{length} bytes)")
        offset += header + length + tailer
    if len(a) != len(b):
        fields.append(f"total size {len(a)} vs {len(b)}")
    return ", ".join(fields) if fields else "identical after masking"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--work-dir", default=None)
    parser.add_argument("--keep", action="store_true", help="keep the work dir")
    parser.add_argument("--filter", default="", help="only run cases whose name contains this")
    args = parser.parse_args()

    work = Path(args.work_dir) if args.work_dir else ROOT / "compat" / "build" / "diff"
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)

    rust_bin = ROOT / "rust" / "target" / "debug" / "xlog-compat"
    cpp_bin = COMPAT / "cpp" / "compat_tool"
    build_tools(rust_bin, cpp_bin)

    records = work / "records.bin"
    records.write_bytes(b"\n".join(RECORDS) + b"\n")

    rust_tool = [str(rust_bin)]
    cpp_tool = [str(cpp_bin)]

    failures = []
    rows = []
    for case in CASES:
        name = case_name(case)
        if args.filter and args.filter not in name:
            continue
        mode, compress, crypt, sync, flush_every = case
        case_dir = work / name
        case_dir.mkdir()

        cpp_xlog = case_dir / "cpp.xlog"
        rust_xlog = case_dir / "rust.xlog"
        encode(cpp_tool, case, records, cpp_xlog)
        encode(rust_tool, case, records, rust_xlog)

        cpp_bytes = cpp_xlog.read_bytes()
        rust_bytes = rust_xlog.read_bytes()
        raw_equal = cpp_bytes == rust_bytes

        # `is_compress == false` on the async path is not a decodable
        # combination: the magic byte says "zlib"/"zstd", so every decoder
        # (including the repo's own) inflates a payload that was never
        # compressed. It is still worth comparing the two encoders byte for
        # byte, so the case runs without the decode step.
        decodable = sync == 1 or compress == 1
        # `decode_log_file.c` stops as soon as the input is consumed, without
        # draining ZSTD_decompressStream, so on the async zstd path it silently
        # drops the tail of every record. That affects its own files just as
        # much, so those cases are judged by the Rust round trip alone.
        cpp_lossless = sync == 1 or mode == "zlib"

        roundtrip = "encoder-only"
        if decodable:
            # C++ encoder -> C++ decoder and -> Rust decoder.
            decode(cpp_tool, cpp_xlog, case_dir / "cpp_cpp.plain")
            decode(rust_tool, cpp_xlog, case_dir / "rust_cpp.plain")
            # Rust encoder -> Rust decoder and -> C++ decoder.
            decode(rust_tool, rust_xlog, case_dir / "rust_rust.plain")
            decode(cpp_tool, rust_xlog, case_dir / "cpp_rust.plain")

            by_cpp = (case_dir / "cpp_cpp.plain").read_bytes()
            by_rust = (case_dir / "cpp_rust.plain").read_bytes()
            rust_on_cpp = (case_dir / "rust_cpp.plain").read_bytes()
            rust_on_rust = (case_dir / "rust_rust.plain").read_bytes()

            # 1. the Rust decoder has to recover the input from both files.
            if rust_on_cpp != EXPECTED:
                failures.append(f"{name}: Rust did not decode the C++ file back to the input")
            if rust_on_rust != EXPECTED:
                failures.append(f"{name}: Rust did not decode its own file back to the input")
            # 2. where the C++ decoder is lossless it has to round trip too,
            #    and it must not tell the two files apart.
            if cpp_lossless:
                if by_cpp != EXPECTED:
                    failures.append(f"{name}: the C++ decoder did not round trip its own file")
                if by_rust != EXPECTED:
                    failures.append(f"{name}: the C++ decoder lost data reading the Rust file")
                roundtrip = "exact"
            else:
                roundtrip = f"cpp keeps {len(by_cpp)}/{len(EXPECTED)}B"

        # Byte-for-byte comparison of the two encoders. Deterministic only when
        # neither an ephemeral ECDH key nor a compressor (which need not agree
        # byte for byte across implementations) can change the bytes.
        deterministic = crypt == 0 and (sync == 1 or compress == 0)
        normalized_equal = normalize(
            cpp_bytes, mask_seq=not sync and compress == 0, mask_pubkey=crypt == 0
        ) == normalize(
            rust_bytes, mask_seq=not sync and compress == 0, mask_pubkey=crypt == 0
        )
        if deterministic and not normalized_equal:
            failures.append(
                f"{name}: C++ and Rust produced different bytes ({describe_diff(cpp_bytes, rust_bytes)})"
            )
        status = "identical" if raw_equal else ("same modulo hour" if normalized_equal else "differ")
        rows.append((name, len(cpp_bytes), len(rust_bytes), status, roundtrip))

    print(f"\n{'case':<40}{'c++ B':>8}{'rust B':>8}  {'bytes':<20}{'round trip'}")
    print("-" * 92)
    print("-" * 88)
    for name, cpp_len, rust_len, status, roundtrip in rows:
        print(f"{name:<40}{cpp_len:>8}{rust_len:>8}  {status:<20}{roundtrip}")
    print("-" * 92)
    print(f"{len(rows)} cases, {len(failures)} failure(s)")

    for failure in failures:
        print(f"FAIL {failure}", file=sys.stderr)

    if not args.keep:
        shutil.rmtree(work, ignore_errors=True)
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
