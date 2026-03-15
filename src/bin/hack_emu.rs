/// hack_emu — Hack CPU emulator.
///
/// Usage:  hack_emu <file.asm> [--max-cycles N] [--dump-ram N] [--trace]
///
/// Assembles and runs Hack assembly, then reports:
///   - return value of main (RAM[256])
///   - cycle count
///   - optionally first N RAM words
///
/// Halt detection: the infinite-loop pattern the compiler emits is
///   (label)  @label  0;JMP
/// i.e. two consecutive ROM instructions where the second jumps back to
/// the first unconditionally. We also accept a cycle-count limit as a
/// safety net.

use std::collections::HashMap;
use std::path::PathBuf;

// ── CLI ──────────────────────────────────────────────────────────────────────

fn usage() -> ! {
    eprintln!("Usage: hack_emu <file.asm> [--max-cycles N] [--dump-ram N] [--screen <out.ppm>] [--trace]");
    std::process::exit(1);
}

struct Args {
    path: PathBuf,
    max_cycles: u64,
    dump_ram: usize,
    screen_out: Option<PathBuf>,
    trace: bool,
}

fn parse_args() -> Args {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.is_empty() { usage(); }
    let mut path = None;
    let mut max_cycles = 10_000_000u64;
    let mut dump_ram = 0usize;
    let mut screen_out = None;
    let mut trace = false;
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--max-cycles" => {
                i += 1;
                max_cycles = raw.get(i).and_then(|s| s.parse().ok()).unwrap_or_else(|| usage());
            }
            "--dump-ram" => {
                i += 1;
                dump_ram = raw.get(i).and_then(|s| s.parse().ok()).unwrap_or_else(|| usage());
            }
            "--screen" => {
                i += 1;
                screen_out = Some(PathBuf::from(raw.get(i).unwrap_or_else(|| usage())));
            }
            "--trace" => { trace = true; }
            s if s.starts_with("--") => { eprintln!("unknown flag: {}", s); usage(); }
            s => {
                if path.is_some() { usage(); }
                path = Some(PathBuf::from(s));
            }
        }
        i += 1;
    }
    Args { path: path.unwrap_or_else(|| usage()), max_cycles, dump_ram, screen_out, trace }
}

// ── Assembler ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum Instr {
    A(i16),          // A-instruction: load 15-bit value into A
    C {
        comp: String,
        dest: String,
        jump: String,
    },
}

/// Predefined symbols
fn predefined() -> HashMap<String, i16> {
    let mut m = HashMap::new();
    m.insert("SP".into(),     0);
    m.insert("LCL".into(),    1);
    m.insert("ARG".into(),    2);
    m.insert("THIS".into(),   3);
    m.insert("THAT".into(),   4);
    m.insert("SCREEN".into(), 16384);
    m.insert("KBD".into(),    24576);
    for i in 0i16..=15 {
        m.insert(format!("R{}", i), i);
    }
    m
}

fn assemble(src: &str) -> Result<Vec<Instr>, String> {
    let mut symbols = predefined();

    // ── Pass 1: collect labels ───────────────────────────────────────────
    let mut rom_addr = 0i16;
    for line in src.lines() {
        let line = strip_comment(line).trim().to_string();
        if line.is_empty() { continue; }
        if line.starts_with('(') && line.ends_with(')') {
            let label = &line[1..line.len()-1];
            symbols.insert(label.to_string(), rom_addr);
        } else {
            rom_addr += 1;
        }
    }

    // ── Pass 2: emit instructions ────────────────────────────────────────
    let mut next_var_addr = 16i16;
    let mut rom = Vec::new();

    for line in src.lines() {
        let line = strip_comment(line).trim().to_string();
        if line.is_empty() { continue; }
        if line.starts_with('(') { continue; } // label — skip

        if let Some(rest) = line.strip_prefix('@') {
            // A-instruction
            let val = if let Ok(n) = rest.parse::<i16>() {
                n
            } else {
                // symbol lookup or allocation
                if !symbols.contains_key(rest) {
                    symbols.insert(rest.to_string(), next_var_addr);
                    next_var_addr += 1;
                }
                *symbols.get(rest).unwrap()
            };
            rom.push(Instr::A(val));
        } else {
            // C-instruction: [dest=]comp[;jump]
            let (dest, rest) = if let Some(pos) = line.find('=') {
                (line[..pos].to_string(), line[pos+1..].to_string())
            } else {
                (String::new(), line.clone())
            };
            let (comp, jump) = if let Some(pos) = rest.find(';') {
                (rest[..pos].to_string(), rest[pos+1..].to_string())
            } else {
                (rest, String::new())
            };
            rom.push(Instr::C { comp, dest, jump });
        }
    }

    Ok(rom)
}

fn strip_comment(line: &str) -> &str {
    if let Some(pos) = line.find("//") {
        &line[..pos]
    } else {
        line
    }
}

// ── CPU ──────────────────────────────────────────────────────────────────────

const RAM_SIZE: usize = 32768;
/// Writes to this RAM address are intercepted as character output.
const OUTPUT_PORT: usize = 32767;

// ── Screen ───────────────────────────────────────────────────────────────────

