/// C preprocessor: text-level macro expansion and conditional inclusion.
///
/// Runs before the lexer. Supports:
///   #define NAME value           — object-like macro
///   #define NAME(a, b) body      — function-like macro
///   #undef NAME
///   #ifdef NAME / #ifndef NAME / #else / #endif
///   #if expr / #elif expr        — integer constant expressions (basic arithmetic & defined())
///   #include "path"              — relative file inclusion

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Built-in virtual header <hack.h> — declarations for all Hack platform builtins.
const HACK_H_SOURCE: &str = r#"
#ifndef __HACK_H__
#define __HACK_H__

/* --- basic I/O --- */
int putchar(int c);
int puts(char *s);
int read_key(void);

/* --- screen (pixels) --- */
void draw_pixel(int x, int y);
void clear_pixel(int x, int y);
void fill_screen(void);
void clear_screen(void);

/* --- text rendering --- */
void draw_char(char *str_ptr, int col, int row);
void draw_string(int col, int row, char *s);
void print_at(int col, int row, char *s);

/* --- math --- */
int abs(int x);
int min(int a, int b);
int max(int a, int b);

/* --- string functions --- */
char *strcpy(char *dst, char *src);
int   strcmp(char *a, char *b);
char *strcat(char *dst, char *src);
int   strlen(char *s);
char *itoa(int n, char *buf);

/* --- graphics helpers --- */
void draw_line(int x1, int y1, int x2, int y2);
void draw_rect(int x, int y, int w, int h);
void fill_rect(int x, int y, int w, int h);

/* --- memory --- */
void *malloc(int n);
void free(void *ptr);

/* --- system --- */
void sys_wait(int ms);

#endif /* __HACK_H__ */
"#;

#[derive(Debug, Error, Clone)]
#[error("preprocessor error at {file}:{line}: {msg}")]
pub struct PreprocError {
    pub file: String,
    pub line: u32,
    pub msg: String,
}

impl PreprocError {
    fn new(file: &str, line: u32, msg: impl Into<String>) -> Self {
        Self { file: file.to_string(), line, msg: msg.into() }
    }
}

/// A macro definition: either object-like (`params = None`) or function-like.
#[derive(Debug, Clone)]
struct MacroDef {
    params: Option<Vec<String>>,
    body: String,
}

/// Preprocess `source`, resolving `#include "..."` relative to `base_dir`.
/// If `base_dir` is `None`, `#include` directives will error if attempted.
pub fn preprocess(source: &str, base_dir: Option<&Path>) -> Result<String, PreprocError> {
    let file_name = base_dir
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "<input>".to_string());
    let mut ctx = PreprocCtx::new();
    ctx.expand_source(source, &file_name, base_dir, 0)
}

struct PreprocCtx {
    macros: HashMap<String, MacroDef>,
}

impl PreprocCtx {
    fn new() -> Self {
        Self { macros: HashMap::new() }
    }

