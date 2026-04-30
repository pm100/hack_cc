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

// ── Long (32-bit) integer tests ───────────────────────────────────────────────

#[test]
fn test_long_basic_add() {
    // 100000 + 200000 = 300000; result > 255 so return (int)(result == 300000 ? 42 : 0)
    let src = r#"
int main() {
    long a;
    long b;
    long c;
    a = 100000;
    b = 200000;
    c = a + b;
    if (c == 300000) return 42;
    return 0;
}
"#;
    let asm = compile_whole(src, "long_add");
    let (code, _) = run(&asm);
    assert_eq!(code, 42);
}

#[test]
fn test_long_sub() {
    // 300000 - 100000 = 200000; return 1 if correct
    let src = r#"
int main() {
    long a;
    long b;
    a = 300000;
    b = 100000;
    long c;
    c = a - b;
    if (c == 200000) return 1;
    return 0;
}
"#;
    let asm = compile_whole(src, "long_sub");
    let (code, _) = run(&asm);
    assert_eq!(code, 1);
}

#[test]
fn test_long_neg() {
    // Negate a positive long
    let src = r#"
int main() {
    long a;
    a = 65537;
    long b;
    b = -a;
    if (b == -65537) return 7;
    return 0;
}
"#;
    let asm = compile_whole(src, "long_neg");
    let (code, _) = run(&asm);
    assert_eq!(code, 7);
}

#[test]
fn test_long_cast_to_int() {
    // Cast long to int (take low word)
    let src = r#"
int main() {
    long a;
    a = 65538;
    int b;
    b = (int)a;
    return b;
}
"#;
    let asm = compile_whole(src, "long_cast_int");
    let (code, _) = run(&asm);
    // 65538 = 0x10002; lo word = 2
    assert_eq!(code, 2);
}

#[test]
fn test_long_comparison() {
    // Test Long comparisons
    let src = r#"
int main() {
    long a;
    long b;
    a = 100000;
    b = 200000;
    if (a < b) return 1;
    return 0;
}
"#;
    let asm = compile_whole(src, "long_cmp");
    let (code, _) = run(&asm);
    assert_eq!(code, 1);
}

#[test]
fn test_long_two_step() {
    // Long arithmetic via two-step compile+link
    let src = r#"
int main() {
    long x;
    x = 70000;
    x = x + 1;
    if (x == 70001) return 5;
    return 0;
}
"#;
    let s = compile_to_s(src, "long_ts");
    let asm = link_to_asm(&[&s], "long_ts");
    let (code, _) = run(&asm);
    assert_eq!(code, 5);
}

#[test]
fn test_long_add_carry() {
    // 65535 + 1 = 65536: lo word overflows, carry propagates to hi
    let src = r#"
int main() {
    long a;
    long b;
    a = 65535;
    b = 1;
    long c;
    c = a + b;
    if (c == 65536) return 11;
    return 0;
}
"#;
    let asm = compile_whole(src, "long_carry");
    let (code, _) = run(&asm);
    assert_eq!(code, 11);
}

#[test]
fn test_long_sub_borrow() {
    // 65536 - 1 = 65535: lo word underflows, borrow propagates from hi
    let src = r#"
int main() {
    long a;
    long b;
    a = 65536;
    b = 1;
    long c;
    c = a - b;
    if (c == 65535) return 12;
    return 0;
}
"#;
    let asm = compile_whole(src, "long_borrow");
    let (code, _) = run(&asm);
    assert_eq!(code, 12);
}

#[test]
fn test_long_neg_carry() {
    // -(65536): hi=1, lo=0 -> negation lo path: ~0+1=0, carry into hi
    let src = r#"
int main() {
    long a;
    a = 65536;
    long b;
    b = -a;
    if (b == -65536) return 13;
    return 0;
}
"#;
    let asm = compile_whole(src, "long_neg_carry");
    let (code, _) = run(&asm);
    assert_eq!(code, 13);
}

#[test]
fn test_long_mul() {
    // 300 * 300 = 90000 (> 65535, forces carry from lo to hi word)
    let src = r#"
int main() {
    long a;
    long b;
    a = 300;
    b = 300;
    long c;
    c = a * b;
    if (c == 90000) return 14;
    return 0;
}
"#;
    let asm = compile_whole(src, "long_mul");
    let (code, _) = run(&asm);
    assert_eq!(code, 14);
}

