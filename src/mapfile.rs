/// Map-file generator: produces a human-readable report of the ROM/RAM layout
/// produced by the compiler and linker.

use std::collections::HashSet;
use crate::assembler;

/// Generate a map report from the final linked assembly text.
/// Returns the formatted report as a string.
pub fn generate_map(asm: &str) -> String {
    let result = match assembler::assemble_with_symbols(asm, 16) {
        Ok(r) => r,
        Err(e) => return format!("// map generation failed: {}\n", e),
    };

    let mut out = String::new();
    out.push_str("=== hack_cc Memory Map ===\n\n");

    // ── ROM section ──────────────────────────────────────────────────────────
    let total_rom = result.words.len();
    out.push_str(&format!(
        "ROM: {} instructions  (0x0000 – 0x{:04x})\n\n",
        total_rom,
        total_rom.saturating_sub(1)
    ));

    // Symbols provided by library modules (from .provides directives in linked asm)
    let provided: HashSet<String> = asm
        .lines()
        .filter_map(|l| l.trim().strip_prefix(".provides "))
        .flat_map(|rest| rest.split_whitespace().map(String::from))
        .collect();

    // Categorise ROM labels
    let mut bootstrap:  Vec<(&str, u16)> = Vec::new();
    let mut user_fns:   Vec<(&str, u16)> = Vec::new();
    let mut lib_fns:    Vec<(&str, u16)> = Vec::new();
    let mut internals:  Vec<(&str, u16)> = Vec::new();

    for (name, addr) in &result.rom_labels {
        let addr = *addr;
        if name == "Bootstrap" || name.starts_with("__ld_") || name == "__end" {
            bootstrap.push((name, addr));
        } else if provided.contains(name.as_str()) {
            // Public symbol exported by a library module
            lib_fns.push((name, addr));
        } else if !name.starts_with("__")
               && !name.starts_with("L_")
               && !name.contains('$') {
            // Looks like a user-defined function entry point
            user_fns.push((name, addr));
        } else {
            internals.push((name, addr));
        }
    }

    let emit_section = |out: &mut String, header: &str, entries: &[(&str, u16)]| {
        if entries.is_empty() { return; }
        out.push_str(&format!("  [{}]\n", header));
        for (name, addr) in entries {
            out.push_str(&format!("    {:<36} ROM[{:5}]  0x{:04x}\n", name, addr, addr));
        }
        out.push('\n');
    };

    emit_section(&mut out, "Bootstrap",        &bootstrap);
    emit_section(&mut out, "User functions",   &user_fns);
    emit_section(&mut out, "Library",          &lib_fns);
    // Internal jump labels ($if_*, $while_*, __vm_*) are omitted – too numerous.

    // ── RAM section ──────────────────────────────────────────────────────────
    let sp_base = extract_sp_base(asm).unwrap_or(256);

    // Group consecutive elements of the same array/string into one entry
    let grouped = group_ram_vars(&result.ram_vars);

    // Partition into data-segment items and runtime scratch
    let data_items: Vec<_> = grouped.iter()
        .filter(|(name, _, _)| is_data_sym(name))
        .collect();
    let scratch_items: Vec<_> = grouped.iter()
        .filter(|(name, _, _)| !is_data_sym(name))
        .collect();

    let data_start = data_items.first().map(|(_, a, _)| *a);
    let data_end   = result.ram_vars.iter()
        .filter(|(n, _)| is_data_sym(n))
        .last()
        .map(|(_, a)| *a);
    let data_words = match (data_start, data_end) {
        (Some(s), Some(e)) => e - s + 1,
        _ => 0,
    };

    out.push_str(&format!(
        "RAM: stack base RAM[{}]  (0x{:04x})\n\n",
        sp_base, sp_base
    ));

    if !data_items.is_empty() {
        out.push_str(&format!(
            "  [Data segment]  RAM[{}..{}]  ({} words)\n",
            data_start.unwrap_or(16),
            data_end.unwrap_or(16),
            data_words
        ));
        for (name, addr, count) in &data_items {
            if *count == 1 {
                out.push_str(&format!("    {:<36} RAM[{:5}]  0x{:04x}\n", name, addr, addr));
            } else {
                out.push_str(&format!(
                    "    {:<36} RAM[{:5}]  0x{:04x}  ({} words)\n",
                    name, addr, addr, count
                ));
            }
        }
        out.push('\n');
    }

    if !scratch_items.is_empty() {
        let scratch_start = scratch_items.first().map(|(_, a, _)| *a).unwrap_or(0);
        let scratch_end   = scratch_items.last().map(|(_, a, c)| a + *c as u16 - 1).unwrap_or(0);
        out.push_str(&format!(
            "  [Runtime scratch]  RAM[{}..{}]\n",
            scratch_start, scratch_end
        ));
        for (name, addr, count) in &scratch_items {
            if *count == 1 {
                out.push_str(&format!("    {:<36} RAM[{:5}]  0x{:04x}\n", name, addr, addr));
            } else {
                out.push_str(&format!(
                    "    {:<36} RAM[{:5}]  0x{:04x}  ({} words)\n",
                    name, addr, addr, count
                ));
            }
        }
        out.push('\n');
    }

    out.push_str(&format!("  Stack base:  RAM[{}]  0x{:04x}\n\n", sp_base, sp_base));

    // ── Linked library modules ───────────────────────────────────────────────
    if !provided.is_empty() {
        let mut mods: Vec<&str> = provided.iter().map(String::as_str).collect();
        mods.sort_unstable();
        out.push_str(&format!("Linked library symbols ({}):\n", mods.len()));
        // Wrap at ~80 chars
        let mut line = String::from("  ");
        for (i, sym) in mods.iter().enumerate() {
            let sep = if i + 1 < mods.len() { ", " } else { "" };
            let piece = format!("{}{}", sym, sep);
            if line.len() + piece.len() > 80 {
                out.push_str(&line);
                out.push('\n');
                line = String::from("  ");
            }
            line.push_str(&piece);
        }
        if line.trim().len() > 0 {
            out.push_str(&line);
            out.push('\n');
        }
    }

    out
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Detect the SP-base value from the bootstrap preamble (`@N D=A @SP M=D`).
fn extract_sp_base(asm: &str) -> Option<u16> {
    let mut lines = asm.lines().peekable();
    while let Some(line) = lines.next() {
        let line = line.trim();
        if line.starts_with("// Bootstrap") {
            // The next non-empty, non-comment line should be `@N`
            for next in lines.by_ref() {
                let next = next.trim();
                if next.is_empty() || next.starts_with("//") { continue; }
                if let Some(n_str) = next.strip_prefix('@') {
                    if let Ok(n) = n_str.parse::<u16>() {
                        return Some(n);
                    }
                }
                break;
            }
        }
    }
    None
}

/// True if a RAM variable name belongs to the static data segment
/// (string literals or C globals), false if it's a runtime scratch variable.
fn is_data_sym(name: &str) -> bool {
    name.starts_with("__str_") || name.starts_with("__g_")
}

/// Strip a trailing `_N` suffix (N all digits) to get the "base" name
/// of a multi-word string literal or array element, but only when the remaining
/// prefix itself still contains `_` (so `__str_1_0` → `__str_1`, but
/// `__str_1` stays as `__str_1`).
fn base_name(name: &str) -> &str {
    if let Some(idx) = name.rfind('_') {
        let suffix = &name[idx + 1..];
        if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
            let prefix = &name[..idx];
            // Only strip if the prefix after the leading `__` still contains `_`
            // (meaning there's a meaningful nested name, not just `__str`).
            if prefix.len() > 2 && prefix[2..].contains('_') {
                return prefix;
            }
        }
    }
    name
}

/// Group consecutive RAM vars that belong to the same array/string into
/// `(base_name, base_addr, count)` tuples.
fn group_ram_vars(vars: &[(String, u16)]) -> Vec<(String, u16, usize)> {
    let mut groups: Vec<(String, u16, usize)> = Vec::new();
    for (name, addr) in vars {
        let base = base_name(name);
        if let Some(last) = groups.last_mut() {
            if last.0 == base {
                last.2 += 1;
                continue;
            }
        }
        groups.push((base.to_string(), *addr, 1));
    }
    groups
}