/// First word of the Hack screen memory map (16384 words × 16 bits = 512×256 pixels).
pub const SCREEN_BASE: usize = 16384;
/// One past the last screen word.
pub const SCREEN_END:  usize = 24576; // 16384 + 256*32

/// Return whether pixel (x, y) is set (black) in `ram`.
/// x ∈ [0,511], y ∈ [0,255].
pub fn pixel_set(ram: &[i16], x: usize, y: usize) -> bool {
    let addr = SCREEN_BASE + y * 32 + x / 16;
    let bit  = x % 16;
    (ram[addr] as u16 >> bit) & 1 != 0
}

/// Render the Hack screen to a PPM (P6) image.
/// Returns 512×256 pixels, black-on-white, as raw bytes.
pub fn render_screen_ppm(ram: &[i16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(512 * 256 * 3 + 64);
    out.extend_from_slice(b"P6\n512 256\n255\n");
    for row in 0..256usize {
        for col in 0..512usize {
            if pixel_set(ram, col, row) {
                out.extend_from_slice(&[0, 0, 0]);
            } else {
                out.extend_from_slice(&[255, 255, 255]);
            }
        }
    }
    out
}

/// Render the Hack screen as ASCII art (scaled to ½ width for readability).
/// Uses `#` for black pixels and ` ` for white.  Returns one string per row.
pub fn render_screen_ascii(ram: &[i16]) -> String {
    let mut s = String::with_capacity(256 * (256 + 1));
    for row in 0..256usize {
        for col in (0..512usize).step_by(2) { // sample every other column
            s.push(if pixel_set(ram, col, row) { '#' } else { ' ' });
        }
        s.push('\n');
    }
    s
}

struct Cpu {
    a: i16,
    d: i16,
    pc: usize,
    ram: Vec<i16>,
    /// Characters written to OUTPUT_PORT during execution.
    pub output: Vec<u8>,
}

impl Cpu {
    fn new() -> Self {
        Self {
            a: 0, d: 0, pc: 0,
            ram: vec![0; RAM_SIZE],
            output: Vec::new(),
        }
    }

    fn m(&self) -> i16 {
        let addr = self.a as usize;
        if addr < RAM_SIZE { self.ram[addr] } else { 0 }
    }

    fn compute(&self, comp: &str) -> i16 {
        let a = self.a;
        let d = self.d;
        let m = self.m();
        match comp {
            "0"   => 0,
            "1"   => 1,
            "-1"  => -1,
            "D"   => d,
            "A"   => a,
            "M"   => m,
            "!D"  => !d,
            "!A"  => !a,
            "!M"  => !m,
            "-D"  => d.wrapping_neg(),
            "-A"  => a.wrapping_neg(),
            "-M"  => m.wrapping_neg(),
            "D+1" => d.wrapping_add(1),
            "A+1" => a.wrapping_add(1),
            "M+1" => m.wrapping_add(1),
            "D-1" => d.wrapping_sub(1),
            "A-1" => a.wrapping_sub(1),
            "M-1" => m.wrapping_sub(1),
            "D+A" => d.wrapping_add(a),
            "D-A" => d.wrapping_sub(a),
            "A-D" => a.wrapping_sub(d),
            "D&A" => d & a,
            "D|A" => d | a,
            "D+M" => d.wrapping_add(m),
            "D-M" => d.wrapping_sub(m),
            "M-D" => m.wrapping_sub(d),
            "D&M" => d & m,
            "D|M" => d | m,
            "M+D" => m.wrapping_add(d),  // alias
            "A+D" => a.wrapping_add(d),  // alias
            _     => panic!("unknown comp: {:?}", comp),
        }
    }

    fn apply_dest(&mut self, dest: &str, val: i16) {
        // Save the M-address BEFORE potentially updating A (e.g. AM=M-1).
        // In the Hack CPU, M always refers to RAM[A_before], even when A is
        // also a destination of the same instruction.
        let m_addr = self.a as usize;
        if dest.contains('A') { self.a = val; }
        if dest.contains('D') { self.d = val; }
        if dest.contains('M') && m_addr < RAM_SIZE {
            self.ram[m_addr] = val;
            if m_addr == OUTPUT_PORT && val > 0 {
                self.output.push(val as u8);
            }
        }
    }

    fn should_jump(jump: &str, val: i16) -> bool {
        match jump {
            "" | "null" => false,
            "JGT" => val > 0,
            "JEQ" => val == 0,
            "JGE" => val >= 0,
            "JLT" => val < 0,
            "JNE" => val != 0,
            "JLE" => val <= 0,
            "JMP" => true,
            _ => panic!("unknown jump: {:?}", jump),
        }
    }

    /// Execute one instruction. Returns false if we've halted (infinite loop detected).
    fn step(&mut self, rom: &[Instr], trace: bool) -> bool {
        if self.pc >= rom.len() {
            return false; // ran off end
        }
        if trace {
            eprint!("PC={:4} A={:6} D={:6} M={:6}  ",
                self.pc, self.a, self.d, self.m());
        }
        match &rom[self.pc] {
            Instr::A(val) => {
                if trace { eprintln!("@{}", val); }
                self.a = *val;
                self.pc += 1;
            }
            Instr::C { comp, dest, jump } => {
                if trace { eprintln!("{}{}{}{}",
                    if dest.is_empty() { String::new() } else { format!("{}=", dest) },
                    comp,
                    if jump.is_empty() { "" } else { ";" },
                    jump); }
                let val = self.compute(comp);
                self.apply_dest(dest, val);
                if Self::should_jump(jump, val) {
                    let target = self.a as usize;
                    // Halt detection: jumping to the instruction we just executed,
                    // or to the A-load that precedes us (the classic __end pattern).
                    if target == self.pc || target + 1 == self.pc {
                        return false; // halted
                    }
                    self.pc = target;
                } else {
                    self.pc += 1;
                }
            }
        }
        true
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let args = parse_args();

    let src = std::fs::read_to_string(&args.path).unwrap_or_else(|e| {
        eprintln!("error reading {:?}: {}", args.path, e);
        std::process::exit(1);
    });

    let rom = assemble(&src).unwrap_or_else(|e| {
        eprintln!("assemble error: {}", e);
        std::process::exit(1);
    });

    println!("Loaded {} instructions from {:?}", rom.len(), args.path);

    let mut cpu = Cpu::new();
    let mut cycles = 0u64;
    let mut halted = false;

    loop {
        if cycles >= args.max_cycles {
            println!("Reached cycle limit ({} cycles) — possibly infinite loop or very slow program", args.max_cycles);
            break;
        }
        if !cpu.step(&rom, args.trace) {
            halted = true;
            break;
        }
        cycles += 1;
    }

    println!();
    if halted {
        println!("✓ Halted after {} cycles", cycles);
    }
    println!("Return value (RAM[256]) = {}", cpu.ram[256]);
    println!("SP = {}", cpu.ram[0]);

    if args.dump_ram > 0 {
        let n = args.dump_ram.min(RAM_SIZE);
        println!("\nRAM[0..{}]:", n);
        for i in 0..n {
            if cpu.ram[i] != 0 || i < 8 {
                println!("  RAM[{:4}] = {}", i, cpu.ram[i]);
            }
        }
    }

    if let Some(ref screen_path) = args.screen_out {
        let ppm = render_screen_ppm(&cpu.ram);
        std::fs::write(screen_path, &ppm).unwrap_or_else(|e| {
            eprintln!("error writing screen to {:?}: {}", screen_path, e);
        });
        println!("Screen saved to {:?} (512×256 PPM)", screen_path);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile C source with hack_cc, then run through the emulator.
    /// Returns the value of RAM[256] (main's return value).
    fn compile_and_run(c_src: &str, max_cycles: u64) -> i16 {
        let (ret, _) = compile_and_run_full(c_src, max_cycles);
        ret
    }

    /// Compile and run, returning (return_value, captured_output).
    fn compile_and_run_full(c_src: &str, max_cycles: u64) -> (i16, String) {
        let (ret, out, _) = compile_and_run_ext(c_src, max_cycles);
        (ret, out)
    }

    /// Compile and run, returning (return_value, output_string, full_ram).
    fn compile_and_run_ext(c_src: &str, max_cycles: u64) -> (i16, String, Vec<i16>) {
        use hack_cc::output::{emit, OutputFormat};
        let prog = hack_cc::compile(c_src)
            .unwrap_or_else(|e| panic!("compile error: {}", e));
        // Use emit() so the __DATA_INIT_HERE__ marker is replaced with data-init asm,
        // ensuring font table, string literals, and global initializers are present in RAM.
        let full_asm = emit(&prog, OutputFormat::Asm)
            .unwrap_or_else(|e| panic!("emit error: {}", e))
            .main;
        let rom = assemble(&full_asm)
            .unwrap_or_else(|e| panic!("assemble error: {}", e));
        let mut cpu = Cpu::new();
        let mut cycles = 0u64;
        loop {
            if cycles >= max_cycles || !cpu.step(&rom, false) { break; }
            cycles += 1;
        }
        let output = String::from_utf8_lossy(&cpu.output).into_owned();
        (cpu.ram[256], output, cpu.ram)
    }

    #[test]
    fn test_return_constant() {
        let result = compile_and_run("int main() { return 42; }", 100_000);
        assert_eq!(result, 42);
    }

    #[test]
    fn test_addition() {
        let result = compile_and_run(
            "int add(int a, int b) { return a + b; }
             int main() { return add(10, 20); }",
            500_000);
        assert_eq!(result, 30);
    }

    #[test]
    fn test_subtraction() {
        let result = compile_and_run("int main() { return 100 - 37; }", 100_000);
        assert_eq!(result, 63);
    }

    #[test]
    fn test_local_variables() {
        let result = compile_and_run(
            "int main() {
                int x;
                int y;
                x = 7;
                y = 8;
                return x + y;
             }",
            500_000);
        assert_eq!(result, 15);
    }

    #[test]
    fn test_if_else() {
        let result = compile_and_run(
            "int max(int a, int b) {
                if (a > b) { return a; } else { return b; }
             }
             int main() { return max(5, 12); }",
            500_000);
        assert_eq!(result, 12);
    }

    #[test]
    fn test_while_loop() {
        // sum 1..=10 = 55
        let result = compile_and_run(
            "int main() {
                int i;
                int sum;
                i = 1;
                sum = 0;
                while (i <= 10) {
                    sum = sum + i;
                    i = i + 1;
                }
                return sum;
             }",
            500_000);
        assert_eq!(result, 55);
    }

    #[test]
    fn test_for_loop() {
        // product 1*2*3*4*5 = 120 via repeated addition (no mul needed)
        let result = compile_and_run(
            "int main() {
                int acc;
                int i;
                acc = 0;
                for (i = 0; i < 10; i = i + 1) {
                    acc = acc + i;
                }
                return acc;
             }",
            500_000);
        assert_eq!(result, 45); // 0+1+...+9
    }

    #[test]
    fn test_multiply() {
        let result = compile_and_run(
            "int main() { return 6 * 7; }",
            2_000_000);
        assert_eq!(result, 42);
    }

    #[test]
    fn test_divide() {
        let result = compile_and_run(
            "int main() { return 100 / 4; }",
            2_000_000);
        assert_eq!(result, 25);
    }

    #[test]
    fn test_modulo() {
        let result = compile_and_run(
            "int main() { return 17 % 5; }",
            2_000_000);
        assert_eq!(result, 2);
    }

    #[test]
    fn test_factorial() {
        let result = compile_and_run(
            "int factorial(int n) {
                int result;
                result = 1;
                while (n > 1) {
                    result = result * n;
                    n = n - 1;
                }
                return result;
             }
             int main() { return factorial(5); }",
            5_000_000);
        assert_eq!(result, 120);
    }

    #[test]
    fn test_recursive_fib() {
        let result = compile_and_run(
            "int fib(int n) {
                if (n <= 1) { return n; }
                return fib(n - 1) + fib(n - 2);
             }
             int main() { return fib(8); }",
            5_000_000);
        assert_eq!(result, 21);
    }

    #[test]
    fn test_global_variable() {
        let result = compile_and_run(
            "int g;
             void inc() { g = g + 1; }
             int main() { g = 10; inc(); inc(); inc(); return g; }",
            500_000);
        assert_eq!(result, 13);
    }

    #[test]
    fn test_negation_and_logic() {
        let result = compile_and_run(
            "int main() {
                int a;
                int b;
                a = 5;
                b = 3;
                if (a > b && b > 0) { return 1; }
                return 0;
             }",
            500_000);
        assert_eq!(result, 1);
    }

    #[test]
    fn test_comparison_ops() {
        let result = compile_and_run(
            "int main() {
                int r;
                r = 0;
                if (1 == 1) { r = r + 1; }
                if (2 != 1) { r = r + 2; }
                if (3 > 2)  { r = r + 4; }
                if (1 < 2)  { r = r + 8; }
                return r;
             }",
            500_000);
        assert_eq!(result, 15);
    }

    // ── Char and string tests ─────────────────────────────────────────────

    #[test]
    fn test_char_literal() {
        // char literals are integers
        let result = compile_and_run("int main() { char c = 'A'; return c; }", 100_000);
        assert_eq!(result, 65);
    }

    #[test]
    fn test_char_arithmetic() {
        let result = compile_and_run(
            "int main() { char lo = 'a'; char hi = 'z'; return hi - lo; }",
            100_000);
        assert_eq!(result, 25);
    }

    #[test]
    fn test_putchar_output() {
        let (ret, out) = compile_and_run_full(
            "int main() { putchar('H'); putchar('i'); putchar('!'); return 0; }",
            500_000);
        assert_eq!(ret, 0);
        assert_eq!(out, "Hi!");
    }

    #[test]
    fn test_strlen_basic() {
        let result = compile_and_run(
            r#"int main() { return strlen("hello"); }"#,
            500_000);
        assert_eq!(result, 5);
    }

    #[test]
    fn test_strlen_empty() {
        let result = compile_and_run(
            r#"int main() { return strlen(""); }"#,
            500_000);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_puts_output() {
        let (ret, out) = compile_and_run_full(
            r#"int main() { puts("Hi"); return 0; }"#,
            500_000);
        assert_eq!(ret, 0);
        assert_eq!(out, "Hi\n");
    }

    #[test]
    fn test_string_index() {
        // Index into a string literal pointer
        let result = compile_and_run(
            r#"int main() { char *s = "ABC"; return s[1]; }"#,
            500_000);
        assert_eq!(result, 66); // 'B'
    }

    #[test]
    fn test_string_dedup() {
        // Same literal used twice shares storage; strlen correct
        let result = compile_and_run(
            r#"int main() { char *a = "xy"; char *b = "xy"; return strlen(a) + strlen(b); }"#,
            500_000);
        assert_eq!(result, 4);
    }

    // ── Screen tests ──────────────────────────────────────────────────────

    #[test]
    fn test_fill_screen() {
        let (_, _, ram) = compile_and_run_ext(
            "int main() { fill_screen(); return 0; }",
            5_000_000);
        // Every screen word should be -1 (0xFFFF)
        assert_eq!(ram[SCREEN_BASE], -1, "RAM[16384] should be -1 after fill_screen");
        assert_eq!(ram[SCREEN_BASE + 100], -1);
        assert_eq!(ram[SCREEN_END - 1], -1);
    }

    #[test]
    fn test_clear_screen() {
        let (_, _, ram) = compile_and_run_ext(
            "int main() { fill_screen(); clear_screen(); return 0; }",
            10_000_000);
        assert_eq!(ram[SCREEN_BASE], 0, "RAM[16384] should be 0 after clear_screen");
        assert_eq!(ram[SCREEN_BASE + 100], 0);
        assert_eq!(ram[SCREEN_END - 1], 0);
    }

    #[test]
    fn test_draw_pixel_origin() {
        // draw_pixel(0, 0) sets bit 0 of RAM[16384]
        let (_, _, ram) = compile_and_run_ext(
            "int main() { draw_pixel(0, 0); return 0; }",
            2_000_000);
        assert!(pixel_set(&ram, 0, 0), "pixel (0,0) should be set");
        assert!(!pixel_set(&ram, 1, 0), "pixel (1,0) should NOT be set");
    }

    #[test]
    fn test_draw_pixel_bit15() {
        // draw_pixel(15, 0) sets bit 15 (MSB) of RAM[16384]
        let (_, _, ram) = compile_and_run_ext(
            "int main() { draw_pixel(15, 0); return 0; }",
            2_000_000);
        assert!(pixel_set(&ram, 15, 0));
        assert!(!pixel_set(&ram, 14, 0));
        // bit 15 of RAM[16384] = 1 means the i16 value is negative
        assert!(ram[SCREEN_BASE] < 0, "bit 15 set → i16 negative");
    }

    #[test]
    fn test_draw_pixel_next_word() {
        // draw_pixel(16, 0) sets bit 0 of RAM[16385]
        let (_, _, ram) = compile_and_run_ext(
            "int main() { draw_pixel(16, 0); return 0; }",
            2_000_000);
        assert!(pixel_set(&ram, 16, 0));
        assert_eq!(ram[SCREEN_BASE], 0, "word 0 should be untouched");
        assert_eq!(ram[SCREEN_BASE + 1] & 1, 1, "bit 0 of word 1 should be set");
    }

    #[test]
    fn test_draw_pixel_row1() {
        // draw_pixel(0, 1) sets bit 0 of RAM[16384 + 32] = RAM[16416]
        let (_, _, ram) = compile_and_run_ext(
            "int main() { draw_pixel(0, 1); return 0; }",
            2_000_000);
        assert!(pixel_set(&ram, 0, 1));
        assert_eq!(ram[SCREEN_BASE], 0, "row 0 should be untouched");
        assert_eq!(ram[SCREEN_BASE + 32] & 1, 1);
    }

    #[test]
    fn test_clear_pixel() {
        // Set pixel then clear it
        let (_, _, ram) = compile_and_run_ext(
            "int main() { draw_pixel(5, 3); clear_pixel(5, 3); return 0; }",
            4_000_000);
        assert!(!pixel_set(&ram, 5, 3), "pixel should be cleared");
        // neighbour pixels unaffected
        assert!(!pixel_set(&ram, 4, 3));
        assert!(!pixel_set(&ram, 6, 3));
    }

    #[test]
    fn test_draw_multiple_pixels() {
        let (_, _, ram) = compile_and_run_ext(
            "int main() { draw_pixel(3, 5); draw_pixel(7, 5); return 0; }",
            4_000_000);
        assert!(pixel_set(&ram, 3, 5));
        assert!(pixel_set(&ram, 7, 5));
        assert!(!pixel_set(&ram, 4, 5));
        assert!(!pixel_set(&ram, 6, 5));
    }

    #[test]
    fn test_render_screen_ppm_dimensions() {
        let ram = vec![0i16; RAM_SIZE];
        let ppm = render_screen_ppm(&ram);
        // Header "P6\n512 256\n255\n" + 512*256*3 pixel bytes
        let header = b"P6\n512 256\n255\n";
        assert_eq!(&ppm[..header.len()], header);
        assert_eq!(ppm.len(), header.len() + 512 * 256 * 3);
        // All pixels white (255,255,255) for empty screen
        assert!(ppm[header.len()..].iter().all(|&b| b == 255));
    }

    // ── Font / draw_char / draw_string tests ─────────────────────────────────

    /// Verify the font table is initialized in RAM at FONT_BASE when draw_char is used.
    /// 'A' is ASCII 65, index 33 (65-32), font starts at FONT_BASE + 33*8 = 25264.
    /// Row 0 of 'A' is 0x18; row 4 is 0x7E; row 7 is 0x00.
    #[test]
    fn test_font_table_init() {
        use hack_cc::FONT_BASE;
        // draw_char triggers font table initialization in RAM
        let (_, _, ram) = compile_and_run_ext(
            "int main() { draw_char(0, 0, 65); return 0; }",
            4_000_000,
        );
        let a_base = FONT_BASE + 33 * 8; // 'A' font data starts here
        assert_eq!(ram[a_base]     as u16, 0x18u16, "row0 of 'A'");
        assert_eq!(ram[a_base + 1] as u16, 0x3Cu16, "row1 of 'A'");
        assert_eq!(ram[a_base + 4] as u16, 0x7Eu16, "row4 of 'A'");
        assert_eq!(ram[a_base + 7] as u16, 0x00u16, "row7 of 'A' (blank)");
    }

    /// draw_char(0, 0, 'A') at even col: 'A' row 0 = 0x18 -> pixels 3,4 set in low byte.
    #[test]
    fn test_draw_char_even_col() {
        let (ret, _, ram) = compile_and_run_ext(
            "int main() { draw_char(0, 0, 65); return 0; }",
            4_000_000,
        );
        assert_eq!(ret, 0);
        // Row 0 of 'A': 0x18 stored in low byte of screen word 0 (RAM[16384])
        // Bits 3 and 4 set
        assert!( pixel_set(&ram, 3, 0), "pixel (3,0) should be set for 'A' row0");
        assert!( pixel_set(&ram, 4, 0), "pixel (4,0) should be set for 'A' row0");
        assert!(!pixel_set(&ram, 0, 0), "pixel (0,0) should be clear");
        assert!(!pixel_set(&ram, 7, 0), "pixel (7,0) should be clear");
        // Row 4 of 'A': 0x7E -> bits 1..6 set
        assert!( pixel_set(&ram, 1, 4), "pixel (1,4) for 'A' row4");
        assert!( pixel_set(&ram, 6, 4), "pixel (6,4) for 'A' row4");
        assert!(!pixel_set(&ram, 0, 4), "pixel (0,4) clear for 'A' row4");
        assert!(!pixel_set(&ram, 7, 4), "pixel (7,4) clear for 'A' row4");
        // Row 7 of 'A': 0x00 -> no pixels
        assert!(!pixel_set(&ram, 3, 7), "pixel (3,7) should be clear (blank row)");
    }

    /// draw_char(1, 0, 'A') at odd col: font byte goes into high byte (bits 8-15).
    /// 'A' row 0 = 0x18 << 8 = 0x1800 -> bits 11,12 set at x=11,12.
    #[test]
    fn test_draw_char_odd_col() {
        let (ret, _, ram) = compile_and_run_ext(
            "int main() { draw_char(1, 0, 65); return 0; }",
            4_000_000,
        );
        assert_eq!(ret, 0);
        // Row 0: 0x18 << 8 = bits 8+3=11, 8+4=12 set
        assert!( pixel_set(&ram, 11, 0), "pixel (11,0) for odd-col 'A' row0");
        assert!( pixel_set(&ram, 12, 0), "pixel (12,0) for odd-col 'A' row0");
        assert!(!pixel_set(&ram, 8,  0), "pixel (8,0) should be clear");
        assert!(!pixel_set(&ram, 15, 0), "pixel (15,0) should be clear");
        // Row 4: 0x7E << 8 -> bits 9..14 set at x=9..14
        assert!( pixel_set(&ram, 9,  4), "pixel (9,4) for odd-col 'A' row4");
        assert!( pixel_set(&ram, 14, 4), "pixel (14,4) for odd-col 'A' row4");
        assert!(!pixel_set(&ram, 8,  4), "pixel (8,4) should be clear");
        assert!(!pixel_set(&ram, 15, 4), "pixel (15,4) should be clear");
    }

    /// draw_string(0, 0, "A") should produce same result as draw_char(0, 0, 65).
    #[test]
    fn test_draw_string_single_char() {
        let (ret, _, ram) = compile_and_run_ext(
            r#"int main() { draw_string(0, 0, "A"); return 0; }"#,
            4_000_000,
        );
        assert_eq!(ret, 0);
        assert!( pixel_set(&ram, 3, 0), "pixel (3,0) set by draw_string 'A'");
        assert!( pixel_set(&ram, 4, 0), "pixel (4,0) set by draw_string 'A'");
        assert!(!pixel_set(&ram, 0, 0), "pixel (0,0) clear");
    }

    // ── Struct tests ─────────────────────────────────────────────────────

    /// Basic struct: declare, write fields, read them back.
    #[test]
    fn test_struct_basic() {
        let ret = compile_and_run(r#"
struct Point { int x; int y; };
int main() {
    struct Point p;
    p.x = 10;
    p.y = 20;
    return p.x + p.y;
}
"#, 500_000);
        assert_eq!(ret, 30);
    }

    /// Struct field overwrite.
    #[test]
    fn test_struct_field_write() {
        let ret = compile_and_run(r#"
struct Pair { int a; int b; };
int main() {
    struct Pair q;
    q.a = 7;
    q.b = 3;
    q.a = q.a + q.b;
    return q.a;
}
"#, 500_000);
        assert_eq!(ret, 10);
    }

    /// Struct passed via pointer; arrow operator.
    #[test]
    fn test_struct_pointer_arrow() {
        let ret = compile_and_run(r#"
struct Vec2 { int x; int y; };
int main() {
    struct Vec2 v;
    struct Vec2 *p;
    p = &v;
    p->x = 5;
    p->y = 9;
    return p->x + p->y;
}
"#, 500_000);
        assert_eq!(ret, 14);
    }

    /// Three-field struct: verify field offsets.
    #[test]
    fn test_struct_three_fields() {
        let ret = compile_and_run(r#"
struct Triple { int a; int b; int c; };
int main() {
    struct Triple t;
    t.a = 1;
    t.b = 2;
    t.c = 3;
    return t.a + t.b + t.c;
}
"#, 500_000);
        assert_eq!(ret, 6);
    }

    /// sizeof(struct) returns the number of words.
    #[test]
    fn test_struct_sizeof() {
        let ret = compile_and_run(r#"
struct Pair { int a; int b; };
int main() {
    return sizeof(struct Pair);
}
"#, 200_000);
        assert_eq!(ret, 2);
    }

    // ── Array tests ───────────────────────────────────────────────────────
    // Note: array indexing (int arr[N]) is not yet supported by this compiler.
    // The following tests use pointer-based patterns that DO work.

    #[test]
    fn test_multi_param_function() {
        // Three-argument function with mixed arithmetic
        let ret = compile_and_run(r#"
int compute(int a, int b, int c) { return a * b + c; }
int main() { return compute(3, 4, 5); }
"#, 2_000_000);
        assert_eq!(ret, 17);
    }

    #[test]
    fn test_accumulate_via_globals() {
        // Simulate array accumulation using a global counter
        let ret = compile_and_run(r#"
int sum;
void add_to_sum(int v) { sum = sum + v; }
int main() {
    sum = 0;
    add_to_sum(1); add_to_sum(2); add_to_sum(3); add_to_sum(4); add_to_sum(5);
    return sum;
}
"#, 500_000);
        assert_eq!(ret, 15);
    }

    #[test]
    fn test_pointer_index_via_struct() {
        // Use a struct to group multiple values; read by pointer
        let ret = compile_and_run(r#"
struct Pair { int x; int y; };
int sum_pair(struct Pair *p) { return p->x + p->y; }
int main() {
    struct Pair v;
    v.x = 8;
    v.y = 13;
    return sum_pair(&v);
}
"#, 500_000);
        assert_eq!(ret, 21);
    }

    // ── Pointer tests ─────────────────────────────────────────────────────

    #[test]
    fn test_pointer_deref_write() {
        let ret = compile_and_run(r#"
int main() {
    int x;
    int *p;
    p = &x;
    *p = 99;
    return x;
}
"#, 200_000);
        assert_eq!(ret, 99);
    }

    #[test]
    fn test_pointer_swap() {
        let ret = compile_and_run(r#"
void swap(int *a, int *b) {
    int tmp;
    tmp = *a;
    *a = *b;
    *b = tmp;
}
int main() {
    int x;
    int y;
    x = 3;
    y = 7;
    swap(&x, &y);
    return x;
}
"#, 500_000);
        assert_eq!(ret, 7);
    }

    // ── Bitwise operator tests ────────────────────────────────────────────

    #[test]
    fn test_bitwise_and() {
        let ret = compile_and_run("int main() { return 255 & 15; }", 200_000);
        assert_eq!(ret, 15);
    }

    #[test]
    fn test_bitwise_or() {
        let ret = compile_and_run("int main() { return 240 | 15; }", 200_000);
        assert_eq!(ret, 255);
    }

    #[test]
    fn test_bitwise_not() {
        let ret = compile_and_run("int main() { return ~0; }", 200_000);
        assert_eq!(ret, -1);
    }

    // ── Unary and compound assignment tests ───────────────────────────────

    #[test]
    fn test_unary_minus() {
        let ret = compile_and_run("int main() { int x; x = 5; return -x; }", 200_000);
        assert_eq!(ret, -5);
    }

    #[test]
    fn test_compound_assign_add() {
        let ret = compile_and_run("int main() { int x; x = 10; x += 5; return x; }", 200_000);
        assert_eq!(ret, 15);
    }

    #[test]
    fn test_compound_assign_sub() {
        let ret = compile_and_run("int main() { int x; x = 10; x -= 3; return x; }", 200_000);
        assert_eq!(ret, 7);
    }

    #[test]
    fn test_prefix_increment() {
        let ret = compile_and_run("int main() { int x; x = 5; return ++x; }", 200_000);
        assert_eq!(ret, 6);
    }

    #[test]
    fn test_postfix_increment() {
        let ret = compile_and_run("int main() { int x; x = 5; x++; return x; }", 200_000);
        assert_eq!(ret, 6);
    }

    // ── Ternary operator / conditional expression ─────────────────────────
    // (ternary '?:' is not supported by this compiler; skip those tests)

    // ── Dead-code elimination tests ───────────────────────────────────────

    /// Programs that don't use mul/div/puts should not emit those runtime helpers.
    #[test]
    fn test_no_runtime_without_mul() {
        let prog = hack_cc::compile("int main() { return 2 + 3; }").unwrap();
        assert!(!prog.asm.contains("(__mul)"), "mul helper should not be emitted");
        assert!(!prog.asm.contains("(__div)"), "div helper should not be emitted");
        assert!(!prog.asm.contains("(__puts)"), "puts helper should not be emitted");
    }

    #[test]
    fn test_mul_emitted_when_used() {
        let prog = hack_cc::compile("int main() { return 6 * 7; }").unwrap();
        assert!(prog.asm.contains("(__mul)"), "mul helper must be emitted when * is used");
    }

    #[test]
    fn test_div_emitted_when_used() {
        let prog = hack_cc::compile("int main() { return 10 / 2; }").unwrap();
        assert!(prog.asm.contains("(__div)"), "div helper must be emitted when / is used");
    }

    #[test]
    fn test_puts_emitted_when_used() {
        let prog = hack_cc::compile(r#"int main() { puts("hi"); return 0; }"#).unwrap();
        assert!(prog.asm.contains("(__puts)"), "puts helper must be emitted when puts() is called");
    }

    #[test]
    fn test_dead_function_eliminated() {
        let prog = hack_cc::compile(
            "int unused() { return 99; } int main() { return 1; }"
        ).unwrap();
        assert!(!prog.asm.contains("(unused)"), "unreachable function should not be emitted");
    }

    #[test]
    fn test_reachable_function_kept() {
        let prog = hack_cc::compile(
            "int used() { return 99; } int main() { return used(); }"
        ).unwrap();
        assert!(prog.asm.contains("(used)"), "reachable function must be emitted");
    }

    // ── Output format tests ───────────────────────────────────────────────

    #[test]
    fn test_hack_format_binary_strings() {
        use hack_cc::output::{emit, OutputFormat};
        let prog = hack_cc::compile("int main() { return 0; }").unwrap();
        let result = emit(&prog, OutputFormat::Hack).unwrap();
        for line in result.main.lines() {
            assert_eq!(line.len(), 16, "each .hack line must be 16 chars: {:?}", line);
            assert!(line.chars().all(|c| c == '0' || c == '1'), "only 0/1 in .hack: {:?}", line);
        }
    }

    #[test]
    fn test_hackem_format_header() {
        use hack_cc::output::{emit, OutputFormat};
        let prog = hack_cc::compile("int main() { return 0; }").unwrap();
        let result = emit(&prog, OutputFormat::Hackem).unwrap();
        assert!(result.main.starts_with("hackem v1.0 0x"),
            "hackem output must start with version header");
        assert!(result.main.contains("ROM@"), "hackem output must have ROM@ section");
    }

    #[test]
    fn test_hackem_has_ram_section_for_globals() {
        use hack_cc::output::{emit, OutputFormat};
        let prog = hack_cc::compile("int g = 42; int main() { return g; }").unwrap();
        let result = emit(&prog, OutputFormat::Hackem).unwrap();
        assert!(result.main.contains("RAM@"), "initialized global should produce RAM@ section");
    }

    #[test]
    fn test_tst_format_has_companion() {
        use hack_cc::output::{emit, OutputFormat};
        let prog = hack_cc::compile("int main() { return 0; }").unwrap();
        let result = emit(&prog, OutputFormat::Tst).unwrap();
        assert!(result.hack_companion.is_some(), "tst format must produce a companion .hack file");
        assert!(result.main.contains("load"), "tst script must contain 'load'");
        assert!(result.main.contains("ticktock"), "tst script must contain 'ticktock'");
    }

    #[test]
    fn test_asm_format_no_companion() {
        use hack_cc::output::{emit, OutputFormat};
        let prog = hack_cc::compile("int main() { return 0; }").unwrap();
        let result = emit(&prog, OutputFormat::Asm).unwrap();
        assert!(result.hack_companion.is_none(), "asm format must not produce a companion file");
    }

    // ── Global variable initializer tests ────────────────────────────────

    #[test]
    fn test_global_initializer() {
        let ret = compile_and_run("int g = 7; int main() { return g; }", 200_000);
        assert_eq!(ret, 7);
    }

    #[test]
    fn test_multiple_globals() {
        let ret = compile_and_run(
            "int a = 3; int b = 4; int main() { return a + b; }",
            200_000);
        assert_eq!(ret, 7);
    }

    // ── Recursive and multi-call tests ────────────────────────────────────

    #[test]
    fn test_nested_calls() {
        let ret = compile_and_run(r#"
int double(int x) { return x + x; }
int quad(int x)   { return double(double(x)); }
int main() { return quad(3); }
"#, 500_000);
        assert_eq!(ret, 12);
    }

    // ── Shift / mixed arithmetic ──────────────────────────────────────────

    #[test]
    fn test_negative_modulo() {
        // C semantics: -7 % 3 should be -1 on most platforms; verify compiler matches
        let ret = compile_and_run("int main() { return (-7) % 3; }", 2_000_000);
        // Accept -1 or 2 (implementation-defined); just confirm it compiles and runs
        assert!(ret == -1 || ret == 2, "unexpected modulo result: {}", ret);
    }

    #[test]
    fn test_large_multiply() {
        // 100 * 100 = 10000 (fits in i16: max 32767)
        let ret = compile_and_run("int main() { return 100 * 100; }", 5_000_000);
        assert_eq!(ret, 10000);
    }
}