#[test]
fn test_long_mul_large() {
    // 1000 * 1000 = 1000000 (well above 65535, exercises multiple carry steps)
    let src = r#"
int main() {
    long a;
    long b;
    a = 1000;
    b = 1000;
    long c;
    c = a * b;
    if (c == 1000000) return 15;
    return 0;
}
"#;
    let asm = compile_whole(src, "long_mul_large");
    let (code, _) = run(&asm);
    assert_eq!(code, 15);
}

#[test]
fn test_long_div() {
    // 1000000 / 1000 = 1000 (large dividend, quotient fits in single word)
    let src = r#"
int main() {
    long a;
    long b;
    a = 1000000;
    b = 1000;
    long c;
    c = a / b;
    if (c == 1000) return 16;
    return 0;
}
"#;
    let asm = compile_whole(src, "long_div");
    let (code, _) = run(&asm);
    assert_eq!(code, 16);
}

#[test]
fn test_long_div_quotient_large() {
    // 131072 / 2 = 65536 (quotient > 65535, forces hi word in result)
    let src = r#"
int main() {
    long a;
    long b;
    a = 131072;
    b = 2;
    long c;
    c = a / b;
    if (c == 65536) return 17;
    return 0;
}
"#;
    let asm = compile_whole(src, "long_div_large");
    let (code, _) = run(&asm);
    assert_eq!(code, 17);
}

#[test]
fn test_long_mod() {
    // 1000003 % 1000 = 3 (exercises remainder path in __ldiv)
    let src = r#"
int main() {
    long a;
    long b;
    a = 1000003;
    b = 1000;
    long c;
    c = a % b;
    if (c == 3) return 18;
    return 0;
}
"#;
    let asm = compile_whole(src, "long_mod");
    let (code, _) = run(&asm);
    assert_eq!(code, 18);
}

#[test]
fn test_long_neg_int_widening() {
    // Negative int assigned to long must sign-extend (hi word = -1)
    let src = r#"
int main() {
    int a;
    a = -5;
    long b;
    b = a;
    if (b == -5) return 19;
    return 0;
}
"#;
    let asm = compile_whole(src, "long_neg_widen");
    let (code, _) = run(&asm);
    assert_eq!(code, 19);
}

#[test]
fn test_long_mixed_arithmetic() {
    // long + int: int must be sign-extended before __ladd
    let src = r#"
int main() {
    long a;
    a = 100000;
    int b;
    b = 5;
    long c;
    c = a + b;
    if (c == 100005) return 20;
    return 0;
}
"#;
    let asm = compile_whole(src, "long_mixed_arith");
    let (code, _) = run(&asm);
    assert_eq!(code, 20);
}

#[test]
fn test_long_mixed_negative_int() {
    // long + negative int: -1 must sign-extend to long -1 (hi=-1, lo=-1)
    let src = r#"
int main() {
    long a;
    a = 100000;
    int b;
    b = -1;
    long c;
    c = a + b;
    if (c == 99999) return 21;
    return 0;
}
"#;
    let asm = compile_whole(src, "long_mixed_neg_int");
    let (code, _) = run(&asm);
    assert_eq!(code, 21);
}

#[test]
fn test_long_func_param_return() {
    // long as function parameter and return value
    let src = r#"
long double_it(long x) {
    return x * 2;
}
int main() {
    long r;
    r = double_it(100000);
    if (r == 200000) return 22;
    return 0;
}
"#;
    let asm = compile_whole(src, "long_func");
    let (code, _) = run(&asm);
    assert_eq!(code, 22);
}

#[test]
fn test_long_char_widening() {
    // char promoted to long (positive and negative char)
    let src = r#"
int main() {
    char c;
    c = 100;
    long l;
    l = c;
    if (l != 100) return 0;
    long big;
    big = 100000;
    big = big + c;
    if (big == 100100) return 23;
    return 0;
}
"#;
    let asm = compile_whole(src, "long_char_widen");
    let (code, _) = run(&asm);
    assert_eq!(code, 23);
}