    fn expand_source(
        &mut self,
        source: &str,
        file: &str,
        base_dir: Option<&Path>,
        depth: u32,
    ) -> Result<String, PreprocError> {
        if depth > 64 {
            return Err(PreprocError::new(file, 0, "#include recursion limit exceeded"));
        }

        let mut output = String::new();
        // Stack of (active, else_seen) for #ifdef/#if nesting.
        // `active` = whether the current branch is being emitted.
        let mut cond_stack: Vec<(bool, bool)> = Vec::new();

        for (line_idx, line) in source.lines().enumerate() {
            let line_no = (line_idx + 1) as u32;
            let trimmed = line.trim_start();

            if let Some(rest) = trimmed.strip_prefix('#') {
                let rest = rest.trim_start();
                let (directive, args) = split_directive(rest);

                match directive {
                    "define" => {
                        if !self.is_active(&cond_stack) {
                            output.push('\n');
                            continue;
                        }
                        self.parse_define(args, file, line_no)?;
                    }
                    "undef" => {
                        if !self.is_active(&cond_stack) {
                            output.push('\n');
                            continue;
                        }
                        let name = args.trim();
                        self.macros.remove(name);
                    }
                    "ifdef" => {
                        let name = args.trim();
                        let active = self.is_active(&cond_stack) && self.macros.contains_key(name);
                        cond_stack.push((active, false));
                    }
                    "ifndef" => {
                        let name = args.trim();
                        let active = self.is_active(&cond_stack) && !self.macros.contains_key(name);
                        cond_stack.push((active, false));
                    }
                    "if" => {
                        let active = if self.is_active(&cond_stack) {
                            self.eval_if_expr(args.trim(), file, line_no)? != 0
                        } else {
                            false
                        };
                        cond_stack.push((active, false));
                    }
                    "elif" => {
                        let len = cond_stack.len();
                        if len == 0 {
                            return Err(PreprocError::new(file, line_no, "#elif without #if"));
                        }
                        if cond_stack[len - 1].1 {
                            return Err(PreprocError::new(file, line_no, "#elif after #else"));
                        }
                        let was_active = cond_stack[len - 1].0;
                        let parent_active = len < 2 || cond_stack[len - 2].0;
                        cond_stack[len - 1].0 = if !was_active && parent_active {
                            self.eval_if_expr(args.trim(), file, line_no)? != 0
                        } else {
                            false
                        };
                    }
                    "else" => {
                        let len = cond_stack.len();
                        if len == 0 {
                            return Err(PreprocError::new(file, line_no, "#else without #if"));
                        }
                        if cond_stack[len - 1].1 {
                            return Err(PreprocError::new(file, line_no, "duplicate #else"));
                        }
                        let was_active = cond_stack[len - 1].0;
                        let parent_active = len < 2 || cond_stack[len - 2].0;
                        cond_stack[len - 1].0 = !was_active && parent_active;
                        cond_stack[len - 1].1 = true;
                    }
                    "endif" => {
                        if cond_stack.pop().is_none() {
                            return Err(PreprocError::new(file, line_no, "#endif without #if"));
                        }
                    }
                    "include" => {
                        if !self.is_active(&cond_stack) {
                            output.push('\n');
                            continue;
                        }
                        let path_str = parse_include_path(args, file, line_no)?;
                        // Virtual built-in headers
                        if path_str == "__builtin__/hack.h" {
                            let expanded = self.expand_source(
                                HACK_H_SOURCE,
                                "<hack.h>",
                                None,
                                depth + 1,
                            )?;
                            output.push_str(&expanded);
                            continue;
                        }
                        let inc_path = resolve_include(&path_str, base_dir, file, line_no)?;
                        let content = std::fs::read_to_string(&inc_path).map_err(|e| {
                            PreprocError::new(file, line_no, format!("cannot read {:?}: {}", inc_path, e))
                        })?;
                        let inc_base = inc_path.parent().map(Path::to_path_buf);
                        let inc_name = inc_path.to_string_lossy().into_owned();
                        let expanded = self.expand_source(
                            &content,
                            &inc_name,
                            inc_base.as_deref(),
                            depth + 1,
                        )?;
                        output.push_str(&expanded);
                    }
                    _ => {
                        // Unknown directive: emit as blank line if active, skip otherwise
                        if self.is_active(&cond_stack) {
                            return Err(PreprocError::new(
                                file, line_no,
                                format!("unknown preprocessor directive '#{}'", directive),
                            ));
                        }
                    }
                }
                output.push('\n');
                continue;
            }

            // Non-directive line
            if self.is_active(&cond_stack) {
                let expanded = self.expand_line(line, file, line_no)?;
                output.push_str(&expanded);
            }
            output.push('\n');
        }

        if !cond_stack.is_empty() {
            return Err(PreprocError::new(file, 0, "unterminated #if/#ifdef block"));
        }

        Ok(output)
    }

    fn is_active(&self, stack: &[(bool, bool)]) -> bool {
        stack.iter().all(|(active, _)| *active)
    }

