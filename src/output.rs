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

// ── Internal helpers ─────────────────────────────────────────────────────────

/// Produce full assembly with the data-init placeholder replaced by generated code.
fn asm_with_data(prog: &CompiledProgram) -> String {
    let data_asm = data_inits_to_asm(&prog.data);
    replace_placeholder(&prog.asm, &data_asm)
}

/// Produce assembly with the data-init placeholder removed (for formats with separate data).
fn asm_without_data(prog: &CompiledProgram) -> String {
    replace_placeholder(&prog.asm, "")
}

fn replace_placeholder(asm: &str, replacement: &str) -> String {
    // The placeholder may appear with or without a trailing newline
    let with_nl    = "// __DATA_INIT_HERE__\n";
    let without_nl = "// __DATA_INIT_HERE__";
    if asm.contains(with_nl) {
        asm.replace(with_nl, replacement)
    } else {
        asm.replace(without_nl, replacement)
    }
}

/// Generate Hack assembly data-initialisation code from a list of DataInit entries.
fn data_inits_to_asm(data: &[DataInit]) -> String {
    let non_zero: Vec<&DataInit> = data.iter().filter(|d| d.value != 0).collect();
    if non_zero.is_empty() { return String::new(); }
    let mut out = String::from("// Data initialization\n");
    for init in non_zero {
        let v = init.value;
        if v == 1 {
            out.push_str(&format!("D=1\n@{}\nM=D\n", init.address));
        } else if v == -1 {
            out.push_str(&format!("D=-1\n@{}\nM=D\n", init.address));
        } else if v > 0 {
            out.push_str(&format!("@{}\nD=A\n@{}\nM=D\n", v, init.address));
        } else {
            // negative: load abs value, negate D
            out.push_str(&format!("@{}\nD=-A\n@{}\nM=D\n", -(v as i32), init.address));
        }
    }
    out
}

// ── Format emitters ──────────────────────────────────────────────────────────

fn emit_asm(prog: &CompiledProgram) -> Result<EmitResult, String> {
    Ok(EmitResult { main: asm_with_data(prog), hack_companion: None })
}

fn emit_hack(prog: &CompiledProgram) -> Result<EmitResult, String> {
    let full_asm = asm_with_data(prog);
    let words = assembler::assemble(&full_asm).map_err(|e| e.to_string())?;
    let binary = words_to_binary_strings(&words);
    Ok(EmitResult { main: binary, hack_companion: None })
}

fn emit_hackem(prog: &CompiledProgram) -> Result<EmitResult, String> {
    // Assemble code WITHOUT the data bootstrap (data goes in RAM@ sections instead)
    let code_asm = asm_without_data(prog);
    let words = assembler::assemble(&code_asm).map_err(|e| e.to_string())?;

    // Locate the halt address (ROM address of the `(__end)` label)
    let halt = find_label_addr(&code_asm, "__end").unwrap_or(0);

    let mut out = format!("hackem v1.0 0x{:04x}\n", halt);
    out.push_str("ROM@0000\n");
    for w in &words {
        out.push_str(&format!("{:04x}\n", w));
    }

    // Emit RAM@ sections for all non-zero data entries
    emit_ram_sections(&prog.data, &mut out);

    Ok(EmitResult { main: out, hack_companion: None })
}

fn emit_tst(prog: &CompiledProgram) -> Result<EmitResult, String> {
    // The .hack companion: code only, data pre-loaded by the .tst script
    let code_asm = asm_without_data(prog);
    let words = assembler::assemble(&code_asm).map_err(|e| e.to_string())?;
    let binary = words_to_binary_strings(&words);

    // Build the .tst script
    let mut tst = String::new();
    tst.push_str("// Auto-generated nand2tetris test script\n");
    tst.push_str("load prog.hack,\n");
    tst.push_str("output-file prog.out,\n");
    tst.push_str("output-list RAM[0]%D1.6.1;\n\n");

    // Pre-load data via set commands
    let mut sorted: Vec<(u16, i16)> = prog.data.iter()
        .filter(|d| d.value != 0)
        .map(|d| (d.address, d.value))
        .collect();
    sorted.sort_by_key(|&(addr, _)| addr);

    if !sorted.is_empty() {
        tst.push_str("// Pre-load data into RAM\n");
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
