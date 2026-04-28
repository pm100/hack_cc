/// Symbol-scan linker for the Hack runtime library.
///
/// After code generation produces assembly text, the linker:
/// 1. Scans the text for `@symbol` references.
/// 2. Collects all defined labels `(symbol)`.
/// 3. For each undefined reference, looks it up in the runtime library index.
/// 4. Appends the matching `.s` file's text and rescans for new undefined refs.
/// 5. Repeats until no more undefined references can be resolved.
///
/// The runtime library is embedded at compile time via `include_str!` so
/// there are no file-system accesses at link time.

use std::collections::{HashMap, HashSet};

/// A single runtime module: the symbol it provides and the assembly text.
#[derive(Debug, Clone)]
pub struct RuntimeModule {
    pub provides: String,
    pub text: &'static str,
}

/// Build the runtime library index from embedded `.s` files.
pub fn runtime_library() -> Vec<RuntimeModule> {
    vec![
        RuntimeModule { provides: "__mul".into(),          text: include_str!("runtime/math/__mul.s") },
        RuntimeModule { provides: "__div".into(),          text: include_str!("runtime/math/__div.s") },
        RuntimeModule { provides: "__strlen".into(),       text: include_str!("runtime/io/__strlen.s") },
        RuntimeModule { provides: "__strcpy".into(),       text: include_str!("runtime/io/__strcpy.s") },
        RuntimeModule { provides: "__strcmp".into(),       text: include_str!("runtime/io/__strcmp.s") },
        RuntimeModule { provides: "__strcat".into(),       text: include_str!("runtime/io/__strcat.s") },
        RuntimeModule { provides: "__itoa".into(),         text: include_str!("runtime/io/__itoa.s") },
        RuntimeModule { provides: "__draw_pixel".into(),   text: include_str!("runtime/screen/__draw_pixel.s") },
        RuntimeModule { provides: "__clear_pixel".into(),  text: include_str!("runtime/screen/__clear_pixel.s") },
        RuntimeModule { provides: "__fill_screen".into(),  text: include_str!("runtime/screen/__fill_screen.s") },
        RuntimeModule { provides: "__clear_screen".into(), text: include_str!("runtime/screen/__clear_screen.s") },
        RuntimeModule { provides: "__draw_char".into(),    text: include_str!("runtime/screen/__draw_char.s") },
        RuntimeModule { provides: "__draw_string".into(),  text: include_str!("runtime/screen/__draw_string.s") },
        RuntimeModule { provides: "__key_pressed".into(),  text: include_str!("runtime/keyboard/__key_pressed.s") },
        RuntimeModule { provides: "__alloc".into(),        text: include_str!("runtime/memory/__alloc.s") },
        RuntimeModule { provides: "__dealloc".into(),      text: include_str!("runtime/memory/__dealloc.s") },
        RuntimeModule { provides: "malloc".into(),         text: include_str!("runtime/memory/malloc.s") },
        RuntimeModule { provides: "free".into(),           text: include_str!("runtime/memory/free.s") },
        RuntimeModule { provides: "__sys_wait".into(),     text: include_str!("runtime/sys/__sys_wait.s") },
        RuntimeModule { provides: "sys_wait".into(),       text: include_str!("runtime/sys/sys_wait.s") },
        RuntimeModule { provides: "draw_line".into(),      text: include_str!("runtime/screen/draw_line.s") },
        RuntimeModule { provides: "draw_rect".into(),      text: include_str!("runtime/screen/draw_rect.s") },
        RuntimeModule { provides: "fill_rect".into(),       text: include_str!("runtime/screen/fill_rect.s") },
        RuntimeModule { provides: "clear_rect".into(),      text: include_str!("runtime/screen/clear_rect.s") },
        // IO wrappers (VM-convention) — port output
        RuntimeModule { provides: "__puts".into(),          text: include_str!("runtime/io/__puts.s") },
        RuntimeModule { provides: "putchar".into(),         text: include_str!("runtime/io/putchar.s") },
        RuntimeModule { provides: "puts".into(),            text: include_str!("runtime/io/puts.s") },
        // Screen-output alternatives: linked when putchar_screen/puts_screen are called
        RuntimeModule { provides: "__console_putchar".into(), text: include_str!("runtime/io/__console_putchar.s") },
        RuntimeModule { provides: "__puts_screen".into(),   text: include_str!("runtime/io/__puts_screen.s") },
        RuntimeModule { provides: "putchar_screen".into(),  text: include_str!("runtime/io/putchar_screen.s") },
        RuntimeModule { provides: "puts_screen".into(),     text: include_str!("runtime/io/puts_screen.s") },
        RuntimeModule { provides: "strlen".into(),          text: include_str!("runtime/io/strlen.s") },
        RuntimeModule { provides: "strcpy".into(),          text: include_str!("runtime/io/strcpy.s") },
        RuntimeModule { provides: "strcmp".into(),          text: include_str!("runtime/io/strcmp.s") },
        RuntimeModule { provides: "strcat".into(),          text: include_str!("runtime/io/strcat.s") },
        RuntimeModule { provides: "itoa".into(),            text: include_str!("runtime/io/itoa.s") },
        RuntimeModule { provides: "atoi".into(),            text: include_str!("runtime/io/atoi.s") },
        RuntimeModule { provides: "strchr".into(),          text: include_str!("runtime/io/strchr.s") },
        // Memory utilities
        RuntimeModule { provides: "memset".into(),          text: include_str!("runtime/memory/memset.s") },
        RuntimeModule { provides: "memcpy".into(),          text: include_str!("runtime/memory/memcpy.s") },
        // RNG
        RuntimeModule { provides: "rand".into(),            text: include_str!("runtime/sys/rand.s") },
        RuntimeModule { provides: "srand".into(),           text: include_str!("runtime/sys/srand.s") },
        // Misc wrappers (VM-convention)
        RuntimeModule { provides: "abs".into(),            text: include_str!("runtime/misc/abs.s") },
        RuntimeModule { provides: "min".into(),            text: include_str!("runtime/misc/min.s") },
        RuntimeModule { provides: "max".into(),            text: include_str!("runtime/misc/max.s") },
        // Keyboard wrappers (VM-convention)
        RuntimeModule { provides: "read_key".into(),       text: include_str!("runtime/keyboard/read_key.s") },
        RuntimeModule { provides: "getchar".into(),        text: include_str!("runtime/keyboard/getchar.s") },
        // Screen VM-convention wrappers
        RuntimeModule { provides: "draw_pixel".into(),     text: include_str!("runtime/screen/draw_pixel.s") },
        RuntimeModule { provides: "clear_pixel".into(),    text: include_str!("runtime/screen/clear_pixel.s") },
        RuntimeModule { provides: "fill_screen".into(),    text: include_str!("runtime/screen/fill_screen.s") },
        RuntimeModule { provides: "clear_screen".into(),   text: include_str!("runtime/screen/clear_screen.s") },
        RuntimeModule { provides: "draw_char".into(),      text: include_str!("runtime/screen/draw_char.s") },
        RuntimeModule { provides: "draw_string".into(),    text: include_str!("runtime/screen/draw_string.s") },
        RuntimeModule { provides: "print_at".into(),       text: include_str!("runtime/screen/print_at.s") },
    ]
}