    fn parse_define(&mut self, args: &str, file: &str, line: u32) -> Result<(), PreprocError> {
        let args = args.trim();
        // Find end of macro name
        let name_end = args.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(args.len());
        if name_end == 0 {
            return Err(PreprocError::new(file, line, "#define requires a name"));
        }
        let name = &args[..name_end];
        let rest = &args[name_end..];

        // Function-like macro: NAME(params) body — '(' must immediately follow name
        if rest.starts_with('(') {
            let close = rest.find(')').ok_or_else(|| {
                PreprocError::new(file, line, "missing ')' in #define parameter list")
            })?;
            let param_str = &rest[1..close];
            let params: Vec<String> = if param_str.trim().is_empty() {
                vec![]
            } else {
                param_str.split(',').map(|p| p.trim().to_string()).collect()
            };
            let body = rest[close + 1..].trim().to_string();
            self.macros.insert(name.to_string(), MacroDef { params: Some(params), body });
        } else {
            // Object-like: NAME body
            let body = rest.trim_start().to_string();
            self.macros.insert(name.to_string(), MacroDef { params: None, body });
        }
        Ok(())
    }

    /// Expand macros in a line of non-directive C source text.
    fn expand_line(&self, line: &str, _file: &str, _line_no: u32) -> Result<String, PreprocError> {
        self.expand_text(line)
    }