#[test]
fn test_long_array_read_write() {
    let src = r#"
int main() {
    long arr[3];
    arr[0] = 100000;
    arr[1] = 200000;
    arr[2] = arr[0] + arr[1];
    if (arr[2] == 300000) return 7;
    return 0;
}
"#;
    let asm = compile_whole(src, "long_arr");
    let (code, _) = run(&asm);
    assert_eq!(code, 7);
}

#[test]
fn test_long_pointer_deref() {
    let src = r#"
int main() {
    long x;
    long *p;
    x = 70000;
    p = &x;
    *p = *p + 1;
    if (x == 70001) return 8;
    return 0;
}
"#;
    let asm = compile_whole(src, "long_ptr");
    let (code, _) = run(&asm);
    assert_eq!(code, 8);
}

#[test]
fn test_long_struct_member() {
    let src = r#"
struct Pair { long a; long b; };
int main() {
    struct Pair p;
    p.a = 50000;
    p.b = 60000;
    long s;
    s = p.a + p.b;
    if (s == 110000) return 9;
    return 0;
}
"#;
    let asm = compile_whole(src, "long_struct");
    let (code, _) = run(&asm);
    assert_eq!(code, 9);
}

// ── 9. Long tests adapted from earlier chapters (int assumed 32-bit) ──────────

// Adapted from ch2/bitwise_int_min.c: bitwise NOT on a large long
#[test]
fn test_long_bitwise_not() {
    // ~(-2147483647L) == 2147483646L
    let src = r#"
int main() {
    long a;
    a = -2147483647;
    long b;
    b = ~a;
    if (b == 2147483646) return 1;
    return 0;
}
"#;
    let asm = compile_whole(src, "long_bitwise_not");
    let (code, _) = run(&asm);
    assert_eq!(code, 1);
}

// Adapted from ch2/negate_int_max.c: negate a large positive long
#[test]
fn test_long_negate_large() {
    // -2147483647L is negative, and negating it gives back 2147483647L
    let src = r#"
int main() {
    long a;
    a = 2147483647;
    long b;
    b = -a;
    if (b == -2147483647) return 1;
    return 0;
}
"#;
    let asm = compile_whole(src, "long_negate_large");
    let (code, _) = run(&asm);
    assert_eq!(code, 1);
}

// Adapted from ch5/compound_bitwise_shiftr.c: right-shift compound assign on large long
#[test]
fn test_long_compound_shiftr() {
    // 382574L >>= 4 == 23910
    let src = r#"
int main() {
    long x;
    x = 382574;
    x >>= 4;
    if (x == 23910) return 1;
    return 0;
}
"#;
    let asm = compile_whole(src, "long_compound_shiftr");
    let (code, _) = run(&asm);
    assert_eq!(code, 1);
}

// Adapted from ch5/allocate_temps_and_vars.c: mixed long/int locals don't clobber each other
#[test]
fn test_long_mixed_locals_stack() {
    // long a, int b, long c, int d — make sure they each have correct values
    let src = r#"
int main() {
    long a;
    a = 2147483646;
    int b;
    b = 0;
    long c;
    c = a / 6 + !b;
    int r;
    r = (c * 2 == a - 1431655762);
    return r;
}
"#;
    let asm = compile_whole(src, "long_mixed_locals_stack");
    let (code, _) = run(&asm);
    assert_eq!(code, 1);
}

// Adapted from ch8/empty_loop_body.c: do-while loop with long counter
#[test]
fn test_long_do_while_loop() {
    // i=131076 (hi=2,lo=4), subtract 65537 each iteration while > 0
    // iter1: 131076-65537=65539 (hi=1,lo=3) > 0: continue
    // iter2: 65539-65537=2 (hi=0,lo=2) > 0: continue
    // iter3: 2-65537=-65535 (hi=-1,lo=1) <= 0: stop
    // (int)(-65535) = 1 (lo word)
    let src = r#"
int main() {
    long i;
    i = 131076;
    do ; while ((i = i - 65537) > 0);
    int r;
    r = (int)i;
    return r;
}
"#;
    let asm = compile_whole(src, "long_do_while");
    let (code, _) = run(&asm);
    assert_eq!(code, 1);
}

