/// Two-pass Hack assembler: converts Hack assembly text into 16-bit machine words.
///
/// Instruction encoding:
///   A-instruction: 0vvv_vvvv_vvvv_vvvv  (15-bit value, MSB = 0)
///   C-instruction: 111a_cccc_ccdd_djjj   (MSB = 1)

use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
#[error("assembler error: {0}")]
pub struct AssembleError(pub String);

impl AssembleError {
    fn new(msg: impl Into<String>) -> Self { Self(msg.into()) }
}

/// Assemble Hack assembly source text into a vector of 16-bit machine words.
pub fn assemble(asm: &str) -> Result<Vec<u16>, AssembleError> {
    // Predefined symbols
    let mut symbols: HashMap<String, u16> = HashMap::new();
    symbols.insert("SP".into(), 0);
    symbols.insert("LCL".into(), 1);
    symbols.insert("ARG".into(), 2);
    symbols.insert("THIS".into(), 3);
    symbols.insert("THAT".into(), 4);
    for i in 0u16..=15 {
        symbols.insert(format!("R{}", i), i);
    }
    symbols.insert("SCREEN".into(), 16384);
    symbols.insert("KBD".into(), 24576);

    // Collect lines, stripping comments and blank lines
    let lines: Vec<&str> = asm.lines().collect();

    // Pass 1: collect label addresses
    let mut rom_addr: u16 = 0;
    for line in &lines {
        let line = strip_comment(line).trim();
        if line.is_empty() { continue; }
        if let Some(label) = line.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
            symbols.insert(label.to_string(), rom_addr);
        } else {
            rom_addr = rom_addr.checked_add(1).ok_or_else(|| {
                AssembleError::new("program too large for Hack 32K ROM")
            })?;
        }
    }

    // Pass 2: emit machine words
    let mut code: Vec<u16> = Vec::with_capacity(rom_addr as usize);
    let mut var_addr: u16 = 16; // unresolved symbols allocated here

    for line in &lines {
        let line = strip_comment(line).trim();
        if line.is_empty() { continue; }
        if line.starts_with('(') { continue; } // label definition, not an instruction

        let word = if let Some(sym) = line.strip_prefix('@') {
            // A-instruction
            let value: u16 = if let Ok(n) = sym.parse::<u16>() {
                n
            } else if let Some(&addr) = symbols.get(sym) {
                addr
            } else {
                // Allocate new variable in RAM
                let addr = var_addr;
                symbols.insert(sym.to_string(), addr);
                var_addr += 1;
                addr
            };
            value & 0x7FFF // ensure MSB = 0
        } else {
            // C-instruction
            parse_c_instruction(line)?
        };

        code.push(word);
    }

    Ok(code)
}

fn strip_comment(line: &str) -> &str {
    if let Some(idx) = line.find("//") { &line[..idx] } else { line }
}

fn parse_c_instruction(line: &str) -> Result<u16, AssembleError> {
    // Syntax: [dest=]comp[;jump]
    let (dest_str, rest) = if let Some(idx) = line.find('=') {
        (&line[..idx], &line[idx + 1..])
    } else {
        ("", line)
    };

    let (comp_str, jump_str) = if let Some(idx) = rest.find(';') {
        (&rest[..idx], &rest[idx + 1..])
    } else {
        (rest, "")
    };

    let dest = parse_dest(dest_str.trim())?;
    let comp = parse_comp(comp_str.trim())?;
    let jump = parse_jump(jump_str.trim())?;

    Ok(0b1110_0000_0000_0000 | (comp << 6) | (dest << 3) | jump)
}

fn parse_dest(s: &str) -> Result<u16, AssembleError> {
    Ok(match s {
        ""          => 0b000,
        "M"         => 0b001,
        "D"         => 0b010,
        "MD" | "DM" => 0b011,
        "A"         => 0b100,
        "AM" | "MA" => 0b101,
        "AD" | "DA" => 0b110,
        "AMD" | "ADM" | "MAD" | "MDA" | "DAM" | "DMA" => 0b111,
        _ => return Err(AssembleError::new(format!("unknown dest: '{}'", s))),
    })
}

fn parse_comp(s: &str) -> Result<u16, AssembleError> {
    // 7-bit field: a-bit (bit 6) selects A vs M, bits 5-0 are the ALU control bits
    Ok(match s {
        "0"    => 0b0_101010,
        "1"    => 0b0_111111,
        "-1"   => 0b0_111010,
        "D"    => 0b0_001100,
        "A"    => 0b0_110000,
        "M"    => 0b1_110000,
        "!D"   => 0b0_001101,
        "!A"   => 0b0_110001,
        "!M"   => 0b1_110001,
        "-D"   => 0b0_001111,
        "-A"   => 0b0_110011,
        "-M"   => 0b1_110011,
        "D+1"  => 0b0_011111,
        "A+1"  => 0b0_110111,
        "M+1"  => 0b1_110111,
        "D-1"  => 0b0_001110,
        "A-1"  => 0b0_110010,
        "M-1"  => 0b1_110010,
        "D+A" | "A+D" => 0b0_000010,
        "D+M" | "M+D" => 0b1_000010,
        "D-A"  => 0b0_010011,
        "D-M"  => 0b1_010011,
        "A-D"  => 0b0_000111,
        "M-D"  => 0b1_000111,
        "D&A" | "A&D" => 0b0_000000,
        "D&M" | "M&D" => 0b1_000000,
        "D|A" | "A|D" => 0b0_010101,
        "D|M" | "M|D" => 0b1_010101,
        _ => return Err(AssembleError::new(format!("unknown comp: '{}'", s))),
    })
}

fn parse_jump(s: &str) -> Result<u16, AssembleError> {
    Ok(match s {
        "" | "null" => 0b000,
        "JGT"       => 0b001,
        "JEQ"       => 0b010,
        "JGE"       => 0b011,
        "JLT"       => 0b100,
        "JNE"       => 0b101,
        "JLE"       => 0b110,
        "JMP"       => 0b111,
        _ => return Err(AssembleError::new(format!("unknown jump condition: '{}'", s))),
    })
}