/// Link `user_asm` with runtime modules as needed.
///
/// Returns the final combined assembly text with only the required runtime
/// modules appended, in dependency order (determined automatically by
/// repeated symbol scanning).
pub fn link(user_asm: &str) -> String {
    let library = runtime_library();
    let index: HashMap<&str, &RuntimeModule> =
        library.iter().map(|m| (m.provides.as_str(), m)).collect();

    let mut combined = user_asm.to_string();
    let mut included: HashSet<String> = HashSet::new();

    loop {
        let defined = collect_defined(&combined);
        let referenced = collect_referenced(&combined);
        let mut added_any = false;

        for sym in &referenced {
            if defined.contains(sym) || included.contains(sym) {
                continue;
            }
            if let Some(module) = index.get(sym.as_str()) {
                combined.push('\n');
                combined.push_str(module.text);
                included.insert(sym.clone());
                added_any = true;
            }
        }

        if !added_any {
            break;
        }
    }

    combined
}

/// Collect all defined labels `(symbol)` from assembly text.
fn collect_defined(asm: &str) -> HashSet<String> {
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
fn collect_referenced(asm: &str) -> HashSet<String> {
    let mut refs = HashSet::new();
    for line in asm.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix('@') {
            // Skip numeric literals and lines with comments/spaces
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
        let out = link(asm);
        assert!(out.contains("(__mul)"));
        // __mul doesn't need __div so __div should not appear
        assert!(!out.contains("(__div)"));
    }

    #[test]
    fn test_link_transitive_itoa_needs_div() {
        let asm = "@__itoa\n0;JMP\n";
        let out = link(asm);
        assert!(out.contains("(__itoa)"));
        assert!(out.contains("(__div)"), "itoa calls __div so it should be linked in");
    }

    #[test]
    fn test_link_draw_string_needs_draw_char() {
        let asm = "@__draw_string\n0;JMP\n";
        let out = link(asm);
        assert!(out.contains("(__draw_string)"));
        assert!(out.contains("(__draw_char)"));
    }

    #[test]
    fn test_link_no_unused_runtime() {
        let asm = "D=M\n@R13\nM=D\n";
        let out = link(asm);
        // No runtime symbols referenced, nothing should be added
        assert!(!out.contains("(__mul)"));
        assert!(!out.contains("(__div)"));
    }
}
