/// Output format module: converts a `CompiledProgram` into the desired file format.
///
/// Four formats are supported:
///   asm    – Hack assembly text (data initialised by bootstrap code)
///   hackem – hackem binary format with ROM@ and RAM@ sections
///   hack   – nand2tetris .hack binary (16-bit binary strings per line)
///   tst    – nand2tetris test script (.tst) + companion .hack binary

use crate::{CompiledProgram, DataInit};
use crate::assembler;

/// The four supported output formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Hack assembly text; data initialised by bootstrap code (default).
    Asm,
    /// hackem v1.0 binary: `ROM@` section (hex) + `RAM@` sections for data.
    Hackem,
    /// nand2tetris .hack binary: 16-bit binary strings, data in bootstrap code.
    Hack,
    /// nand2tetris test script + companion .hack binary (data via `set RAM[]` commands).
    Tst,
}

/// Result of `emit()`. For `Tst` format there is a companion `.hack` file.
pub struct EmitResult {
    /// Primary output file content.
    pub main: String,
    /// For `Tst` format: the `.hack` binary that the `.tst` script loads.
    pub hack_companion: Option<String>,
}

/// Convert a compiled program to the requested output format.
pub fn emit(prog: &CompiledProgram, format: OutputFormat) -> Result<EmitResult, String> {
    match format {
        OutputFormat::Asm    => emit_asm(prog),
        OutputFormat::Hackem => emit_hackem(prog),
        OutputFormat::Hack   => emit_hack(prog),
        OutputFormat::Tst    => emit_tst(prog),
    }
}

// ── Format emitters ──────────────────────────────────────────────────────────

fn emit_asm(prog: &CompiledProgram) -> Result<EmitResult, String> {
    Ok(EmitResult { main: prog.asm.clone(), hack_companion: None })
}

fn emit_hack(prog: &CompiledProgram) -> Result<EmitResult, String> {
    let words = assembler::assemble_with_base(&prog.asm, 16).map_err(|e| e.to_string())?;
    let binary = words_to_binary_strings(&words);
    Ok(EmitResult { main: binary, hack_companion: None })
}

fn emit_hackem(prog: &CompiledProgram) -> Result<EmitResult, String> {
    // Assemble the full program (init code is in the bootstrap)
    let words = assembler::assemble_with_base(&prog.asm, 16).map_err(|e| e.to_string())?;

    // Locate the halt address (ROM address of the `(__end)` label)
    let halt = find_label_addr(&prog.asm, "__end").unwrap_or(0);

    let mut out = format!("hackem v1.0 0x{:04x}\n", halt);
    out.push_str("ROM@0000\n");
    for w in &words {
        out.push_str(&format!("{:04x}\n", w));
    }

    // Emit RAM@ sections for font table data only
    emit_ram_sections(&prog.data, &mut out);

    Ok(EmitResult { main: out, hack_companion: None })
}

fn emit_tst(prog: &CompiledProgram) -> Result<EmitResult, String> {
    // The .hack companion: full program (init code is inline in bootstrap)
    let words = assembler::assemble_with_base(&prog.asm, 16).map_err(|e| e.to_string())?;
    let binary = words_to_binary_strings(&words);

    // Build the .tst script
    let mut tst = String::new();
    tst.push_str("// Auto-generated nand2tetris test script\n");
    tst.push_str("load prog.hack,\n");
    tst.push_str("output-file prog.out,\n");
    tst.push_str("output-list RAM[0]%D1.6.1;\n\n");

    // Pre-load font table data via set commands (globals/strings init in bootstrap code)
    let mut sorted: Vec<(u16, i16)> = prog.data.iter()
        .filter(|d| d.value != 0)
        .map(|d| (d.address, d.value))
        .collect();
    sorted.sort_by_key(|&(addr, _)| addr);

    if !sorted.is_empty() {
        tst.push_str("// Pre-load font table into RAM\n");
        for (addr, val) in &sorted {
            tst.push_str(&format!("set RAM[{}] {},\n", addr, val));
        }
        tst.push('\n');
    }

    tst.push_str("set PC 0,\n\n");
    tst.push_str("repeat 100000 {\n");
    tst.push_str("  ticktock;\n");
    tst.push_str("}\n\n");
    tst.push_str("output;\n");

    Ok(EmitResult { main: tst, hack_companion: Some(binary) })
}

// ── Utilities ────────────────────────────────────────────────────────────────

fn words_to_binary_strings(words: &[u16]) -> String {
    let mut out = String::with_capacity(words.len() * 17);
    for &w in words {
        out.push_str(&format!("{:016b}\n", w));
    }
    out
}

/// Scan assembly text for a label definition `(NAME)` and return its ROM address.
fn find_label_addr(asm: &str, label: &str) -> Option<u16> {
    let target = format!("({})", label);
    let mut rom_addr: u16 = 0;
    for line in asm.lines() {
        let line = if let Some(i) = line.find("//") { &line[..i] } else { line }.trim();
        if line.is_empty() { continue; }
        if line.starts_with('.') { continue; }
        if line == target { return Some(rom_addr); }
        if !line.starts_with('(') { rom_addr = rom_addr.saturating_add(1); }
    }
    None
}

/// Emit RAM@ sections grouping data entries into contiguous blocks.
/// Entries within GAP_THRESHOLD words of each other share a section (gap filled with zeros).
fn emit_ram_sections(data: &[DataInit], out: &mut String) {
    let mut entries: Vec<(u16, u16)> = data.iter()
        .filter(|d| d.value != 0)
        .map(|d| (d.address, d.value as u16))
        .collect();
    entries.sort_by_key(|&(addr, _)| addr);
    if entries.is_empty() { return; }

    const GAP_THRESHOLD: u16 = 16;

    let mut i = 0;
    while i < entries.len() {
        let section_start = entries[i].0;
        out.push_str(&format!("RAM@{:04x}\n", section_start));
        let mut current = section_start;

        loop {
            let (addr, val) = entries[i];
            // Fill any gap since last entry with zeros
            while current < addr {
                out.push_str("0000\n");
                current += 1;
            }
            out.push_str(&format!("{:04x}\n", val));
            current += 1;
            i += 1;

            // Start a new section if the next entry is far away
            if i >= entries.len() || entries[i].0.saturating_sub(current) > GAP_THRESHOLD {
                break;
            }
        }
    }
}
