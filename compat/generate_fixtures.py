#!/usr/bin/env python3
"""Freeze the C++ encoder's output as golden fixtures.

`compat/run.py` proves the Rust port matches the C++ *while the C++ is still
here*. Once mars/xlog is deleted that test cannot run any more, so this script
captures what the C++ produced — one .xlog per case plus the text it has to
decode to — and `crates/mars-xlog-compat/tests/golden.rs` replays them on every
`cargo test`, with no C++ involved.

Run it once before deleting the C++ (and again after any deliberate format
change):

    ./compat/cpp/build.sh
    python3 compat/generate_fixtures.py
"""

from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
COMPAT = ROOT / "compat"
OUT = ROOT / "rust" / "crates" / "mars-xlog-compat" / "fixtures"

# Reuse the exact record set the differential matrix uses.
spec = importlib.util.spec_from_file_location("runmod", COMPAT / "run.py")
run = importlib.util.module_from_spec(spec)
spec.loader.exec_module(run)

KEYS = json.loads((COMPAT / "testdata" / "keys.json").read_text())

# compress=0 on the async path is not a decodable combination (the magic says
# zlib/zstd, so decoders inflate data that was never compressed), and the sync
# path ignores compression entirely — so one sync case per (mode, crypt) is
# enough and the rest of the matrix is compress=1.
CASES = [
    {
        "name": f"{mode}-{'sync' if sync else 'async'}-crypt{crypt}-flush{flush_every}",
        "mode": mode,
        "compress": 1,
        "sync": sync,
        "crypt": crypt,
        "flush_every": flush_every,
    }
    for mode in ("zlib", "zstd")
    for sync in (0, 1)
    for crypt in (1, 0)
    for flush_every in (0, 1)
]

# Where the two implementations can be expected to produce identical bytes:
# zlib matches (flate2 runs on zlib-rs, same semantics as the C++'s zlib), and
# crypt records always differ because each side generates an ephemeral client
# key. zstd is excluded: the repo vendors 1.4.4 while the Rust side builds
# 1.5.7.
def byte_exact(case: dict) -> bool:
    if case["crypt"] == 1:
        # Each side generates an ephemeral client key, so nothing is stable.
        return False
    # Sync records are stored verbatim — no compressor is involved — so the
    # zstd version mismatch does not apply to them.
    return case["mode"] == "zlib" or case["sync"] == 1


def main() -> int:
    cpp = COMPAT / "cpp" / "compat_tool"
    if not cpp.exists():
        print("building compat_tool ...")
        subprocess.run([str(COMPAT / "cpp" / "build.sh"), str(cpp)], check=True)

    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "inputs.bin").write_bytes(b"\n".join(run.RECORDS) + b"\n")
    (OUT / "expected.bin").write_bytes(run.EXPECTED)

    manifest = {
        "_comment": (
            "Golden files produced by the C++ xlog implementation "
            "(LogZlibBuffer/LogZstdBuffer via compat/cpp/compat_tool.cc). "
            "inputs.bin holds one log record per line; expected.bin is the text "
            "every decoder has to recover from <case>.xlog. Regenerate with "
            "compat/generate_fixtures.py."
        ),
        "records": "inputs.bin",
        "expected": "expected.bin",
        "privkey": KEYS["privkey"],
        "pubkey": KEYS["pubkey"],
        "cases": [],
    }

    for case in CASES:
        name = case["name"]
        xlog = OUT / f"{name}.xlog"
        subprocess.run(
            [
                str(cpp),
                "encode",
                f"--mode={case['mode']}",
                f"--compress={case['compress']}",
                f"--sync={case['sync']}",
                f"--pubkey={KEYS['pubkey'] if case['crypt'] else ''}",
                f"--records={OUT / 'inputs.bin'}",
                f"--out={xlog}",
                f"--flush-every={case['flush_every']}",
            ],
            check=True,
            capture_output=True,
        )
        manifest["cases"].append(
            {
                "name": name,
                "file": f"{name}.xlog",
                "mode": case["mode"],
                "compress": case["compress"],
                "sync": case["sync"],
                "crypt": case["crypt"],
                "flush_every": case["flush_every"],
                "byte_exact": byte_exact(case),
            }
        )
        print(f"  {name}: {xlog.stat().st_size} bytes")

    (OUT / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"wrote {len(manifest['cases'])} fixtures to {OUT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
