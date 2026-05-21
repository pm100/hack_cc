pub mod lexer;
pub mod parser;
pub mod preprocessor;
pub mod sema;
pub mod codegen;
pub mod assembler;
pub mod output;
pub mod linker;
pub mod mapfile;

pub use codegen::{FONT_BASE, DataInit, CompiledProgram};

use thiserror::Error;
use std::collections::HashMap;
use std::path::PathBuf;

/// Options for a full compilation.
#[derive(Debug)]
pub struct CompileOptions {
    /// Extra `-I` include directories for `#include <...>`.
    pub include_dirs: Vec<PathBuf>,
    /// Pre-defined macros (equivalent to `-D NAME=VALUE` on the command line).
    pub defines: HashMap<String, String>,
    /// Library search directories for the linker.
    pub lib_dirs: Vec<PathBuf>,
    /// Emit source-level debug info (`.dbg` directives) during codegen.
    pub debug: bool,
}

impl Default for CompileOptions {
    fn default() -> Self {
        CompileOptions {
            include_dirs: Vec::new(),
            defines: HashMap::new(),
            lib_dirs: linker::default_lib_dirs(),
            debug: false,
        }
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Preprocess(#[from] preprocessor::PreprocError),
    #[error("lex error: {0}")]
    Lex(#[from] lexer::LexError),
    #[error("parse error: {0}")]
    Parse(#[from] parser::ParseError),
    #[error("semantic error: {0}")]
    Sema(#[from] sema::SemaError),
    #[error("codegen error: {0}")]
    Codegen(#[from] codegen::CodegenError),
    #[error("assemble error: {0}")]
    Assemble(#[from] assembler::AssembleError),
}

pub fn compile(source: &str) -> Result<CompiledProgram, Error> {
    let expanded = preprocessor::preprocess(source, None)?;
    let tokens = lexer::lex(&expanded)?;
    let program = parser::parse(tokens)?;
    let sema_result = sema::analyze(program)?;
    let mut compiled = codegen::generate(sema_result)?;
    compiled.asm = linker::link(&compiled.asm, &linker::default_lib_dirs());
    Ok(compiled)
}

pub fn compile_with_path(source: &str, base_dir: Option<&std::path::Path>) -> Result<CompiledProgram, Error> {
    compile_with_options(source, base_dir, &[])
}

/// Like [`compile_with_path`] but also accepts extra `-I` include directories.
pub fn compile_with_options(
    source: &str,
    base_dir: Option<&std::path::Path>,
    include_dirs: &[PathBuf],
) -> Result<CompiledProgram, Error> {
    let expanded = preprocessor::preprocess_with_dirs(source, base_dir, include_dirs)?;
    let tokens = lexer::lex(&expanded)?;
    let program = parser::parse(tokens)?;
    let sema_result = sema::analyze(program)?;
    let mut compiled = codegen::generate(sema_result)?;
    compiled.asm = linker::link(&compiled.asm, &linker::default_lib_dirs());
    Ok(compiled)
}

/// Full compile with all options: include dirs, pre-defined macros, output mode.
pub fn compile_with_full_options(
    source: &str,
    base_dir: Option<&std::path::Path>,
    opts: &CompileOptions,
) -> Result<CompiledProgram, Error> {
    let expanded = preprocessor::preprocess_with_predefined(source, base_dir, &opts.include_dirs, &opts.defines)?;
    let tokens = lexer::lex(&expanded)?;
    let program = parser::parse(tokens)?;
    let sema_result = sema::analyze(program)?;
    let mut compiled = codegen::generate(sema_result)?;
    compiled.asm = linker::link(&compiled.asm, &opts.lib_dirs);
    Ok(compiled)
}

/// Compile a single C source file to an annotated `.s` object file, with full options.
///
/// Like [`compile_to_object`] but honours `-I` include directories and `-D` pre-defined macros.
pub fn compile_to_object_with_options(
    source: &str,
    base_dir: Option<&std::path::Path>,
    opts: &CompileOptions,
    debug_name: Option<&str>,
) -> Result<String, Error> {
    let expanded = preprocessor::preprocess_with_predefined(source, base_dir, &opts.include_dirs, &opts.defines)?;
    let tokens = lexer::lex(&expanded)?;
    let program = parser::parse(tokens)?;

    let provides: Vec<String> = program.funcs.iter()
        .filter(|f| !f.is_decl && !f.is_static)
        .map(|f| f.name.clone())
        .collect();

    let sema_result = sema::analyze(program)?;

    let string_literals = sema_result.string_literals.clone();
    let globals_info = sema_result.globals.clone();
    let struct_defs = sema_result.struct_defs.clone();

    let compiled = if opts.debug {
        if let Some(name) = debug_name {
            codegen::generate_body_only_with_debug(sema_result, name.to_string())?
        } else {
            codegen::generate_body_only(sema_result)?
        }
    } else {
        codegen::generate_body_only(sema_result)?
    };

    let out = build_object_text(&provides, &string_literals, &globals_info, &struct_defs, &compiled);
    Ok(out)
}

/// Compile a single C source file to an annotated `.s` object file.
///
/// The resulting string is a `.s` file with `.provides` and `.data` directives
/// followed by the function bodies. No bootstrap code is included; use `hack_ld`
/// to link one or more `.s` files into a final executable.
pub fn compile_to_object(source: &str, base_dir: Option<&std::path::Path>) -> Result<String, Error> {
    let expanded = preprocessor::preprocess(source, base_dir)?;
    let tokens = lexer::lex(&expanded)?;
    let program = parser::parse(tokens)?;

    let provides: Vec<String> = program.funcs.iter()
        .filter(|f| !f.is_decl && !f.is_static)
        .map(|f| f.name.clone())
        .collect();

    let sema_result = sema::analyze(program)?;

    // Clone data needed for .data directives before moving sema_result into codegen
    let string_literals = sema_result.string_literals.clone();
    let globals_info = sema_result.globals.clone();
    let struct_defs = sema_result.struct_defs.clone();

    let compiled = codegen::generate_body_only(sema_result)?;

    let out = build_object_text(&provides, &string_literals, &globals_info, &struct_defs, &compiled);
    Ok(out)
}

/// Compile multiple C source files and link them into a single program.
///
/// Each file is compiled independently to a body-only `.s` object, then
/// all objects are linked together with a generated bootstrap.  This avoids
/// the previous AST-merge approach and works correctly across translation
/// units that share globals (both reference the same `@__g_name` symbol).
pub fn compile_files(files: &[(&str, Option<&std::path::Path>)]) -> Result<CompiledProgram, Error> {
    compile_files_with_options(files, &[])
}

/// Like [`compile_files`] but also accepts extra `-I` include directories.
pub fn compile_files_with_options(
    files: &[(&str, Option<&std::path::Path>)],
    include_dirs: &[PathBuf],
) -> Result<CompiledProgram, Error> {
    let mut objects: Vec<String> = Vec::new();
    for (source, base_dir) in files {
        let expanded = preprocessor::preprocess_with_dirs(source, *base_dir, include_dirs)?;
        let tokens = lexer::lex(&expanded)?;
        let program = parser::parse(tokens)?;
        let provides: Vec<String> = program.funcs.iter()
            .filter(|f| !f.is_decl && !f.is_static)
            .map(|f| f.name.clone())
            .collect();
        let sema_result = sema::analyze(program)?;
        let string_literals = sema_result.string_literals.clone();
        let globals_info = sema_result.globals.clone();
        let struct_defs = sema_result.struct_defs.clone();
        let compiled = codegen::generate_body_only(sema_result)?;
        objects.push(build_object_text(&provides, &string_literals, &globals_info, &struct_defs, &compiled));
    }
    link_objects(&objects, &linker::default_lib_dirs())
}

/// Like [`compile_files_with_options`] but accepts full [`CompileOptions`].
pub fn compile_files_with_full_options(
    files: &[(&str, Option<&std::path::Path>)],
    opts: &CompileOptions,
) -> Result<CompiledProgram, Error> {
    let mut objects: Vec<String> = Vec::new();
    for (source, base_dir) in files {
        let expanded = preprocessor::preprocess_with_predefined(source, *base_dir, &opts.include_dirs, &opts.defines)?;
        let tokens = lexer::lex(&expanded)?;
        let program = parser::parse(tokens)?;
        let provides: Vec<String> = program.funcs.iter()
            .filter(|f| !f.is_decl && !f.is_static)
            .map(|f| f.name.clone())
            .collect();
        let sema_result = sema::analyze(program)?;
        let string_literals = sema_result.string_literals.clone();
        let globals_info = sema_result.globals.clone();
        let struct_defs = sema_result.struct_defs.clone();
        let compiled = codegen::generate_body_only(sema_result)?;
        objects.push(build_object_text(&provides, &string_literals, &globals_info, &struct_defs, &compiled));
    }
    link_objects(&objects, &opts.lib_dirs)
}

/// Build an object `.s` text from the parts of a single compiled translation unit.
/// Used by both `compile_to_object` and the multi-file `compile_files_*` paths.
fn build_object_text(
    provides: &[String],
    string_literals: &[(String, Vec<i16>)],
    globals_info: &[(String, crate::parser::Type, Option<sema::GlobalInit>)],
    struct_defs: &std::collections::HashMap<String, Vec<(String, crate::parser::Type)>>,
    compiled: &codegen::CompiledProgram,
) -> String {
    let mut out = String::new();
    out.push_str(".provides");
    for p in provides {
        out.push(' ');
        out.push_str(p);
    }
    out.push('\n');
    // String literal chars
    for (sym_prefix, chars) in string_literals {
        let n = chars.len();
        for (i, &ch) in chars.iter().enumerate() {
            let sym = if i == 0 { sym_prefix.clone() } else { format!("{}_{}", sym_prefix, i) };
            out.push_str(&format!(".data {} {}\n", sym, ch));
        }
        out.push_str(&format!(".data {}_{} 0\n", sym_prefix, n));
    }
    // Multi-word globals (arrays, structs, longs) — emit actual init values
    for (sym, ty, init_val) in globals_info {
        let size = sema::type_size(ty, struct_defs).max(1);
        if size > 1 {
            for i in 0..size {
                let elem_sym = if i == 0 { sym.clone() } else { format!("{}_{}", sym, i) };
                let val: i16 = match (init_val, ty) {
                    (Some(sema::GlobalInit::Array(vals)), _) => {
                        vals.get(i).copied().unwrap_or(0) as i16
                    }
                    (Some(sema::GlobalInit::Scalar(v)), crate::parser::Type::Long) => {
                        let v32 = *v as u32;
                        if i == 0 { (v32 >> 16) as i16 } else { v32 as i16 }
                    }
                    _ => 0,
                };
                out.push_str(&format!(".data {} {}\n", elem_sym, val));
            }
        }
    }
    // Scalar globals
    for (sym, ty, init_val) in globals_info {
        let size = sema::type_size(ty, struct_defs).max(1);
        if size == 1 {
            let val = match init_val {
                Some(sema::GlobalInit::Scalar(v)) => *v as i16,
                _ => 0,
            };
            out.push_str(&format!(".data {} {}\n", sym, val));
        }
    }
    out.push_str(&compiled.asm);
    out
}

/// Link pre-compiled object `.s` texts into a `CompiledProgram`.
/// Parses `.data` directives, generates bootstrap init code (including font
/// table if needed), runs the runtime symbol-scan linker, and returns the
/// final assembled program.
fn link_objects(texts: &[String], lib_dirs: &[PathBuf]) -> Result<CompiledProgram, Error> {
    link_objects_for_format(texts, lib_dirs, output::OutputFormat::Asm, false)
}

/// Format-aware variant of [`link_objects`].
///
/// For `Hackem` and `Tst` output the font table is pre-loaded as static
/// `RAM@` data so no bootstrap ASM is generated for it.  For `Asm` and
/// `Hack` output the font init is inlined in the bootstrap.
fn link_objects_for_format(
    texts: &[String],
    lib_dirs: &[PathBuf],
    fmt: output::OutputFormat,
    debug: bool,
) -> Result<CompiledProgram, Error> {
    // 1. Parse .data entries from all objects (file order → allocation order)
    let mut data_entries: Vec<(String, i16)> = Vec::new();
    for text in texts {
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix(".data ") {
                let mut parts = rest.split_whitespace();
                if let (Some(name), Some(val_str)) = (parts.next(), parts.next()) {
                    if name.starts_with('@') { continue; }
                    if let Ok(v) = val_str.parse::<i16>() {
                        data_entries.push((name.to_string(), v));
                    }
                }
            }
        }
    }

    // 2. Combine all body texts
    let mut combined_bodies = String::new();
    for text in texts {
        combined_bodies.push_str(text);
        combined_bodies.push('\n');
    }

    // 3. Run the runtime linker on bootstrap+bodies so that symbols referenced
    //    by the bootstrap (e.g. @__vm_call) are included in symbol discovery.
    //    SP is set above all data entries + a margin for runtime scratch variables.
    let sp_base = std::cmp::max(256u32, 16 + data_entries.len() as u32 + 64) as u16;

    // For Hackem/Tst: just reserve RAM slots (no store instructions) — values
    // come from RAM@ sections.  For Asm/Hack: emit full init code inline.
    let use_ram_sections = matches!(fmt,
        output::OutputFormat::Hackem | output::OutputFormat::Tst);
    let init_code = if use_ram_sections {
        gen_data_addr_reservations(&data_entries)
    } else {
        gen_data_init_code(&data_entries)
    };
    let bootstrap = codegen::gen_bootstrap(&init_code, sp_base);
    let combined = format!("{}\n{}", bootstrap, combined_bodies);
    let do_link = if debug { linker::link_debug } else { linker::link };
    let linked = do_link(&combined, lib_dirs);

    // Pre-computed DataInits for string literals and globals (Hackem/Tst only).
    // Addresses are deterministic: entry[i] → RAM[16 + i] (assembler allocates
    // in first-encounter order, matching the @name reservation stubs above).
    let string_global_data: Vec<codegen::DataInit> = if use_ram_sections {
        data_entries.iter().enumerate()
            .map(|(i, (_, val))| codegen::DataInit {
                address: 16 + i as u16,
                value: *val,
            })
            .collect()
    } else {
        Vec::new()
    };

    // 4. Detect font usage: __draw_char is linked in only when needed.
    let needs_font = linked.contains("(__draw_char)");

    // 5. Handle font based on output format.
    //    Hackem/Tst: font in RAM@ sections (no bootstrap ASM).
    //    Asm/Hack:   inline font init in bootstrap ASM.
    let (asm, mut all_data) = if needs_font {
        match fmt {
            output::OutputFormat::Hackem | output::OutputFormat::Tst => {
                (linked, codegen::gen_font_data_inits())
            }
            _ => {
                let mut full_init = gen_data_init_code(&data_entries);
                full_init.push_str(&codegen::gen_font_init_asm());
                let full_bootstrap = codegen::gen_bootstrap(&full_init, sp_base);
                let combined2 = format!("{}\n{}", full_bootstrap, combined_bodies);
                (do_link(&combined2, lib_dirs), Vec::new())
            }
        }
    } else {
        (linked, Vec::new())
    };

    // Merge string/global data ahead of font data (address order is maintained
    // by emit_ram_sections which sorts before writing).
    let mut data = string_global_data;
    data.append(&mut all_data);

    Ok(CompiledProgram { asm, data })
}

/// Compile one or more C source files to a linked `CompiledProgram`, using
/// the same two-step (compile-to-object then link) path as `hack_cc -c` +
/// `hack_ld`.  The `fmt` parameter controls font-table handling so that
/// `Hackem`/`Tst` output never embeds font init code in the bootstrap ROM.
///
/// Each tuple is `(source_text, base_dir, debug_name)`. `debug_name` is the
/// source file path emitted in `.dbg` directives when `opts.debug` is true.
pub fn compile_and_link(
    files: &[(&str, Option<&std::path::Path>, Option<&str>)],
    opts: &CompileOptions,
    fmt: output::OutputFormat,
) -> Result<CompiledProgram, Error> {
    let mut objects: Vec<String> = Vec::new();
    for (source, base_dir, debug_name) in files {
        let expanded = preprocessor::preprocess_with_predefined(source, *base_dir, &opts.include_dirs, &opts.defines)?;
        let tokens = lexer::lex(&expanded)?;
        let program = parser::parse(tokens)?;
        let provides: Vec<String> = program.funcs.iter()
            .filter(|f| !f.is_decl && !f.is_static)
            .map(|f| f.name.clone())
            .collect();
        let sema_result = sema::analyze(program)?;
        let string_literals = sema_result.string_literals.clone();
        let globals_info = sema_result.globals.clone();
        let struct_defs = sema_result.struct_defs.clone();
        let compiled = if opts.debug {
            if let Some(name) = debug_name {
                codegen::generate_body_only_with_debug(sema_result, name.to_string())?
            } else {
                codegen::generate_body_only(sema_result)?
            }
        } else {
            codegen::generate_body_only(sema_result)?
        };
        objects.push(build_object_text(&provides, &string_literals, &globals_info, &struct_defs, &compiled));
    }
    link_objects_for_format(&objects, &opts.lib_dirs, fmt, opts.debug)
}


/// For `Hackem`/`Tst` formats: emit `.alloc name` per entry so the assembler
/// reserves RAM slots in the correct order but emits zero ROM instructions —
/// values will be pre-loaded via `RAM@` sections instead.
fn gen_data_addr_reservations(entries: &[(String, i16)]) -> String {
    entries.iter().map(|(name, _)| format!(".alloc {}\n", name)).collect()
}

fn gen_data_init_code(entries: &[(String, i16)]) -> String {
    let mut out = String::new();
    for (name, val) in entries {
        let v = *val;
        if v == 0 {
            out.push_str(&format!("@{}\n", name));
        } else if v == 1 {
            out.push_str(&format!("D=1\n@{}\nM=D\n", name));
        } else if v == -1 {
            out.push_str(&format!("D=-1\n@{}\nM=D\n", name));
        } else if v > 0 {
            out.push_str(&format!("@{}\nD=A\n@{}\nM=D\n", v, name));
        } else {
            out.push_str(&format!("@{}\nD=-A\n@{}\nM=D\n", -(v as i32), name));
        }
    }
    out
}

/// Escape a string for JSON serialization.
pub fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"'  => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c    => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Build and write a `.pdb` JSON debug database alongside the output file.
///
/// * `asm`          – Final linked assembly text.
/// * `sources`      – `(file_content, file_path)` pairs for primary C source files
///                    whose lines should be embedded in the PDB.
/// * `input_paths`  – Command-line source file paths; these seed the `file_info`
///                    ordering so they appear first (file index 0, 1, …).
/// * `out_path`     – Output binary path; `.pdb` is written alongside it.
pub fn write_pdb(
    asm: &str,
    sources: &[(String, PathBuf)],
    input_paths: &[PathBuf],
    out_path: &std::path::Path,
) {
    let ar = match assembler::assemble_with_symbols(asm, 16) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("warning: could not assemble for PDB: {}", e);
            return;
        }
    };

    // Build file_info list: primary sources first, then any extras from .dbg entries.
    let mut file_names: Vec<String> = Vec::new();
    let mut file_index: HashMap<String, usize> = HashMap::new();
    for p in input_paths {
        if let Some(s) = p.to_str() {
            if !file_index.contains_key(s) {
                let idx = file_names.len();
                file_index.insert(s.to_string(), idx);
                file_names.push(s.to_string());
            }
        }
    }
    for (_, file, _) in &ar.dbg_entries {
        if !file_index.contains_key(file) {
            let idx = file_names.len();
            file_index.insert(file.clone(), idx);
            file_names.push(file.clone());
        }
    }

    // source_lines: flat list of all lines from the primary source files.
    let mut source_lines: Vec<String> = Vec::new();
    for (src_text, path) in sources {
        if let Some(name) = path.to_str() {
            if file_index.contains_key(name) {
                for line in src_text.lines() {
                    source_lines.push(line.to_string());
                }
            }
        }
    }

    // source_map: one entry per .dbg annotation.
    let mut source_map_entries: Vec<(usize, usize, usize, u16)> = Vec::new();
    for (addr, file, line_no) in &ar.dbg_entries {
        if let Some(&fi) = file_index.get(file) {
            source_map_entries.push((fi, *line_no as usize, 0, *addr));
        }
    }

    // symbols: ROM labels → Func, RAM vars → Var.
    let mut symbols_json = String::new();
    for (name, addr) in &ar.rom_labels {
        let sym = format!(
            r#"{{"symbol_type":"Func","name":{},"func_type":0,"var_type":0,"storage_class":0,"size":0,"address":{},"instance_type":"","file_type":"C"}}"#,
            json_str(name), addr
        );
        if !symbols_json.is_empty() { symbols_json.push(','); }
        symbols_json.push_str(&sym);
    }
    for (name, addr) in &ar.ram_vars {
        let sym = format!(
            r#"{{"symbol_type":"Var","name":{},"func_type":0,"var_type":0,"storage_class":0,"size":1,"address":{},"instance_type":"","file_type":"C"}}"#,
            json_str(name), addr
        );
        if !symbols_json.is_empty() { symbols_json.push(','); }
        symbols_json.push_str(&sym);
    }

    let mut file_info_json = String::new();
    for name in &file_names {
        if !file_info_json.is_empty() { file_info_json.push(','); }
        let ft = if name.ends_with(".s") { "Asm" } else { "C" };
        file_info_json.push_str(&format!(
            r#"{{"name":{},"file_type":"{}"}}"#,
            json_str(name), ft
        ));
    }

    let source_lines_json: String = source_lines.iter()
        .map(|l| json_str(l))
        .collect::<Vec<_>>()
        .join(",");

    let source_map_json: String = source_map_entries.iter()
        .map(|(fi, ln, col, addr)| format!(
            r#"{{"file":{},"line_no":{},"col_no":{},"addr":{}}}"#,
            fi, ln, col, addr
        ))
        .collect::<Vec<_>>()
        .join(",");

    let pdb_json = format!(
        "{{\n  \"symbols\": [{}],\n  \"source_lines\": [{}],\n  \"source_map\": [{}],\n  \"file_info\": [{}]\n}}\n",
        symbols_json, source_lines_json, source_map_json, file_info_json
    );

    let pdb_path = out_path.with_extension("pdb");
    std::fs::write(&pdb_path, &pdb_json).unwrap_or_else(|e| {
        eprintln!("error writing PDB {:?}: {}", pdb_path, e);
    });
    eprintln!("wrote debug info {:?}", pdb_path);
}
