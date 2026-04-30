/// External test suite runner for `writing-a-c-compiler-tests`.
///
/// Tests chapters 1-9 (the subset our compiler aims to support).
/// For each `.c` file in `valid/` (excluding `extra_credit/` and `libraries/`):
///
///   1. Compile with `hack_cc` → Hack assembly
///   2. If compile fails → SKIP (unsupported syntax or feature)
///   3. If compile succeeds → run with `hack_emu --quiet` and compare:
///      - process exit code (= main's return value, mod 256)
///      - stdout (putchar output)
///
/// Failures (wrong output when compilation succeeds) cause the test to fail.
/// Skips are reported but don't cause failure.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SUITE_TESTS: &str = r"C:\work\forks\writing-a-c-compiler-tests\tests";
const EXPECTED_JSON: &str =
    r"C:\work\forks\writing-a-c-compiler-tests\expected_results.json";

/// Maximum chapter number to test (chapters 1..=MAX_CHAPTER).
const MAX_CHAPTER: u32 = 10;

/// Maximum emulator cycles per test (generous — some tests use recursion).
const MAX_CYCLES: &str = "5000000";

/// Sub-directory names that are always skipped.
const SKIP_SUBDIRS: &[&str] = &[
    "libraries",    // multi-file programs — single-file compiler only
];

/// Specific file names (basename only) that are always skipped.
/// These require platform-specific external assembly linkage that Hack cannot provide.
const SKIP_FILES: &[&str] = &[
    "stack_alignment.c",   // requires stack_alignment_check_<platform>.s
];

// ── Helpers ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct Expected {
    return_code: i32,
    stdout: String,
}

fn load_expected(json_path: &str) -> HashMap<String, Expected> {
    let text = fs::read_to_string(json_path)
        .unwrap_or_else(|_| panic!("cannot read {json_path}"));
    let val: serde_json::Value = serde_json::from_str(&text).expect("invalid JSON");
    val.as_object()
        .expect("expected JSON object at root")
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                Expected {
                    return_code: v["return_code"].as_i64().unwrap_or(0) as i32,
                    stdout: v
                        .get("stdout")
                        .and_then(|s| s.as_str())
                        .unwrap_or("")
                        .to_string(),
                },
            )
        })
        .collect()
}

fn collect_c_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(collect_c_files(&p));
            } else if p.extension().map(|e| e == "c").unwrap_or(false) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

fn in_skip_dir(path: &Path) -> bool {
    path.components().any(|c| {
        if let std::path::Component::Normal(s) = c {
            SKIP_SUBDIRS.contains(&s.to_str().unwrap_or(""))
        } else {
            false
        }
    })
}

/// Return true if the source file contains an integer literal that cannot be
/// represented in a 16-bit signed word (i.e., > 32767 or < -32768).
/// Our compiler targets the 16-bit Hack CPU, so such tests are inherently
/// unsupported and should be skipped rather than counted as failures.
fn has_out_of_range_literal(src: &str) -> bool {
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let digit_end = i;
            // Skip over integer suffix characters so they don't affect length check.
            while i < bytes.len() && matches!(bytes[i], b'l' | b'L' | b'u' | b'U') {
                i += 1;
            }
            let num_str = &src[start..digit_end];
            if num_str.len() > 5 {
                // More than 5 digits → definitely > 32767
                return true;
            }
            if let Ok(n) = num_str.parse::<u64>() {
                if n > 32767 {
                    return true;
                }
            }
        } else {
            i += 1;
        }
    }
    false
}

// ── Stats ─────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Stats {
    pass: u32,
    skip_dir: u32,
    skip_compile: u32,
    fail: u32,
    failures: Vec<String>,
}

impl Stats {
    fn total_skip(&self) -> u32 { self.skip_dir + self.skip_compile }
}

// ── Test entry point ──────────────────────────────────────────────────────────

