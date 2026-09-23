//! Golden files captured from the C++ implementation.
//!
//! `compat/run.py` diffs the two implementations while the C++ is still in the
//! tree. These fixtures keep that guarantee afterwards: every file here was
//! produced by `LogZlibBuffer`/`LogZstdBuffer` (through
//! `compat/cpp/compat_tool.cc`) and has to keep decoding to exactly the text in
//! `expected.bin`, with no C++ involved.
//!
//! Regenerate with `compat/generate_fixtures.py` before deleting the C++, and
//! again after any deliberate on-disk format change.

use std::path::{Path, PathBuf};

use mars_xlog_compat::{decode_records, encode, normalize_for_compare, Opts};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn manifest() -> serde_json::Value {
    let text = std::fs::read_to_string(fixtures().join("manifest.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn privkey() -> [u8; 32] {
    let hex = manifest()["privkey"].as_str().unwrap().to_owned();
    let mut key = [0u8; 32];
    for (i, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap();
    }
    key
}

fn opts(records: &Path, out: &Path, case: &serde_json::Value) -> Opts {
    let mut opts = Opts::new();
    opts.insert("mode".into(), case["mode"].as_str().unwrap().to_owned());
    opts.insert(
        "compress".into(),
        case["compress"].as_i64().unwrap().to_string(),
    );
    opts.insert("sync".into(), case["sync"].as_i64().unwrap().to_string());
    opts.insert(
        "flush-every".into(),
        case["flush_every"].as_i64().unwrap().to_string(),
    );
    opts.insert("records".into(), records.display().to_string());
    opts.insert("out".into(), out.display().to_string());
    if case["crypt"].as_i64().unwrap() == 1 {
        opts.insert(
            "pubkey".into(),
            manifest()["pubkey"].as_str().unwrap().to_owned(),
        );
    }
    opts
}

#[test]
fn every_golden_file_decodes_to_the_original_input() {
    let dir = fixtures();
    let expected = std::fs::read(dir.join("expected.bin")).unwrap();
    let key = privkey();

    for case in manifest()["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let bytes = std::fs::read(dir.join(case["file"].as_str().unwrap())).unwrap();
        let decoded = decode_records(&bytes, &key).unwrap_or_else(|err| panic!("{name}: {err}"));
        assert_eq!(decoded, expected, "{name}: decoded text drifted");
    }
}

#[test]
fn reencoding_reproduces_the_golden_bytes_where_that_is_deterministic() {
    let dir = fixtures();
    let tmp = tempfile::tempdir().unwrap();

    for case in manifest()["cases"].as_array().unwrap() {
        if !case["byte_exact"].as_bool().unwrap() {
            continue;
        }
        let name = case["name"].as_str().unwrap();
        let mask_pubkey = case["crypt"].as_i64().unwrap() == 0;
        let out = tmp.path().join(format!("{name}.xlog"));
        encode(&opts(&dir.join("inputs.bin"), &out, case)).unwrap();

        let golden = std::fs::read(dir.join(case["file"].as_str().unwrap())).unwrap();
        let ours = std::fs::read(&out).unwrap();
        assert_eq!(
            normalize_for_compare(&golden, mask_pubkey),
            normalize_for_compare(&ours, mask_pubkey),
            "{name}: the Rust encoder drifted from the C++ bytes"
        );
    }
}
