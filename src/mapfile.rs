/// Map-file generator: produces a human-readable report of the ROM/RAM layout
/// produced by the compiler and linker.

use std::collections::HashMap;
use crate::assembler;
use crate::codegen::DataInit;

/// Generate a map report from the final linked assembly text.
///
/// `user_source_names`: file names of the user source files (e.g. `["cal.c"]`),
/// used as a fallback label when no `// .source` markers are embedded in the asm.
/// `prog_data`: the `CompiledProgram::data` field, used to detect the font table.
pub fn generate_map(asm: &str, user_source_names: &[&str], prog_data: &[DataInit]) -> String {
    let result = match assembler::assemble_with_symbols(asm, 16) {
        Ok(r) => r,
        Err(e) => return format!("// map generation failed: {}\n", e),
    };

    let mut out = String::new();
    out.push_str("=== hack_cc Memory Map ===\n\n");

    let total_rom = result.words.len();
    out.push_str(&format!(
        "ROM: {} instructions  (0x0000..0x{:04x})\n\n",
        total_rom,
        total_rom.saturating_sub(1)
    ));

    // ── Split at the .library_section marker ─────────────────────────────────
    let (user_asm, lib_asm) = split_library(asm);

    // Library-provided symbols (from lib/ .s modules appended by the linker)
    let lib_provided: std::collections::HashSet<String> = lib_asm
        .lines()
        .filter_map(|l| l.trim().strip_prefix(".provides "))
        .flat_map(|rest| rest.split_whitespace().map(String::from))
        .collect();

    // User modules: per-source-file symbol list
    let user_modules = parse_user_modules(user_asm, user_source_names);
    let user_sym_set: std::collections::HashSet<String> = user_modules.iter()
        .flat_map(|(_, syms)| syms.iter().cloned())
        .collect();

    // ── Displayed ROM labels: only entry points of interest, deduped by addr ──
    // Build a set of "interesting" names: bootstrap + user symbols + lib symbols.
    // This avoids compiler-internal control-flow labels (e.g. __while_top_N)
    // clobbering real function entry points that share the same address.
    let bootstrap_names: std::collections::HashSet<&str> = result.rom_labels.iter()
        .filter(|(n, _)| n == "Bootstrap" || n.starts_with("__ld_") || n == "__end")
        .map(|(n, _)| n.as_str())
        .collect();
    let interesting: std::collections::HashSet<&str> = bootstrap_names.iter().copied()
        .chain(user_sym_set.iter().map(String::as_str))
        .chain(lib_provided.iter().map(String::as_str))
        .collect();

    // rom_labels is sorted by address; keep only interesting names, dedup by addr.
    let displayed: Vec<(String, u16)> = {
        let mut v: Vec<(String, u16)> = Vec::new();
        for (name, addr) in &result.rom_labels {
            if !interesting.contains(name.as_str()) { continue; }
            if v.last().map(|(_, a)| *a) == Some(*addr) { continue; }
            v.push((name.clone(), *addr));
        }
        v
    };

    // ROM lengths: addr → number of instructions until next displayed label
    let rom_lengths: HashMap<u16, u16> = {
        let mut m = HashMap::new();
        for i in 0..displayed.len() {
            let addr = displayed[i].1;
            let next = if i + 1 < displayed.len() { displayed[i + 1].1 } else { total_rom as u16 };
            m.insert(addr, next - addr);
        }
        m
    };

    // Address lookup: use all rom_labels so every user/lib symbol resolves.
    let fn_addr: HashMap<&str, u16> = result.rom_labels.iter()
        .map(|(n, a)| (n.as_str(), *a))
        .collect();

    // ── Categorize into bootstrap / user / library ────────────────────────────
    let mut bootstrap: Vec<(&str, u16)> = Vec::new();
    let mut lib_fns:   Vec<(&str, u16)> = Vec::new();

    for (name, addr) in &displayed {
        let addr = *addr;
        if name == "Bootstrap" || name.starts_with("__ld_") || name == "__end" {
            bootstrap.push((name, addr));
        } else if lib_provided.contains(name.as_str()) {
            lib_fns.push((name, addr));
        }
        // user fns rendered via user_modules below
    }

    // Inline helper: format one ROM entry line.
    // Address column always starts at position 38 (indent + name padded to 38-indent).
    macro_rules! rom_line {
        ($indent:expr, $name:expr, $addr:expr) => {{
            let name_w = 38usize.saturating_sub($indent);
            let len = rom_lengths.get(&$addr).copied().unwrap_or(0);
            format!("{}{:<width$} 0x{:04x}({:>5})  {:>5}\n",
                " ".repeat($indent), $name, $addr, $addr, len, width = name_w)
        }};
    }

    // ── Bootstrap ────────────────────────────────────────────────────────────
    if !bootstrap.is_empty() {
        out.push_str("Bootstrap:\n");
        // Startup preamble occupies ROM[0] up to the first bootstrap label.
        let first_boot = bootstrap.iter().map(|(_, a)| *a).min().unwrap_or(0);
        if first_boot > 0 {
            out.push_str(&format!("  {:<36} 0x{:04x}({:>5})  {:>5}\n",
                "(startup / call main)", 0u16, 0u16, first_boot));
        }
        for (name, addr) in &bootstrap {
            out.push_str(&rom_line!(2, *name, *addr));
        }
        out.push('\n');
    }

    // ── User code (grouped by source file) ───────────────────────────────────
    if !user_modules.is_empty() {
        out.push_str("User code:\n");
        for (src_name, syms) in &user_modules {
            out.push_str(&format!("  [{}]\n", src_name));
            let mut sym_addrs: Vec<(&str, u16)> = syms.iter()
                .filter_map(|s| fn_addr.get(s.as_str()).map(|&a| (s.as_str(), a)))
                .collect();
            sym_addrs.sort_by_key(|(_, a)| *a);
            for (name, addr) in sym_addrs {
                out.push_str(&rom_line!(4, name, addr));
            }
        }
        // Any user-sym-set symbols not covered by a module (safety net)
        let covered: std::collections::HashSet<&str> = user_modules.iter()
            .flat_map(|(_, s)| s.iter().map(String::as_str))
            .collect();
        for (name, addr) in &displayed {
            if user_sym_set.contains(name.as_str()) && !covered.contains(name.as_str()) {
                out.push_str(&rom_line!(2, name.as_str(), *addr));
            }
        }
        out.push('\n');
    }

    // ── Runtime library ───────────────────────────────────────────────────────
    if !lib_fns.is_empty() {
        out.push_str("Runtime library:\n");
        for (name, addr) in &lib_fns {
            out.push_str(&rom_line!(2, *name, *addr));
        }
        out.push('\n');
    }

    // ── RAM section ──────────────────────────────────────────────────────────
    let sp_base = extract_sp_base(asm).unwrap_or(256);
    let grouped = group_ram_vars(&result.ram_vars);

    let data_items: Vec<_> = grouped.iter().filter(|(n, _, _)|  is_data_sym(n)).collect();
    let scratch_items: Vec<_> = grouped.iter().filter(|(n, _, _)| !is_data_sym(n)).collect();

    let data_start = data_items.first().map(|(_, a, _)| *a);
    let data_end   = result.ram_vars.iter().filter(|(n, _)| is_data_sym(n)).last().map(|(_, a)| *a);
    let data_words = match (data_start, data_end) {
        (Some(s), Some(e)) => e - s + 1,
        _ => 0,
    };

    out.push_str(&format!("RAM: stack base {}  (0x{:04x})\n\n", sp_base, sp_base));

    if !data_items.is_empty() {
        out.push_str(&format!(
            "Data segment  ({}..{}  {} words):\n",
            data_start.unwrap_or(16), data_end.unwrap_or(16), data_words
        ));
        for (name, addr, count) in &data_items {
            out.push_str(&format!("  {:<36} 0x{:04x}({:>5})  {:>5}\n", name, addr, addr, count));
        }
        out.push('\n');
    }

    if !scratch_items.is_empty() {
        let scratch_start = scratch_items.first().map(|(_, a, _)| *a).unwrap_or(0);
        let scratch_end   = scratch_items.last().map(|(_, a, c)| a + *c as u16 - 1).unwrap_or(0);
        out.push_str(&format!("Runtime scratch  ({}..{}):\n", scratch_start, scratch_end));
        for (name, addr, count) in &scratch_items {
            out.push_str(&format!("  {:<36} 0x{:04x}({:>5})  {:>5}\n", name, addr, addr, count));
        }
        out.push('\n');
    }

    // ── Font table (Hackem/Tst only: lives in RAM@ sections, not assembler vars) ──
    let font_entries: Vec<u16> = prog_data.iter()
        .map(|d| d.address)
        .filter(|&a| a >= crate::codegen::FONT_BASE as u16)
        .collect();
    let has_font = !font_entries.is_empty();
    if has_font {
        let font_start = crate::codegen::FONT_BASE as u16;
        let font_total = 96u16 * 11;
        let used = font_entries.len();
        out.push_str(&format!(
            "Font table  ({}..{}  {} words):\n",
            font_start, font_start + font_total - 1, font_total
        ));
        out.push_str(&format!(
            "  {:<36} 0x{:04x}({:>5})  {:>5}  ({} non-zero entries)\n",
            "(96 chars × 11 rows bitmap)", font_start, font_start, font_total, used
        ));
        out.push('\n');
    }

    // ── Stack / Heap ──────────────────────────────────────────────────────────
    // Stack grows upward from sp_base.
    // If __alloc (malloc) is linked: heap lives at 2048..15326, pointer at 15327.
    // Otherwise: stack owns all the way up to font table or SCREEN.
    const HEAP_BASE:    u16 = 2048;
    const HEAP_PTR:     u16 = 15327;   // RAM[15327] stores the bump pointer
    const SCREEN_ADDR:  u16 = 16384;

    let has_malloc = lib_provided.contains("__alloc");

    if has_malloc {
        let stack_avail = HEAP_BASE.saturating_sub(sp_base);
        out.push_str(&format!("Stack  ({} words available):\n", stack_avail));
        out.push_str(&format!(
            "  {:<36} 0x{:04x}({:>5})\n", "base (SP)", sp_base, sp_base));
        out.push_str(&format!(
            "  {:<36} 0x{:04x}({:>5})  (heap base)\n",
            "ceiling", HEAP_BASE, HEAP_BASE));
        out.push('\n');

        let heap_ceil   = HEAP_PTR - 1;
        let heap_avail  = heap_ceil.saturating_sub(HEAP_BASE) + 1;
        out.push_str(&format!("Heap  ({} words available):\n", heap_avail));
        out.push_str(&format!(
            "  {:<36} 0x{:04x}({:>5})\n", "base", HEAP_BASE, HEAP_BASE));
        out.push_str(&format!(
            "  {:<36} 0x{:04x}({:>5})  (bump pointer)\n",
            "RAM[15327]", HEAP_PTR, HEAP_PTR));
        out.push_str(&format!(
            "  {:<36} 0x{:04x}({:>5})\n", "ceiling", heap_ceil, heap_ceil));
        out.push('\n');
    } else {
        let stack_ceil = if has_font {
            crate::codegen::FONT_BASE as u16
        } else {
            SCREEN_ADDR
        };
        let stack_avail = stack_ceil.saturating_sub(sp_base);
        let ceil_label  = if has_font { "font table" } else { "SCREEN" };
        out.push_str(&format!("Stack  ({} words available):\n", stack_avail));
        out.push_str(&format!(
            "  {:<36} 0x{:04x}({:>5})\n", "base (SP)", sp_base, sp_base));
        out.push_str(&format!(
            "  {:<36} 0x{:04x}({:>5})  ({})\n",
            "ceiling", stack_ceil, stack_ceil, ceil_label));
        out.push('\n');
    }

    out
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Split `asm` at the `// .library_section` marker line.
/// Returns `(before_marker, after_marker)`.
fn split_library(asm: &str) -> (&str, &str) {
    const MARKER: &str = "// .library_section";
    for line in asm.lines() {
        if line.trim() == MARKER {
            // Find byte offset of this line in the original string
            let line_ptr = line.as_ptr() as usize;
            let asm_ptr  = asm.as_ptr() as usize;
            let offset   = line_ptr - asm_ptr;
            let after    = offset + line.len();
            // Skip the trailing newline after the marker
            let after = if asm[after..].starts_with('\n') { after + 1 } else { after };
            return (&asm[..offset], &asm[after..]);
        }
    }
    (asm, "")
}

/// Collect user-module symbol lists from the user section of the assembly.
///
/// If `// .source <name>` markers are present, groups symbols per source file.
/// Otherwise, returns a single group named from `fallback_names`.
fn parse_user_modules(user_asm: &str, fallback_names: &[&str]) -> Vec<(String, Vec<String>)> {
    let has_source = user_asm.lines().any(|l| l.trim().starts_with("// .source "));

    if has_source {
        let mut modules: Vec<(String, Vec<String>)> = Vec::new();
        let mut current: Option<(String, Vec<String>)> = None;

        for line in user_asm.lines() {
            let line = line.trim();
            if let Some(name) = line.strip_prefix("// .source ") {
                if let Some(prev) = current.take() {
                    if !prev.1.is_empty() { modules.push(prev); }
                }
                current = Some((name.to_string(), Vec::new()));
            } else if let Some(rest) = line.strip_prefix(".provides ") {
                if let Some((_, syms)) = current.as_mut() {
                    syms.extend(rest.split_whitespace().map(String::from));
                }
            }
        }
        if let Some(prev) = current {
            if !prev.1.is_empty() { modules.push(prev); }
        }
        modules
    } else {
        let all: Vec<String> = user_asm.lines()
            .filter_map(|l| l.trim().strip_prefix(".provides "))
            .flat_map(|r| r.split_whitespace().map(String::from))
            .collect();
        if all.is_empty() { return Vec::new(); }
        let name = if fallback_names.is_empty() {
            "user code".to_string()
        } else {
            fallback_names.join(", ")
        };
        vec![(name, all)]
    }
}

// ── Remaining helpers (unchanged) ────────────────────────────────────────────

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
