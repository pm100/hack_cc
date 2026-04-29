/// Symbol-scan linker for the Hack runtime library.
///
/// After code generation produces assembly text, the linker:
/// 1. Scans the text for `@symbol` references.
/// 2. Collects all defined labels `(symbol)`.
/// 3. For each undefined reference, looks it up in the library index (built by
///    scanning `// PROVIDES:` comments in `.s` files on disk).
/// 4. Appends the matching `.s` file text and rescans for new undefined refs.
/// 5. Repeats until no more undefined references can be resolved.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Default library directories.
/// Precedence: `HACK_LIB` env var > `./lib/` relative to cwd > `<exe_dir>/lib/`.
pub fn default_lib_dirs() -> Vec<PathBuf> {
    if let Ok(p) = std::env::var("HACK_LIB") {
        let pb = PathBuf::from(p);
        if pb.exists() { return vec![pb]; }
    }
    let cwd_lib = PathBuf::from("lib");
    if cwd_lib.exists() { return vec![cwd_lib]; }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let exe_lib = exe_dir.join("lib");
            if exe_lib.exists() { return vec![exe_lib]; }
        }
    }
    vec![]
}

/// Build a symbol -> file-content index by scanning `.s` files in `lib_dirs`.
/// Each `.s` file must have a `// PROVIDES: sym1 sym2 ...` comment on its first line.
fn build_lib_index(lib_dirs: &[PathBuf]) -> HashMap<String, Arc<String>> {
    let mut index = HashMap::new();
    for dir in lib_dirs {
        scan_dir(dir, &mut index);
    }
    index
}

fn scan_dir(dir: &Path, index: &mut HashMap<String, Arc<String>>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_dir(&path, index);
        } else if path.extension().and_then(|e| e.to_str()) == Some("s") {
            let Ok(content) = std::fs::read_to_string(&path) else { continue };
            let first_line = content.lines().next().unwrap_or("").to_string();
            if let Some(rest) = first_line.strip_prefix(".provides ") {
                let syms: Vec<String> = rest.split_whitespace().map(|s| s.to_string()).collect();
                if !syms.is_empty() {
                    let shared = Arc::new(content);
                    for sym in syms {
                        index.entry(sym).or_insert_with(|| Arc::clone(&shared));
                    }
                }
            }
        }
    }
}

/// Link `user_asm` with library modules from `lib_dirs`.
///
/// Returns the final combined assembly text with only the required library
/// modules appended, in dependency order (determined by repeated symbol scanning).
pub fn link(user_asm: &str, lib_dirs: &[PathBuf]) -> String {
    let index = build_lib_index(lib_dirs);

    let mut combined = user_asm.to_string();
    let mut included: HashSet<String> = HashSet::new();

    loop {
        let defined = collect_defined(&combined);
        let referenced = collect_referenced(&combined);
        let mut to_append: Vec<Arc<String>> = Vec::new();

        for sym in &referenced {
            if defined.contains(sym) || included.contains(sym) {
                continue;
            }
            if let Some(content) = index.get(sym.as_str()) {
                included.insert(sym.clone());
                to_append.push(Arc::clone(content));
            }
        }

        if to_append.is_empty() {
            break;
        }
        for text in to_append {
            combined.push('\n');
            combined.push_str(&text);
        }
    }

    combined
}

/// Collect all defined labels `(symbol)` from assembly text.
pub fn collect_defined(asm: &str) -> HashSet<String> {
    let mut defined = HashSet::new();
    for line in asm.lines() {
        let line = line.trim();
        if line.starts_with('(') && line.ends_with(')') {
            defined.insert(line[1..line.len() - 1].to_string());
        }
    }
    defined
}

/// Collect all referenced symbols `@symbol` (non-numeric) from assembly text.
pub fn collect_referenced(asm: &str) -> HashSet<String> {
    let mut refs = HashSet::new();
    for line in asm.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix('@') {
            let sym = rest.split_whitespace().next().unwrap_or("");
            if !sym.is_empty() && !sym.chars().next().unwrap_or('x').is_ascii_digit() {
                refs.insert(sym.to_string());
            }
        }
    }
    refs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_lib_dir() -> Vec<PathBuf> {
        vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib")]
    }

    #[test]
    fn test_collect_defined() {
        let asm = "(__mul)\n@R13\nM=0\n(foo)\n";
        let d = collect_defined(asm);
        assert!(d.contains("__mul"));
        assert!(d.contains("foo"));
        assert!(!d.contains("R13"));
    }

    #[test]
    fn test_collect_referenced() {
        let asm = "@R13\n@__mul\n@42\n@foo\n";
        let r = collect_referenced(asm);
        assert!(r.contains("R13"));
        assert!(r.contains("__mul"));
        assert!(r.contains("foo"));
        assert!(!r.contains("42"));
    }

    #[test]
    fn test_link_pulls_mul() {
        let asm = "@__mul\n0;JMP\n";
        let out = link(asm, &test_lib_dir());
        assert!(out.contains("(__mul)"));
        // __mul doesn't need __div so __div should not appear
        assert!(!out.contains("(__div)"));
    }

    #[test]
    fn test_link_transitive_itoa_needs_div() {
        let asm = "@__itoa\n0;JMP\n";
        let out = link(asm, &test_lib_dir());
        assert!(out.contains("(__itoa)"));
        assert!(out.contains("(__div)"), "itoa calls __div so it should be linked in");
    }

    #[test]
    fn test_link_draw_string_needs_draw_char() {
        let asm = "@__draw_string\n0;JMP\n";
        let out = link(asm, &test_lib_dir());
        assert!(out.contains("(__draw_string)"));
        assert!(out.contains("(__draw_char)"));
    }

    #[test]
    fn test_link_no_unused_runtime() {
        let asm = "D=M\n@R13\nM=D\n";
        let out = link(asm, &test_lib_dir());
        // No runtime symbols referenced, nothing should be added
        assert!(!out.contains("(__mul)"));
        assert!(!out.contains("(__div)"));
    }
}