// Adapted from ch8/for_absent_post.c: for loop with long counter, modulo test
#[test]
fn test_long_for_loop_modulo() {
    // Start at -2147483647L, increment until divisible by 5
    // -2147483647 % 5 == -2, so +1 -> -2147483646%5==-1, +2 -> -2147483645%5==0
    // Then return (a % 5 || a > 0) which is 0 (false) since a%5==0 and a<0
    let src = r#"
int main() {
    long a;
    a = -2147483647;
    for (; a % 5 != 0;) {
        a = a + 1;
    }
    int r;
    r = (a % 5 || a > 0);
    return r;
}
"#;
    let asm = compile_whole(src, "long_for_loop_mod");
    let (code, _) = run(&asm);
    assert_eq!(code, 0);
}

// Logical ops on long where only hi word is nonzero (lo == 0)
#[test]
fn test_long_logical_hi_word_only() {
    // l = 65536 (hi=1, lo=0): !l must be 0, and l in boolean context is true
    let src = r#"
int main() {
    long l;
    l = 65536;
    if (!l) return 1;       // wrong: l is nonzero so !l==0
    long zero;
    zero = 0;
    if (!zero == 0) return 2; // !zero==1, !=0 so condition false, no return
    if (l && zero) return 3;  // zero makes it false
    if (!(l || zero)) return 4; // l is nonzero so l||zero==1, !1==0 so no return
    return 42;
}
"#;
    let asm = compile_whole(src, "long_logical_hi");
    let (code, _) = run(&asm);
    assert_eq!(code, 42);
}

// Long comparison where hi word differs but lo words are equal
#[test]
fn test_long_comparison_hi_word() {
    // a = 65536 (hi=1, lo=0), b = 0 (hi=0, lo=0): a > b must be true
    let src = r#"
int main() {
    long a;
    long b;
    a = 65536;
    b = 0;
    if (a <= b) return 1;  // wrong: a > b
    if (b >= a) return 2;  // wrong: b < a
    if (a == b) return 3;  // wrong: they differ in hi word
    if (a < b) return 4;
    if (a != 65536) return 5;
    return 42;
}
"#;
    let asm = compile_whole(src, "long_cmp_hi");
    let (code, _) = run(&asm);
    assert_eq!(code, 42);
}

// Global long variable
#[test]
fn test_long_global_var() {
    let src = r#"
long g;
long g2;
int main() {
    g = 2147483647;
    g2 = g + 1;
    // g + 1 overflows 32-bit signed: 2147483647 + 1 wraps to negative
    if (g2 >= 0) return 1;
    g = -1;
    long h;
    h = ~g;
    if (h != 0) return 2;
    return 42;
}
"#;
    let asm = compile_whole(src, "long_global");
    let (code, _) = run(&asm);
    assert_eq!(code, 42);
}

// Long in ternary expression
#[test]
fn test_long_ternary() {
    let src = r#"
int main() {
    long a;
    long b;
    a = 100000;
    b = 200000;
    long c;
    c = (a > b) ? a : b;  // should pick b
    if (c != 200000) return 1;
    c = (a < b) ? a : b;  // should pick a
    if (c != 100000) return 2;
    return 42;
}
"#;
    let asm = compile_whole(src, "long_ternary");
    let (code, _) = run(&asm);
    assert_eq!(code, 42);
}

// ── 10. Shift operator tests ─────────────────────────────────────────────────

// Int left shift: basic cases including shift-by-zero and overflow to negative
#[test]
fn test_int_shl() {
    let src = r#"
int main() {
    int a;
    a = 3;
    if ((a << 4) != 48) return 1;
    int c;
    c = 1;
    c <<= 14;
    if (c != 16384) return 2;
    c <<= 1;            // 16384 << 1 = -32768 (overflows signed 16-bit)
    if (c != -32768) return 3;
    int d;
    d = 5;
    if ((d << 0) != 5) return 4;
    return 42;
}
"#;
    let asm = compile_whole(src, "int_shl");
    let (code, _) = run(&asm);
    assert_eq!(code, 42);
}

