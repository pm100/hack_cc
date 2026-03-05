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
    eprintln!("Usage: hack_emu <file.asm> [--max-cycles N] [--dump-ram N] [--trace]");
    std::process::exit(1);
}

struct Args {
    path: PathBuf,
    max_cycles: u64,
    dump_ram: usize,
    trace: bool,
}

fn parse_args() -> Args {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.is_empty() { usage(); }
    let mut path = None;
    let mut max_cycles = 10_000_000u64;
    let mut dump_ram = 0usize;
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
            "--trace" => { trace = true; }
            s if s.starts_with("--") => { eprintln!("unknown flag: {}", s); usage(); }
            s => {
                if path.is_some() { usage(); }
                path = Some(PathBuf::from(s));
            }
        }
        i += 1;
    }
    Args { path: path.unwrap_or_else(|| usage()), max_cycles, dump_ram, trace }
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

struct Cpu {
    a: i16,
    d: i16,
    pc: usize,
    ram: Vec<i16>,
}

impl Cpu {
    fn new() -> Self {
        Self {
            a: 0, d: 0, pc: 0,
            ram: vec![0; RAM_SIZE],
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
        if dest.contains('M') {
            if m_addr < RAM_SIZE { self.ram[m_addr] = val; }
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
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile C source with hack_cc, then run through the emulator.
    /// Returns the value of RAM[256] (main's return value).
    fn compile_and_run(c_src: &str, max_cycles: u64) -> i16 {
        let asm = hack_cc::compile(c_src)
            .unwrap_or_else(|e| panic!("compile error: {}", e));
        let rom = assemble(&asm)
            .unwrap_or_else(|e| panic!("assemble error: {}", e));
        let mut cpu = Cpu::new();
        let mut cycles = 0u64;
        loop {
            if cycles >= max_cycles || !cpu.step(&rom, false) { break; }
            cycles += 1;
        }
        cpu.ram[256]
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
}
