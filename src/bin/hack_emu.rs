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
    /// Quiet mode: only emit putchar output to stdout; exit with main's return value.
    quiet: bool,
}

fn parse_args() -> Args {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.is_empty() { usage(); }
    let mut path = None;
    let mut max_cycles = 10_000_000u64;
    let mut dump_ram = 0usize;
    let mut screen_out = None;
    let mut trace = false;
    let mut quiet = false;
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
            "--quiet" => { quiet = true; }
            s if s.starts_with("--") => { eprintln!("unknown flag: {}", s); usage(); }
            s => {
                if path.is_some() { usage(); }
                path = Some(PathBuf::from(s));
            }
        }
        i += 1;
    }
    Args { path: path.unwrap_or_else(|| usage()), max_cycles, dump_ram, screen_out, trace, quiet }
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
    assemble_with_var_base(src, 16)
}

/// Assemble Hack assembly text, allocating named variables starting at `var_base`.
/// Use `var_base = prog.next_var_addr` when assembling compiler output with C static
/// data at RAM[16..next_var_addr], to prevent runtime named variables from colliding
/// with string literals and global variables.
fn assemble_with_var_base(src: &str, var_base: i16) -> Result<Vec<Instr>, String> {
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
    let mut next_var_addr = var_base;
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

// ── Hackem format support ─────────────────────────────────────────────────────

/// Decode a 16-bit machine word into a Hack instruction.
fn decode_word(w: u16) -> Result<Instr, String> {
    if w & 0x8000 == 0 {
        Ok(Instr::A((w & 0x7FFF) as i16))
    } else {
        let comp_bits = (w >> 6) & 0x7F;
        let dest_bits = (w >> 3) & 0x07;
        let jump_bits = w & 0x07;
        let comp = decode_comp(comp_bits)?;
        let dest = decode_dest(dest_bits).to_string();
        let jump = decode_jump(jump_bits).to_string();
        Ok(Instr::C { comp, dest, jump })
    }
}

fn decode_comp(bits: u16) -> Result<String, String> {
    Ok(match bits {
        0b0_101010 => "0",
        0b0_111111 => "1",
        0b0_111010 => "-1",
        0b0_001100 => "D",
        0b0_110000 => "A",
        0b1_110000 => "M",
        0b0_001101 => "!D",
        0b0_110001 => "!A",
        0b1_110001 => "!M",
        0b0_001111 => "-D",
        0b0_110011 => "-A",
        0b1_110011 => "-M",
        0b0_011111 => "D+1",
        0b0_110111 => "A+1",
        0b1_110111 => "M+1",
        0b0_001110 => "D-1",
        0b0_110010 => "A-1",
        0b1_110010 => "M-1",
        0b0_000010 => "D+A",
        0b1_000010 => "D+M",
        0b0_010011 => "D-A",
        0b1_010011 => "D-M",
        0b0_000111 => "A-D",
        0b1_000111 => "M-D",
        0b0_000000 => "D&A",
        0b1_000000 => "D&M",
        0b0_010101 => "D|A",
        0b1_010101 => "D|M",
        other => return Err(format!("unknown comp bits: 0b{:07b}", other)),
    }.to_string())
}

fn decode_dest(bits: u16) -> &'static str {
    match bits {
        0b000 => "",
        0b001 => "M",
        0b010 => "D",
        0b011 => "MD",
        0b100 => "A",
        0b101 => "AM",
        0b110 => "AD",
        0b111 => "AMD",
        _ => "",
    }
}

fn decode_jump(bits: u16) -> &'static str {
    match bits {
        0b000 => "",
        0b001 => "JGT",
        0b010 => "JEQ",
        0b011 => "JGE",
        0b100 => "JLT",
        0b101 => "JNE",
        0b110 => "JLE",
        0b111 => "JMP",
        _ => "",
    }
}

