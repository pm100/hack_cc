/// Integration tests covering all compile/link path combinations:
///
///   1. One-step whole-program compile (baseline)
///   2. Two-step: `hack_cc -c` then `hack_ld` (asm output)
///   3. `-I` include-directory flag in `-c` mode (regression for earlier bug)
///   4. Two-step: multiple .s files linked together
///   5. Global variables through the two-step path
///   6. String literals through the two-step path
///   7. `hack_ld` to `.hackem`: font table in RAM@ sections, not ROM (regression)
///   8. `compile_files` library API (multi-file, no subprocesses)

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const INCLUDE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/include");

fn tmp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("hack_cc_link_tests");
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Compile a C source string to a `.s` object file via `hack_cc -c -I include`.
fn compile_to_s(src: &str, name: &str) -> PathBuf {
    let hack_cc = env!("CARGO_BIN_EXE_hack_cc");
    let dir = tmp_dir();
    let c_path = dir.join(format!("{}.c", name));
    let s_path = dir.join(format!("{}.s", name));
    fs::write(&c_path, src).unwrap();

    let status = Command::new(hack_cc)
        .args([
            c_path.to_str().unwrap(),
            "-I", INCLUDE_DIR,
            "-c",
            "-o", s_path.to_str().unwrap(),
        ])
        .status()
        .unwrap_or_else(|e| panic!("failed to run hack_cc: {}", e));
    assert!(status.success(), "hack_cc -c failed for '{}'", name);
    s_path
}

/// Link `.s` files to a `.asm` output via `hack_ld`.
fn link_to_asm(s_files: &[&Path], name: &str) -> PathBuf {
    let hack_ld = env!("CARGO_BIN_EXE_hack_ld");
    let out_path = tmp_dir().join(format!("{}.asm", name));
    let mut cmd = Command::new(hack_ld);
    for s in s_files { cmd.arg(s); }
    cmd.arg("-o").arg(&out_path);
    let status = cmd.status().unwrap_or_else(|e| panic!("failed to run hack_ld: {}", e));
    assert!(status.success(), "hack_ld failed for '{}'", name);
    out_path
}

/// Link `.s` files to a `.hackem` output via `hack_ld`.
fn link_to_hackem(s_files: &[&Path], name: &str) -> PathBuf {
    let hack_ld = env!("CARGO_BIN_EXE_hack_ld");
    let out_path = tmp_dir().join(format!("{}.hackem", name));
    let mut cmd = Command::new(hack_ld);
    for s in s_files { cmd.arg(s); }
    cmd.arg("-o").arg(&out_path);
    let status = cmd.status().unwrap_or_else(|e| panic!("failed to run hack_ld: {}", e));
    assert!(status.success(), "hack_ld failed for '{}'", name);
    out_path
}

/// Compile a C source string to a `.asm` whole-program output via `hack_cc`.
fn compile_whole(src: &str, name: &str) -> PathBuf {
    let hack_cc = env!("CARGO_BIN_EXE_hack_cc");
    let dir = tmp_dir();
    let c_path = dir.join(format!("{}_wp.c", name));
    let asm_path = dir.join(format!("{}_wp.asm", name));
    fs::write(&c_path, src).unwrap();

    let status = Command::new(hack_cc)
        .args([
            c_path.to_str().unwrap(),
            "-I", INCLUDE_DIR,
            "-o", asm_path.to_str().unwrap(),
        ])
        .status()
        .unwrap_or_else(|e| panic!("failed to run hack_cc: {}", e));
    assert!(status.success(), "hack_cc failed for '{}'", name);
    asm_path
}

/// Run an asm/hackem file with `hack_emu --quiet` and return `(exit_code, stdout)`.
fn run(path: &Path) -> (i32, String) {
    let hack_emu = env!("CARGO_BIN_EXE_hack_emu");
    let out = Command::new(hack_emu)
        .args([path.to_str().unwrap(), "--quiet", "--max-cycles", "2000000"])
        .output()
        .unwrap_or_else(|e| panic!("failed to run hack_emu: {}", e));
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    (code, stdout)
}

// ── 1. Baseline: whole-program compile ───────────────────────────────────────

#[test]
fn test_whole_program_return_value() {
    let asm = compile_whole("int main() { return 42; }", "wp_ret");
    let (code, _) = run(&asm);
    assert_eq!(code, 42);
}