// Int right shift of positive values
#[test]
fn test_int_shr_positive() {
    let src = r#"
int main() {
    int a;
    a = 256;
    if ((a >> 4) != 16) return 1;
    int b;
    b = 32767;
    if ((b >> 1) != 16383) return 2;
    int c;
    c = 192;
    if ((c >> 3) != 24) return 3;
    if ((c >> 0) != 192) return 4;
    return 42;
}
"#;
    let asm = compile_whole(src, "int_shr_pos");
    let (code, _) = run(&asm);
    assert_eq!(code, 42);
}

// Int right shift: arithmetic (sign-extending) on negative values
#[test]
fn test_int_shr_negative() {
    let src = r#"
int main() {
    int a;
    a = -16;
    if ((a >> 2) != -4) return 1;
    int b;
    b = -1;
    if ((b >> 1) != -1) return 2;
    int c;
    c = -32768;
    if ((c >> 1) != -16384) return 3;
    int d;
    d = -8;
    d >>= 2;
    if (d != -2) return 4;
    return 42;
}
"#;
    let asm = compile_whole(src, "int_shr_neg");
    let (code, _) = run(&asm);
    assert_eq!(code, 42);
}

// Int right shift saturates when count >= 15: positive→0, negative→-1
#[test]
fn test_int_shr_saturation() {
    let src = r#"
int main() {
    int a;
    a = 100;
    if ((a >> 15) != 0) return 1;
    int b;
    b = -100;
    if ((b >> 15) != -1) return 2;
    if ((b >> 14) != -1) return 3;
    return 42;
}
"#;
    let asm = compile_whole(src, "int_shr_sat");
    let (code, _) = run(&asm);
    assert_eq!(code, 42);
}

// Char shift: char promoted to int for arithmetic; test basic shifts
#[test]
fn test_char_shift() {
    let src = r#"
int main() {
    char a;
    a = 8;
    char b;
    b = a >> 2;
    if (b != 2) return 1;
    char c;
    c = -8;
    if ((c >> 2) != -2) return 2;
    char d;
    d = 1;
    d <<= 3;
    if (d != 8) return 3;
    return 42;
}
"#;
    let asm = compile_whole(src, "char_shift");
    let (code, _) = run(&asm);
    assert_eq!(code, 42);
}

// Long left shift including cross-word boundary (bit moves from lo into hi)
#[test]
fn test_long_shl() {
    let src = r#"
int main() {
    long a;
    a = 1;
    if ((a << 16) != 65536) return 1;
    long b;
    b = 1;
    b <<= 17;
    if (b != 131072) return 2;
    long c;
    c = 3;
    if ((c << 20) != 3145728) return 3;
    if ((c << 0) != 3) return 4;
    return 42;
}
"#;
    let asm = compile_whole(src, "long_shl");
    let (code, _) = run(&asm);
    assert_eq!(code, 42);
}

// Long right shift of positive values including cross-word boundary
#[test]
fn test_long_shr_positive() {
    let src = r#"
int main() {
    long a;
    a = 65536;
    if ((a >> 16) != 1) return 1;
    long b;
    b = 131072;
    if ((b >> 16) != 2) return 2;
    long c;
    c = 1048576;
    if ((c >> 4) != 65536) return 3;
    if ((c >> 0) != 1048576) return 4;
    return 42;
}
"#;
    let asm = compile_whole(src, "long_shr_pos");
    let (code, _) = run(&asm);
    assert_eq!(code, 42);
}

// Long right shift: arithmetic (sign-extending) on negative values
#[test]
fn test_long_shr_negative() {
    let src = r#"
int main() {
    long a;
    a = -65536;
    if ((a >> 1) != -32768) return 1;
    long b;
    b = -4;
    if ((b >> 2) != -1) return 2;
    long c;
    c = -1;
    if ((c >> 1) != -1) return 3;
    long d;
    d = -2147483647;
    d >>= 16;
    if (d != -32768) return 4;
    return 42;
}
"#;
    let asm = compile_whole(src, "long_shr_neg");
    let (code, _) = run(&asm);
    assert_eq!(code, 42);
}



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

// ── Static locals and file-scope statics ─────────────────────────────────────

