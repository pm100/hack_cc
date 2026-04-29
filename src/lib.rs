pub mod lexer;
pub mod parser;
pub mod preprocessor;
pub mod sema;
pub mod codegen;
pub mod assembler;
pub mod output;
pub mod linker;

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
}

impl Default for CompileOptions {
    fn default() -> Self {
        CompileOptions {
            include_dirs: Vec::new(),
            defines: HashMap::new(),
            lib_dirs: linker::default_lib_dirs(),
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
        .filter(|f| !f.is_decl)
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
            .filter(|f| !f.is_decl)
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
            .filter(|f| !f.is_decl)
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
    globals_info: &[(String, crate::parser::Type, Option<i32>)],
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
    // Multi-word globals
    for (name, ty, _) in globals_info {
        let sym = format!("__g_{}", name);
        let size = sema::type_size(ty, struct_defs).max(1);
        if size > 1 {
            for i in 0..size {
                let elem_sym = if i == 0 { sym.clone() } else { format!("{}_{}", sym, i) };
                out.push_str(&format!(".data {} 0\n", elem_sym));
            }
        }
    }
    // Scalar globals
    for (name, ty, init_val) in globals_info {
        let sym = format!("__g_{}", name);
        let size = sema::type_size(ty, struct_defs).max(1);
        if size == 1 {
            let val = init_val.unwrap_or(0) as i16;
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

    // 3. Run runtime linker on bodies (pulls in needed library modules)
    let linked_bodies = linker::link(&combined_bodies, lib_dirs);

    // 4. Detect font usage: __draw_char linked in iff its label appears
    let needs_font = linked_bodies.contains("(__draw_char)");

    // 5. Build init code: data init + (font init if needed)
    let mut init_code = gen_data_init_code(&data_entries);
    if needs_font {
        init_code.push_str(&codegen::gen_font_init_asm());
    }

    // 6. Build bootstrap and prepend to linked bodies
    let bootstrap = codegen::gen_bootstrap(&init_code);
    let asm = format!("{}\n{}", bootstrap, linked_bodies);

    Ok(CompiledProgram { asm, data: Vec::new() })
}

/// Convert symbolic name-value pairs into Hack assembly init instructions.
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