    /// Expand macros in arbitrary text (single pass).
    fn expand_text(&self, text: &str) -> Result<String, PreprocError> {
        let mut out = String::new();
        let bytes = text.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            // Skip string literals without expanding inside them
            if bytes[i] == b'"' {
                out.push('"');
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' && i + 1 < bytes.len() {
                        out.push(bytes[i] as char);
                        out.push(bytes[i + 1] as char);
                        i += 2;
                    } else if bytes[i] == b'"' {
                        out.push('"');
                        i += 1;
                        break;
                    } else {
                        out.push(bytes[i] as char);
                        i += 1;
                    }
                }
                continue;
            }
            // Skip char literals
            if bytes[i] == b'\'' {
                out.push('\'');
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' && i + 1 < bytes.len() {
                        out.push(bytes[i] as char);
                        out.push(bytes[i + 1] as char);
                        i += 2;
                    } else if bytes[i] == b'\'' {
                        out.push('\'');
                        i += 1;
                        break;
                    } else {
                        out.push(bytes[i] as char);
                        i += 1;
                    }
                }
                continue;
            }
            // Skip line comments
            if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                out.push_str(&text[i..]);
                break;
            }
            // Identifier?
            if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let ident = &text[start..i];
                if let Some(mac) = self.macros.get(ident) {
                    if let Some(params) = &mac.params {
                        // Function-like: consume argument list
                        let rest = &text[i..];
                        if let Some(args_and_rest) = consume_args(rest) {
                            let (arg_texts, after) = args_and_rest;
                            if arg_texts.len() != params.len() {
                                // Argument count mismatch — emit as-is
                                out.push_str(ident);
                            } else {
                                let expanded = expand_func_macro(&mac.body, params, &arg_texts);
                                let re_expanded = self.expand_text(&expanded)?;
                                out.push_str(&re_expanded);
                                i = text.len() - after.len();
                            }
                        } else {
                            out.push_str(ident);
                        }
                    } else {
                        // Object-like
                        let re_expanded = self.expand_text(&mac.body)?;
                        out.push_str(&re_expanded);
                    }
                } else {
                    out.push_str(ident);
                }
                continue;
            }
            out.push(bytes[i] as char);
            i += 1;
        }
        Ok(out)
    }

    /// Evaluate a simple integer constant expression for #if / #elif.
    /// Supports: integer literals, `defined(NAME)`, `!`, `&&`, `||`, `==`, `!=`,
    /// `<`, `<=`, `>`, `>=`, `+`, `-`, `*`, `/`, `%`, parentheses.
    fn eval_if_expr(&self, expr: &str, file: &str, line: u32) -> Result<i64, PreprocError> {
        // First expand macros in the expression (except `defined(...)`)
        let expanded = self.expand_if_expr(expr);
        parse_const_expr(&expanded).map_err(|e| PreprocError::new(file, line, e))
    }

    /// Expand macros in an #if expression, but leave `defined(NAME)` intact.
    fn expand_if_expr(&self, expr: &str) -> String {
        // Replace `defined(NAME)` with 1 or 0, then expand other macros.
        let mut out = String::new();
        let mut rest = expr;
        while let Some(pos) = rest.find("defined") {
            out.push_str(&rest[..pos]);
            rest = &rest[pos + 7..].trim_start();
            if rest.starts_with('(') {
                if let Some(close) = rest.find(')') {
                    let name = rest[1..close].trim();
                    let val = if self.macros.contains_key(name) { "1" } else { "0" };
                    out.push_str(val);
                    rest = &rest[close + 1..];
                    continue;
                }
            }
            out.push_str("defined");
        }
        out.push_str(rest);
        // Now expand remaining macros (simple object-like only)
        self.expand_text(&out).unwrap_or(out)
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn split_directive(s: &str) -> (&str, &str) {
    let s = s.trim_start();
    let end = s.find(|c: char| c.is_whitespace()).unwrap_or(s.len());
    (&s[..end], &s[end..])
}

fn parse_include_path(args: &str, file: &str, line: u32) -> Result<String, PreprocError> {
    let args = args.trim();
    if args.starts_with('"') && args.ends_with('"') && args.len() >= 2 {
        Ok(args[1..args.len() - 1].to_string())
    } else if args.starts_with('<') && args.ends_with('>') {
        // Allow <hack.h> as a virtual system header; others still unsupported
        let name = &args[1..args.len() - 1];
        if name == "hack.h" {
            Ok("__builtin__/hack.h".to_string())
        } else {
            Err(PreprocError::new(file, line, "#include <system> headers are not supported"))
        }
    } else {
        Err(PreprocError::new(file, line, "malformed #include path"))
    }
}

fn resolve_include(
    path: &str,
    base_dir: Option<&Path>,
    file: &str,
    line: u32,
) -> Result<PathBuf, PreprocError> {
    let base = base_dir.ok_or_else(|| {
        PreprocError::new(file, line, "#include not supported without a base directory")
    })?;
    Ok(base.join(path))
}

/// Consume a function-macro argument list starting with '('.
/// Returns `Some((args, remaining_text))` or `None` if no `(` follows immediately.
fn consume_args(text: &str) -> Option<(Vec<String>, &str)> {
    let text = text.trim_start();
    if !text.starts_with('(') {
        return None;
    }
    let text = &text[1..];
    let mut depth = 1usize;
    let mut args: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut chars = text.char_indices();
    let mut end = text.len();
    for (i, c) in &mut chars {
        match c {
            '(' => { depth += 1; current.push(c); }
            ')' => {
                depth -= 1;
                if depth == 0 {
                    args.push(current.trim().to_string());
                    end = i + 1;
                    break;
                }
                current.push(c);
            }
            ',' if depth == 1 => {
                args.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(c),
        }
    }
    if depth != 0 { return None; }
    Some((args, &text[end..]))
}

/// Expand a function-like macro body, substituting parameters with argument texts.
fn expand_func_macro(body: &str, params: &[String], args: &[String]) -> String {
    let mut out = body.to_string();
    // Replace each param name with the corresponding arg (longest-first to avoid partial matches)
    let mut pairs: Vec<(&str, &str)> = params.iter().zip(args.iter())
        .map(|(p, a)| (p.as_str(), a.as_str()))
        .collect();
    pairs.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    for (param, arg) in pairs {
        // Replace whole-word occurrences only
        let mut replaced = String::new();
        let mut rest = out.as_str();
        while let Some(pos) = rest.find(param) {
            let before = pos.checked_sub(1)
                .and_then(|i| rest.as_bytes().get(i))
                .map(|&b| b.is_ascii_alphanumeric() || b == b'_')
                .unwrap_or(false);
            let after = rest.as_bytes().get(pos + param.len())
                .map(|&b| b.is_ascii_alphanumeric() || b == b'_')
                .unwrap_or(false);
            replaced.push_str(&rest[..pos]);
            if before || after {
                replaced.push_str(param);
            } else {
                replaced.push_str(arg);
            }
            rest = &rest[pos + param.len()..];
        }
        replaced.push_str(rest);
        out = replaced;
    }
    out
}

// ── Constant expression evaluator for #if ────────────────────────────────────

fn parse_const_expr(expr: &str) -> Result<i64, String> {
    let tokens = tokenize_expr(expr)?;
    let mut pos = 0;
    let val = parse_or(&tokens, &mut pos)?;
    Ok(val)
}

fn tokenize_expr(s: &str) -> Result<Vec<ExprToken>, String> {
    let mut tokens = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() { i += 1; continue; }
        match bytes[i] {
            b'0'..=b'9' => {
                let start = i;
                if bytes[i] == b'0' && i + 1 < bytes.len() && (bytes[i+1] == b'x' || bytes[i+1] == b'X') {
                    i += 2;
                    while i < bytes.len() && bytes[i].is_ascii_hexdigit() { i += 1; }
                    let n = i64::from_str_radix(&s[start+2..i], 16)
                        .map_err(|_| "invalid hex literal".to_string())?;
                    tokens.push(ExprToken::Num(n));
                } else {
                    while i < bytes.len() && bytes[i].is_ascii_digit() { i += 1; }
                    let n: i64 = s[start..i].parse().map_err(|_| "invalid literal".to_string())?;
                    tokens.push(ExprToken::Num(n));
                }
            }
            b'(' => { tokens.push(ExprToken::LParen); i += 1; }
            b')' => { tokens.push(ExprToken::RParen); i += 1; }
            b'!' if bytes.get(i+1) != Some(&b'=') => { tokens.push(ExprToken::Not); i += 1; }
            b'~' => { tokens.push(ExprToken::BitNot); i += 1; }
            b'+' => { tokens.push(ExprToken::Plus); i += 1; }
            b'-' => { tokens.push(ExprToken::Minus); i += 1; }
            b'*' => { tokens.push(ExprToken::Mul); i += 1; }
            b'/' => { tokens.push(ExprToken::Div); i += 1; }
            b'%' => { tokens.push(ExprToken::Mod); i += 1; }
            b'&' if bytes.get(i+1) == Some(&b'&') => { tokens.push(ExprToken::And); i += 2; }
            b'|' if bytes.get(i+1) == Some(&b'|') => { tokens.push(ExprToken::Or); i += 2; }
            b'&' => { tokens.push(ExprToken::BitAnd); i += 1; }
            b'|' => { tokens.push(ExprToken::BitOr); i += 1; }
            b'^' => { tokens.push(ExprToken::BitXor); i += 1; }
            b'=' if bytes.get(i+1) == Some(&b'=') => { tokens.push(ExprToken::Eq); i += 2; }
            b'!' if bytes.get(i+1) == Some(&b'=') => { tokens.push(ExprToken::Ne); i += 2; }
            b'<' if bytes.get(i+1) == Some(&b'=') => { tokens.push(ExprToken::Le); i += 2; }
            b'>' if bytes.get(i+1) == Some(&b'=') => { tokens.push(ExprToken::Ge); i += 2; }
            b'<' => { tokens.push(ExprToken::Lt); i += 1; }
            b'>' => { tokens.push(ExprToken::Gt); i += 1; }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                // Undefined macro name — treat as 0
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                tokens.push(ExprToken::Num(0));
            }
            c => return Err(format!("unexpected character '{}' in #if expression", c as char)),
        }
    }
    tokens.push(ExprToken::Eof);
    Ok(tokens)
}