#[test]
fn test_static_local_persists() {
    // Static local should retain value across calls.
    let src = r#"
int foo(void) {
    static int a = 3;
    a = a * 2;
    return a;
}
int main(void) {
    int x = foo(); // 6
    int y = foo(); // 12
    if (x == 6 && y == 12) return 1;
    return 0;
}
"#;
    let asm = compile_whole(src, "sl_persist");
    let (code, _) = run(&asm);
    assert_eq!(code, 1);
}

#[test]
fn test_static_locals_distinct_funcs() {
    // Static locals with same name in different functions are distinct.
    let src = r#"
int foo(void) {
    static int a = 3;
    a = a * 2;
    return a;
}
int bar(void) {
    static int a = 4;
    a = a + 1;
    return a;
}
int main(void) {
    // foo: 6, bar: 5, foo: 12, bar: 6 => sum = 29
    return foo() + bar() + foo() + bar();
}
"#;
    let asm = compile_whole(src, "sl_distinct");
    let (code, _) = run(&asm);
    assert_eq!(code, 29);
}

#[test]
fn test_static_local_uninitialized() {
    // Uninitialized static local is zero-initialized.
    let src = r#"
int foo(void) {
    static int x;
    x = x + 1;
    return x;
}
int main(void) {
    int ret = 0;
    int i;
    for (i = 0; i < 4; i = i + 1)
        ret = foo();
    return ret;
}
"#;
    let asm = compile_whole(src, "sl_uninit");
    let (code, _) = run(&asm);
    assert_eq!(code, 4);
}

#[test]
fn test_static_file_scope() {
    // Static file-scope variable has internal linkage but normal behavior.
    let src = r#"
static int x = 7;
int main(void) {
    x = x + 1;
    return x;
}
"#;
    let asm = compile_whole(src, "sf_scope");
    let (code, _) = run(&asm);
    assert_eq!(code, 8);
}

#[test]
fn test_static_local_multiple_scopes() {
    // Two static locals with same name in different blocks of the same function are distinct.
    let src = r#"
int foo(void) {
    static int i = 10;
    i = i + 1;
    {
        static int i = 100;
        i = i + 10;
    }
    return i;
}
int main(void) {
    int r1 = foo(); // outer i: 11, inner i: 110
    int r2 = foo(); // outer i: 12, inner i: 120
    if (r1 == 11 && r2 == 12) return 5;
    return 0;
}
"#;
    let asm = compile_whole(src, "sl_scopes");
    let (code, _) = run(&asm);
    assert_eq!(code, 5);
}

#[test]
fn test_static_then_extern_file_scope() {
    // `static int foo = 3; extern int foo;` - extern takes on same linkage.
    let src = r#"
static int foo = 3;
extern int foo;
int main(void) {
    return foo;
}
"#;
    let asm = compile_whole(src, "sf_extern");
    let (code, _) = run(&asm);
    assert_eq!(code, 3);
}

#[test]
fn test_tentative_definition() {
    // Multiple tentative definitions merge into one zero-initialized global.
    let src = r#"
int foo;
int foo;
int main(void) {
    int i;
    for (i = 0; i < 5; i = i + 1)
        foo = foo + 1;
    return foo;
}
int foo;
"#;
    let asm = compile_whole(src, "tentative");
    let (code, _) = run(&asm);
    assert_eq!(code, 5);
}

#[test]
fn test_local_extern_refers_to_global() {
    // `extern int a;` inside a block refers to the file-scope `a`.
    let src = r#"
int a = 5;
int return_a(void) { return a; }
int main(void) {
    int a = 3;
    {
        extern int a;
        if (a != 5) return 1;
        a = 4;
    }
    return a + return_a();
}
"#;
    let asm = compile_whole(src, "le_global");
    let (code, _) = run(&asm);
    assert_eq!(code, 7);
}

#[test]
fn test_type_before_storage_class() {
    // `int static foo(void)` and `int static bar` should work.
    let src = r#"
int static foo(void) {
    return 3;
}
int static bar = 4;
int main(void) {
    return foo() + bar;
}
"#;
    let asm = compile_whole(src, "tbsc");
    let (code, _) = run(&asm);
    assert_eq!(code, 7);
}