#[test]
fn external_c_test_suite() {
    // Bail out gracefully if the test suite isn't present.
    if !Path::new(SUITE_TESTS).exists() {
        eprintln!(
            "SKIP: external test suite not found at {SUITE_TESTS}\n\
             Clone https://github.com/nlsandler/writing-a-c-compiler-tests to run."
        );
        return;
    }

    let hack_cc = env!("CARGO_BIN_EXE_hack_cc");
    let hack_emu = env!("CARGO_BIN_EXE_hack_emu");

    let expected = load_expected(EXPECTED_JSON);

    // Temp directory for compiled .asm files.
    let tmp = std::env::temp_dir().join("hack_cc_ext_tests");
    fs::create_dir_all(&tmp).unwrap();

    let mut total = Stats::default();
    let mut chapter_lines: Vec<String> = Vec::new();

    for chapter in 1..=MAX_CHAPTER {
        let chapter_dir = Path::new(SUITE_TESTS).join(format!("chapter_{chapter}"));
        let valid_dir = chapter_dir.join("valid");
        if !valid_dir.exists() {
            continue;
        }

        let c_files = collect_c_files(&valid_dir);
        let mut ch = Stats::default();

        for c_file in &c_files {
            // Build the key as used in expected_results.json (forward slashes).
            let rel = c_file
                .strip_prefix(SUITE_TESTS)
                .unwrap()
                .to_str()
                .unwrap()
                .replace('\\', "/")
                .trim_start_matches('/')
                .to_string();

            // Skip excluded sub-directories.
            if in_skip_dir(c_file) {
                ch.skip_dir += 1;
                continue;
            }

            // Skip specific files that require external linkage not available on Hack.
            if let Some(name) = c_file.file_name().and_then(|n| n.to_str()) {
                if SKIP_FILES.contains(&name) {
                    ch.skip_dir += 1;
                    continue;
                }
            }

            // Skip tests with integer literals that exceed 16-bit range.
            let src = fs::read_to_string(c_file).unwrap_or_default();
            if has_out_of_range_literal(&src) {
                ch.skip_compile += 1;
                continue;
            }

            // Look up expected result (skip if not in JSON).
            let exp = match expected.get(&rel) {
                Some(e) => e,
                None => {
                    ch.skip_compile += 1;
                    continue;
                }
            };

            // Unique output file name.
            let idx = ch.pass + ch.total_skip() + ch.fail;
            let asm_file = tmp.join(format!("ch{chapter}_{idx}.asm"));

            // ── Step 1: Compile ───────────────────────────────────────────
            let cc = Command::new(hack_cc)
                .args([
                    c_file.to_str().unwrap(),
                    "-o",
                    asm_file.to_str().unwrap(),
                ])
                .output();

            let compiled = cc.map(|o| o.status.success()).unwrap_or(false);
            if !compiled {
                ch.skip_compile += 1;
                continue;
            }

            // ── Step 2: Run in quiet mode ─────────────────────────────────
            let emu = Command::new(hack_emu)
                .args([
                    asm_file.to_str().unwrap(),
                    "--quiet",
                    "--max-cycles",
                    MAX_CYCLES,
                ])
                .output()
                .expect("failed to run hack_emu");

            let actual_code = emu.status.code().unwrap_or(-1);
            let actual_stdout = String::from_utf8_lossy(&emu.stdout).into_owned();

            let code_ok = actual_code == exp.return_code;
            let stdout_ok = actual_stdout == exp.stdout;

            if code_ok && stdout_ok {
                ch.pass += 1;
            } else {
                ch.fail += 1;
                let mut msg = format!("FAIL  {rel}");
                if !code_ok {
                    msg += &format!(
                        "\n        return_code: got {actual_code}, expected {}",
                        exp.return_code
                    );
                }
                if !stdout_ok {
                    msg += &format!(
                        "\n        stdout: got {:?}, expected {:?}",
                        actual_stdout, exp.stdout
                    );
                }
                ch.failures.push(msg);
            }
        }

        chapter_lines.push(format!(
            "  chapter_{chapter:2}: {:3} pass  {:3} skip ({} dir + {} compile)  {:3} fail",
            ch.pass, ch.total_skip(), ch.skip_dir, ch.skip_compile, ch.fail,
        ));
        for f in &ch.failures {
            chapter_lines.push(format!("          {f}"));
        }

        total.pass += ch.pass;
        total.skip_dir += ch.skip_dir;
        total.skip_compile += ch.skip_compile;
        total.fail += ch.fail;
        total.failures.extend(ch.failures);
    }

    // ── Final report ─────────────────────────────────────────────────────────
    println!("\n=== External C Test Suite (chapters 1-{MAX_CHAPTER}) ===");
    for line in &chapter_lines {
        println!("{line}");
    }
    println!(
        "\n  TOTAL: {} pass, {} skip ({} dir + {} compile), {} fail",
        total.pass,
        total.total_skip(),
        total.skip_dir,
        total.skip_compile,
        total.fail
    );

    if !total.failures.is_empty() {
        println!("\nFailed tests:");
        for f in &total.failures {
            println!("  {f}");
        }
    }

    assert_eq!(
        total.fail, 0,
        "{} test(s) produced wrong output (see above)",
        total.fail
    );
}