/// Load a `.hackem` file: returns (ROM instructions, initial RAM contents).
fn load_hackem(src: &str) -> Result<(Vec<Instr>, Vec<i16>), String> {
    let mut lines = src.lines().peekable();

    // Parse header: "hackem v1.0 0xXXXX"
    let header = lines.next().ok_or("empty hackem file")?;
    if !header.starts_with("hackem v1.0") {
        return Err(format!("not a hackem file: {:?}", header));
    }

    let mut rom: Vec<Instr> = Vec::new();
    let mut ram = vec![0i16; RAM_SIZE];
    let mut in_rom = false;
    let mut ram_base: usize = 0;
    let mut ram_cursor: usize = 0;

    for line in lines {
        let line = line.trim();
        if line.is_empty() { continue; }

        if let Some(addr_hex) = line.strip_prefix("ROM@") {
            let _ = u32::from_str_radix(addr_hex, 16)
                .map_err(|_| format!("bad ROM@ address: {}", addr_hex))?;
            in_rom = true;
            continue;
        }

        if let Some(addr_hex) = line.strip_prefix("RAM@") {
            let addr = usize::from_str_radix(addr_hex, 16)
                .map_err(|_| format!("bad RAM@ address: {}", addr_hex))?;
            ram_base = addr;
            ram_cursor = addr;
            in_rom = false;
            continue;
        }

        let word = u16::from_str_radix(line, 16)
            .map_err(|_| format!("bad hex word: {:?}", line))?;

        if in_rom {
            rom.push(decode_word(word)?);
        } else {
            if ram_cursor < RAM_SIZE {
                ram[ram_cursor] = word as i16;
            }
            ram_cursor += 1;
            let _ = ram_base; // suppress unused warning
        }
    }

    Ok((rom, ram))
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

    let (rom, initial_ram) = if src.starts_with("hackem v1.0") {
        load_hackem(&src).unwrap_or_else(|e| {
            eprintln!("hackem load error: {}", e);
            std::process::exit(1);
        })
    } else {
        let instr = assemble(&src).unwrap_or_else(|e| {
            eprintln!("assemble error: {}", e);
            std::process::exit(1);
        });
        (instr, vec![0i16; RAM_SIZE])
    };

    if !args.quiet {
        println!("Loaded {} instructions from {:?}", rom.len(), args.path);
    }

    let mut cpu = Cpu::new();
    // Pre-load RAM for hackem format (font data, string literals, etc.)
    for (i, &v) in initial_ram.iter().enumerate() {
        if v != 0 && i < RAM_SIZE {
            cpu.ram[i] = v;
        }
    }
    let mut cycles = 0u64;
    let mut halted = false;

    loop {
        if cycles >= args.max_cycles {
            if !args.quiet {
                println!("Reached cycle limit ({} cycles) — possibly infinite loop or very slow program", args.max_cycles);
            }
            break;
        }
        if !cpu.step(&rom, args.trace) {
            halted = true;
            break;
        }
        cycles += 1;
    }

    // In quiet mode: print only putchar output, then exit with main's return value.
    if args.quiet {
        print!("{}", String::from_utf8_lossy(&cpu.output));
        std::process::exit((cpu.ram[256] as u8) as i32);
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
        use hack_cc::{CompileOptions, compile_with_full_options};
        use std::path::PathBuf;

        // Prepend #include <hack.h> if the test source doesn't already have it,
        // so test programs can call puts/putchar/strlen/etc. without boilerplate.
        let src_with_header;
        let src: &str = if c_src.contains("#include") {
            c_src
        } else {
            src_with_header = format!("#include <hack.h>\n{}", c_src);
            &src_with_header
        };

        let opts = CompileOptions {
            include_dirs: vec![PathBuf::from("include")],
            ..Default::default()
        };
        let prog = compile_with_full_options(src, None, &opts)
            .unwrap_or_else(|e| panic!("compile error: {}", e));
        // Use emit() so the __DATA_INIT_HERE__ marker is replaced with data-init asm,
        // ensuring font table, string literals, and global initializers are present in RAM.
        let full_asm = emit(&prog, OutputFormat::Asm)
            .unwrap_or_else(|e| panic!("emit error: {}", e))
            .main;
        // All globals use symbolic names (__g_name) so they get unique RAM addresses
        // allocated by the assembler. Start variable allocation at 16 (default).
        let rom = assemble_with_var_base(&full_asm, 16)
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
        // 'A' is char index 33 (65-32), 11 rows per char in Jack OS 8×11 font
        // FONT_8X11[33] = [12,30,51,51,63,51,51,51,51,0,0]
        // Jack OS font uses bit-0=leftmost, matching Hack screen — stored as-is.
        let a_base = FONT_BASE + 33 * 11;
        assert_eq!(ram[a_base]     as u16, 12u16,  "row0 of 'A'");
        assert_eq!(ram[a_base + 1] as u16, 30u16,  "row1 of 'A'");
        assert_eq!(ram[a_base + 4] as u16, 63u16,  "row4 of 'A'");
        assert_eq!(ram[a_base + 9] as u16, 0u16,   "row9 of 'A' (blank)");
    }

    /// draw_char(0, 0, 'A') at even col.
    /// Jack OS 'A' row0=12=0x0C → bits 2,3 set in low byte → pixels (2,0),(3,0).
    #[test]
    fn test_draw_char_even_col() {
        let (ret, _, ram) = compile_and_run_ext(
            "int main() { draw_char(0, 0, 65); return 0; }",
            4_000_000,
        );
        assert_eq!(ret, 0);
        // Row 0 of 'A': 12 = 0x0C stored in low byte → bits 2,3 set → pixels 2,3
        assert!( pixel_set(&ram, 2, 0), "pixel (2,0) should be set for 'A' row0");
        assert!( pixel_set(&ram, 3, 0), "pixel (3,0) should be set for 'A' row0");
        assert!(!pixel_set(&ram, 0, 0), "pixel (0,0) should be clear");
        assert!(!pixel_set(&ram, 7, 0), "pixel (7,0) should be clear");
        // Row 4 of 'A': 63 = 0x3F -> bits 0..5 set → pixels 0..5
        assert!( pixel_set(&ram, 0, 4), "pixel (0,4) for 'A' row4");
        assert!( pixel_set(&ram, 5, 4), "pixel (5,4) for 'A' row4");
        assert!(!pixel_set(&ram, 6, 4), "pixel (6,4) should be clear for 'A' row4");
        assert!(!pixel_set(&ram, 7, 4), "pixel (7,4) should be clear for 'A' row4");
        // Row 9 of 'A': 0 -> no pixels
        assert!(!pixel_set(&ram, 2, 9), "pixel (2,9) should be clear (blank row)");
    }

    /// draw_char(1, 0, 'A') at odd col: font byte goes into high byte (bits 8-15).
    /// 'A' row 0 = 12 = 0x0C → high byte = 0x0C00 → bits 10,11 set → pixels 10,11.
    #[test]
    fn test_draw_char_odd_col() {
        let (ret, _, ram) = compile_and_run_ext(
            "int main() { draw_char(1, 0, 65); return 0; }",
            4_000_000,
        );
        assert_eq!(ret, 0);
        // Row 0: 12 in high byte → bits 8+2=10, 8+3=11 set
        assert!( pixel_set(&ram, 10, 0), "pixel (10,0) for odd-col 'A' row0");
        assert!( pixel_set(&ram, 11, 0), "pixel (11,0) for odd-col 'A' row0");
        assert!(!pixel_set(&ram,  8, 0), "pixel (8,0) should be clear");
        assert!(!pixel_set(&ram, 15, 0), "pixel (15,0) should be clear");
        // Row 4: 63 in high byte → bits 8..13 set → pixels 8..13
        assert!( pixel_set(&ram,  8, 4), "pixel (8,4) for odd-col 'A' row4");
        assert!( pixel_set(&ram, 13, 4), "pixel (13,4) for odd-col 'A' row4");
        assert!(!pixel_set(&ram, 14, 4), "pixel (14,4) should be clear");
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
        assert!( pixel_set(&ram, 2, 0), "pixel (2,0) set by draw_string 'A'");
        assert!( pixel_set(&ram, 3, 0), "pixel (3,0) set by draw_string 'A'");
        assert!(!pixel_set(&ram, 7, 0), "pixel (7,0) clear");
    }

    /// Regression test: puts_screen() with a string literal must not have
    /// address collision between C data (string at RAM[16+]) and runtime named
    /// variables (__con_col, __con_row, etc. also allocated near RAM[16]).
    /// Before the fix, __con_row ended up at the address holding ASCII ',' from
    /// "Hello, World!" → row=44 → no pixels were rendered (blank screen).
    #[test]
    fn test_puts_screen_no_collision() {
        use hack_cc::{CompileOptions, compile_with_full_options};
        use hack_cc::output::{emit, OutputFormat};
        use std::path::PathBuf;
        let mut opts = CompileOptions {
            include_dirs: vec![PathBuf::from("include")],
            ..Default::default()
        };
        opts.defines.insert("HACK_OUTPUT_SCREEN".to_string(), "1".to_string());
        let prog = compile_with_full_options(
            r#"#include <hack.h>
int main() { puts_screen("Hello, World!"); return 0; }"#,
            None, &opts,
        ).unwrap_or_else(|e| panic!("compile error: {}", e));
        let full_asm = emit(&prog, OutputFormat::Asm)
            .unwrap_or_else(|e| panic!("emit error: {}", e))
            .main;
        let rom = assemble_with_var_base(&full_asm, 16)
            .unwrap_or_else(|e| panic!("assemble error: {}", e));
        let mut cpu = Cpu::new();
        let mut cycles = 0u64;
        loop {
            if cycles >= 8_000_000 || !cpu.step(&rom, false) { break; }
            cycles += 1;
        }
        let ram = cpu.ram;
        assert_eq!(ram[256], 0);
        // At least some pixels must be set: "Hello" starts at col=0, row=0.
        // 'H' row0=0x33=51 → bits 0,1,4,5 set → pixels (0,0) and (1,0).
        assert!(pixel_set(&ram, 0, 0), "pixel (0,0) should be set for 'H' row0 (no var collision)");
        assert!(pixel_set(&ram, 1, 0), "pixel (1,0) should be set for 'H' row0 (no var collision)");
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
        use hack_cc::{CompileOptions, compile_with_full_options};
        use std::path::PathBuf;
        let opts = CompileOptions { include_dirs: vec![PathBuf::from("include")], ..Default::default() };
        let prog = compile_with_full_options(
            r#"#include <hack.h>
int main() { puts("hi"); return 0; }"#, None, &opts).unwrap();
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
    fn test_hackem_global_initialized_in_bootstrap() {
        use hack_cc::output::{emit, OutputFormat};
        let prog = hack_cc::compile("int g = 42; int main() { return g; }").unwrap();
        // Globals are now initialized inline in bootstrap code (not RAM@ sections).
        // Verify the assembly contains the symbol and initializer value.
        let result = emit(&prog, OutputFormat::Asm).unwrap();
        assert!(result.main.contains("@__g_g"), "global should produce symbolic reference");
        assert!(result.main.contains("@42"), "global initializer value should appear in bootstrap");
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