#[derive(Debug, Clone, PartialEq)]
enum ExprToken { Num(i64), LParen, RParen, Not, BitNot, Plus, Minus, Mul, Div, Mod,
    And, Or, BitAnd, BitOr, BitXor, Eq, Ne, Lt, Le, Gt, Ge, Eof }

fn parse_or(t: &[ExprToken], p: &mut usize) -> Result<i64, String> {
    let mut v = parse_and(t, p)?;
    while t.get(*p) == Some(&ExprToken::Or) { *p += 1; v = if v != 0 || parse_and(t, p)? != 0 { 1 } else { 0 }; }
    Ok(v)
}
fn parse_and(t: &[ExprToken], p: &mut usize) -> Result<i64, String> {
    let mut v = parse_bitor(t, p)?;
    while t.get(*p) == Some(&ExprToken::And) { *p += 1; let r = parse_bitor(t, p)?; v = if v != 0 && r != 0 { 1 } else { 0 }; }
    Ok(v)
}
fn parse_bitor(t: &[ExprToken], p: &mut usize) -> Result<i64, String> {
    let mut v = parse_bitxor(t, p)?;
    while t.get(*p) == Some(&ExprToken::BitOr) { *p += 1; v |= parse_bitxor(t, p)?; }
    Ok(v)
}
fn parse_bitxor(t: &[ExprToken], p: &mut usize) -> Result<i64, String> {
    let mut v = parse_bitand(t, p)?;
    while t.get(*p) == Some(&ExprToken::BitXor) { *p += 1; v ^= parse_bitand(t, p)?; }
    Ok(v)
}
fn parse_bitand(t: &[ExprToken], p: &mut usize) -> Result<i64, String> {
    let mut v = parse_eq(t, p)?;
    while t.get(*p) == Some(&ExprToken::BitAnd) { *p += 1; v &= parse_eq(t, p)?; }
    Ok(v)
}
fn parse_eq(t: &[ExprToken], p: &mut usize) -> Result<i64, String> {
    let mut v = parse_rel(t, p)?;
    loop {
        if t.get(*p) == Some(&ExprToken::Eq) { *p += 1; v = if v == parse_rel(t, p)? { 1 } else { 0 }; }
        else if t.get(*p) == Some(&ExprToken::Ne) { *p += 1; v = if v != parse_rel(t, p)? { 1 } else { 0 }; }
        else { break; }
    }
    Ok(v)
}
fn parse_rel(t: &[ExprToken], p: &mut usize) -> Result<i64, String> {
    let mut v = parse_add(t, p)?;
    loop {
        if t.get(*p) == Some(&ExprToken::Lt) { *p += 1; v = if v < parse_add(t, p)? { 1 } else { 0 }; }
        else if t.get(*p) == Some(&ExprToken::Le) { *p += 1; v = if v <= parse_add(t, p)? { 1 } else { 0 }; }
        else if t.get(*p) == Some(&ExprToken::Gt) { *p += 1; v = if v > parse_add(t, p)? { 1 } else { 0 }; }
        else if t.get(*p) == Some(&ExprToken::Ge) { *p += 1; v = if v >= parse_add(t, p)? { 1 } else { 0 }; }
        else { break; }
    }
    Ok(v)
}
fn parse_add(t: &[ExprToken], p: &mut usize) -> Result<i64, String> {
    let mut v = parse_mul(t, p)?;
    loop {
        if t.get(*p) == Some(&ExprToken::Plus) { *p += 1; v += parse_mul(t, p)?; }
        else if t.get(*p) == Some(&ExprToken::Minus) { *p += 1; v -= parse_mul(t, p)?; }
        else { break; }
    }
    Ok(v)
}
fn parse_mul(t: &[ExprToken], p: &mut usize) -> Result<i64, String> {
    let mut v = parse_unary(t, p)?;
    loop {
        if t.get(*p) == Some(&ExprToken::Mul) { *p += 1; v *= parse_unary(t, p)?; }
        else if t.get(*p) == Some(&ExprToken::Div) {
            *p += 1; let r = parse_unary(t, p)?;
            if r == 0 { return Err("division by zero in #if expression".to_string()); }
            v /= r;
        }
        else if t.get(*p) == Some(&ExprToken::Mod) {
            *p += 1; let r = parse_unary(t, p)?;
            if r == 0 { return Err("modulo by zero in #if expression".to_string()); }
            v %= r;
        }
        else { break; }
    }
    Ok(v)
}
fn parse_unary(t: &[ExprToken], p: &mut usize) -> Result<i64, String> {
    if t.get(*p) == Some(&ExprToken::Not) { *p += 1; return Ok(if parse_unary(t, p)? == 0 { 1 } else { 0 }); }
    if t.get(*p) == Some(&ExprToken::BitNot) { *p += 1; return Ok(!parse_unary(t, p)?); }
    if t.get(*p) == Some(&ExprToken::Minus) { *p += 1; return Ok(-parse_unary(t, p)?); }
    if t.get(*p) == Some(&ExprToken::Plus) { *p += 1; return parse_unary(t, p); }
    parse_primary_expr(t, p)
}
fn parse_primary_expr(t: &[ExprToken], p: &mut usize) -> Result<i64, String> {
    match t.get(*p) {
        Some(ExprToken::Num(n)) => { let v = *n; *p += 1; Ok(v) }
        Some(ExprToken::LParen) => {
            *p += 1;
            let v = parse_or(t, p)?;
            if t.get(*p) != Some(&ExprToken::RParen) {
                return Err("expected ')' in #if expression".to_string());
            }
            *p += 1;
            Ok(v)
        }
        other => Err(format!("unexpected token {:?} in #if expression", other)),
    }
}