#[test]
fn test_whole_program_putchar() {
    let src = "#include <hack.h>\nint main() { putchar('H'); putchar('i'); return 0; }";
    let asm = compile_whole(src, "wp_putchar");
    let (code, out) = run(&asm);
    assert_eq!(code, 0);
    assert_eq!(out, "Hi");
}

// ── 2. Two-step: hack_cc -c then hack_ld ─────────────────────────────────────

#[test]
fn test_two_step_return_value() {
    let s = compile_to_s("int main() { return 42; }", "ts_ret");
    let asm = link_to_asm(&[&s], "ts_ret");
    let (code, _) = run(&asm);
    assert_eq!(code, 42);
}

#[test]
fn test_two_step_putchar() {
    let src = "#include <hack.h>\nint main() { putchar('H'); putchar('i'); return 0; }";
    let s = compile_to_s(src, "ts_putchar");
    let asm = link_to_asm(&[&s], "ts_putchar");
    let (code, out) = run(&asm);
    assert_eq!(code, 0);
    assert_eq!(out, "Hi");
}

#[test]
fn test_two_step_arithmetic() {
    let src = "int fib(int n) {
        if (n <= 1) return n;
        return fib(n-1) + fib(n-2);
    }
    int main() { return fib(10); }";
    let s = compile_to_s(src, "ts_fib");
    let asm = link_to_asm(&[&s], "ts_fib");
    let (code, _) = run(&asm);
    assert_eq!(code, 55);
}

// ── 3. -I include directory in -c mode (regression) ──────────────────────────

/// This was the original bug: -I was ignored when -c was used. The include
/// directory must be searched so that #include <hack.h> resolves.
#[test]
fn test_include_dir_with_c_flag() {
    let src = "#include <hack.h>\nint main() { putchar('X'); return 7; }";
    let s = compile_to_s(src, "incl_c");
    let asm = link_to_asm(&[&s], "incl_c");
    let (code, out) = run(&asm);
    assert_eq!(code, 7);
    assert_eq!(out, "X");
}

// ── 4. Multi-file link ────────────────────────────────────────────────────────

#[test]
fn test_two_file_link() {
    let lib_src = "int add(int a, int b) { return a + b; }";
    let main_src = "int add(int a, int b);\nint main() { return add(10, 32); }";
    let s_lib = compile_to_s(lib_src, "mf_lib");
    let s_main = compile_to_s(main_src, "mf_main");
    let asm = link_to_asm(&[&s_main, &s_lib], "mf");
    let (code, _) = run(&asm);
    assert_eq!(code, 42);
}

#[test]
fn test_three_file_link() {
    let math_src = "int mul(int a, int b) { return a * b; }";
    let util_src = "int mul(int a, int b);\nint square(int x) { return mul(x, x); }";
    let main_src = "int square(int x);\nint main() { return square(7); }";
    let s_math = compile_to_s(math_src, "tf_math");
    let s_util = compile_to_s(util_src, "tf_util");
    let s_main = compile_to_s(main_src, "tf_main");
    let asm = link_to_asm(&[&s_main, &s_util, &s_math], "tf");
    let (code, _) = run(&asm);
    assert_eq!(code, 49);
}

// ── 5. Global variables through two-step ─────────────────────────────────────

#[test]
fn test_two_step_globals() {
    let src = "int g = 10;\nint main() { g += 5; return g; }";
    let s = compile_to_s(src, "ts_globals");
    let asm = link_to_asm(&[&s], "ts_globals");
    let (code, _) = run(&asm);
    assert_eq!(code, 15);
}

#[test]
fn test_two_step_global_across_files() {
    let a_src = "int counter = 0;\nvoid bump() { counter++; }";
    let b_src = "extern int counter;\nvoid bump();\nint main() { bump(); bump(); bump(); return counter; }";
    let s_a = compile_to_s(a_src, "ga_a");
    let s_b = compile_to_s(b_src, "ga_b");
    let asm = link_to_asm(&[&s_b, &s_a], "ga");
    let (code, _) = run(&asm);
    assert_eq!(code, 3);
}

// ── 6. String literals through two-step ──────────────────────────────────────

#[test]
fn test_two_step_string_literal() {
    let src = "#include <hack.h>\nint main() { puts(\"Hi\"); return 0; }";
    let s = compile_to_s(src, "ts_str");
    let asm = link_to_asm(&[&s], "ts_str");
    let (code, out) = run(&asm);
    assert_eq!(code, 0);
    // puts appends newline
    assert!(out.starts_with("Hi"), "expected output starting with 'Hi', got {:?}", out);
}

// ── 7. hackem output: font table must be in RAM@ sections ────────────────────

/// Regression: previously hack_ld --format hackem (or -o *.hackem) emitted the
/// font-table as ~2820 inline bootstrap instructions, bloating the ROM and
/// leaving the RAM@ sections empty. Now it must use static RAM@ sections.
#[test]
fn test_hackem_font_in_ram_not_rom() {
    let src = "#define HACK_OUTPUT_SCREEN\n#include <hack.h>\nint main() { putchar_screen('A'); return 0; }";
    let s = compile_to_s(src, "hfont");
    let hackem = link_to_hackem(&[&s], "hfont");
    let content = fs::read_to_string(&hackem).unwrap();

    // RAM@ sections must be present (font table entries)
    assert!(
        content.contains("RAM@"),
        "hackem output must have RAM@ sections for font table"
    );

    // The halt address is on the first line: "hackem v1.0 0xNNNN"
    // With font in RAM@, the ROM is tiny. With font inline it would be ~3000.
    let first_line = content.lines().next().unwrap();
    let halt_hex = first_line.split("0x").nth(1).expect("no hex halt in first line");
    let halt_addr = u32::from_str_radix(halt_hex, 16).unwrap();
    assert!(
        halt_addr < 500,
        "halt address 0x{:x} is too large — font table init must not be in ROM", halt_addr
    );
}

/// Two-step hackem output must be functionally correct (not just well-formed).
#[test]
fn test_hackem_two_step_runs() {
    let src = "#include <hack.h>\nint main() { puts(\"OK\"); return 5; }";
    let s = compile_to_s(src, "hrun");
    let hackem = link_to_hackem(&[&s], "hrun");
    let (code, out) = run(&hackem);
    assert_eq!(code, 5);
    assert!(out.starts_with("OK"), "expected 'OK' output, got {:?}", out);
}

// ── 8. compile_files library API ─────────────────────────────────────────────

#[test]
fn test_compile_files_api_single() {
    use hack_cc::{compile_files, output::{emit, OutputFormat}};

    let src = "int main() { return 99; }";
    let prog = compile_files(&[(src, None)]).unwrap();
    let asm = emit(&prog, OutputFormat::Asm).unwrap().main;

    let asm_path = tmp_dir().join("api_single.asm");
    fs::write(&asm_path, &asm).unwrap();
    let (code, _) = run(&asm_path);
    assert_eq!(code, 99);
}

#[test]
fn test_compile_files_api_multi() {
    use hack_cc::{compile_files, output::{emit, OutputFormat}};

    let lib_src = "int triple(int x) { return x * 3; }";
    let main_src = "int triple(int x);\nint main() { return triple(7); }";

    let prog = compile_files(&[(main_src, None), (lib_src, None)]).unwrap();
    let asm = emit(&prog, OutputFormat::Asm).unwrap().main;

    let asm_path = tmp_dir().join("api_multi.asm");
    fs::write(&asm_path, &asm).unwrap();
    let (code, _) = run(&asm_path);
    assert_eq!(code, 21);
}

#[test]
fn test_compile_files_api_with_stdlib() {
    use hack_cc::output::{emit, OutputFormat};

    let src = "#include <hack.h>\nint main() { putchar('Z'); return 0; }";
    let opts = hack_cc::CompileOptions {
        include_dirs: vec![std::path::PathBuf::from(INCLUDE_DIR)],
        ..Default::default()
    };
    let prog = hack_cc::compile_files_with_full_options(&[(src, None)], &opts).unwrap();
    let asm = emit(&prog, OutputFormat::Asm).unwrap().main;

    let asm_path = tmp_dir().join("api_stdlib.asm");
    fs::write(&asm_path, &asm).unwrap();
    let (code, out) = run(&asm_path);
    assert_eq!(code, 0);
    assert_eq!(out, "Z");
}
