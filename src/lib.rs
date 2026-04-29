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

    let mut out = String::new();
    // Directive: which symbols this object file provides
    out.push_str(".provides");
    for p in &provides {
        out.push(' ');
        out.push_str(p);
    }
    out.push('\n');

    // Emit .data directives for all globals and string literals (in allocation order).
    // hack_ld uses these to generate bootstrap init code with correct consecutive allocation.

    // 1. String literal chars (symbol prefix, each char, then null terminator)
    for (sym_prefix, chars) in &string_literals {
        let n = chars.len();
        for (i, &ch) in chars.iter().enumerate() {
            let sym = if i == 0 { sym_prefix.clone() } else { format!("{}_{}", sym_prefix, i) };
            out.push_str(&format!(".data {} {}\n", sym, ch));
        }
        out.push_str(&format!(".data {}_{} 0\n", sym_prefix, n));
    }

    // 2. Multi-word globals (all elements, zero-initialized)
    for (name, ty, _init_val) in &globals_info {
        let sym = format!("__g_{}", name);
        let size = sema::type_size(ty, &struct_defs).max(1);
        if size > 1 {
            for i in 0..size {
                let elem_sym = if i == 0 { sym.clone() } else { format!("{}_{}", sym, i) };
                out.push_str(&format!(".data {} 0\n", elem_sym));
            }
        }
    }

    // 3. Scalar globals (all, to establish deterministic allocation order)
    for (name, ty, init_val) in &globals_info {
        let sym = format!("__g_{}", name);
        let size = sema::type_size(ty, &struct_defs).max(1);
        if size == 1 {
            let val = init_val.unwrap_or(0) as i16;
            out.push_str(&format!(".data {} {}\n", sym, val));
        }
    }

    // Font table data (absolute addresses)
    for d in &compiled.data {
        out.push_str(&format!(".data @{} {}\n", d.address, d.value));
    }

    out.push_str(&compiled.asm);
    Ok(out)
}

/// Compile multiple C source files and link them into a single program.
///
/// Each `(source, base_dir)` pair is preprocessed and parsed independently.
/// The resulting `Program` ASTs are merged — struct definitions are
/// deduplicated by name, and duplicate forward declarations (e.g., from
/// headers included in several files) are collapsed into one.  The merged
/// program then goes through the normal sema → codegen → linker pipeline.
pub fn compile_files(files: &[(&str, Option<&std::path::Path>)]) -> Result<CompiledProgram, Error> {
    compile_files_with_options(files, &[])
}

/// Like [`compile_files`] but also accepts extra `-I` include directories.
pub fn compile_files_with_options(
    files: &[(&str, Option<&std::path::Path>)],
    include_dirs: &[PathBuf],
) -> Result<CompiledProgram, Error> {
    let mut merged = parser::Program {
        struct_defs: vec![],
        globals: vec![],
        funcs: vec![],
    };

    let mut seen_structs: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut seen_globals: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut defined_funcs: std::collections::HashSet<String> = std::collections::HashSet::new();

    let mut parsed_programs: Vec<parser::Program> = Vec::new();
    for (source, base_dir) in files {
        let expanded = preprocessor::preprocess_with_dirs(source, *base_dir, include_dirs)?;
        let tokens = lexer::lex(&expanded)?;
        let program = parser::parse(tokens)?;
        for f in &program.funcs {
            if !f.is_decl {
                defined_funcs.insert(f.name.clone());
            }
        }
        parsed_programs.push(program);
    }

    for program in parsed_programs {
        for sd in program.struct_defs {
            if seen_structs.insert(sd.name.clone()) {
                merged.struct_defs.push(sd);
            }
        }
        for g in program.globals {
            if seen_globals.insert(g.1.clone()) {
                merged.globals.push(g);
            }
        }
        for f in program.funcs {
            if f.is_decl && defined_funcs.contains(&f.name) {
                continue;
            }
            merged.funcs.push(f);
        }
    }

    let sema_result = sema::analyze(merged)?;
    let mut compiled = codegen::generate(sema_result)?;
    compiled.asm = linker::link(&compiled.asm, &linker::default_lib_dirs());
    Ok(compiled)
}

/// Like [`compile_files_with_options`] but accepts full [`CompileOptions`].
pub fn compile_files_with_full_options(
    files: &[(&str, Option<&std::path::Path>)],
    opts: &CompileOptions,
) -> Result<CompiledProgram, Error> {
    let mut merged = parser::Program {
        struct_defs: vec![],
        globals: vec![],
        funcs: vec![],
    };

    let mut seen_structs: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut seen_globals: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut defined_funcs: std::collections::HashSet<String> = std::collections::HashSet::new();

    let mut parsed_programs: Vec<parser::Program> = Vec::new();
    for (source, base_dir) in files {
        let expanded = preprocessor::preprocess_with_predefined(source, *base_dir, &opts.include_dirs, &opts.defines)?;
        let tokens = lexer::lex(&expanded)?;
        let program = parser::parse(tokens)?;
        for f in &program.funcs {
            if !f.is_decl {
                defined_funcs.insert(f.name.clone());
            }
        }
        parsed_programs.push(program);
    }

    for program in parsed_programs {
        for sd in program.struct_defs {
            if seen_structs.insert(sd.name.clone()) {
                merged.struct_defs.push(sd);
            }
        }
        for g in program.globals {
            if seen_globals.insert(g.1.clone()) {
                merged.globals.push(g);
            }
        }
        for f in program.funcs {
            if f.is_decl && defined_funcs.contains(&f.name) {
                continue;
            }
            merged.funcs.push(f);
        }
    }

    let sema_result = sema::analyze(merged)?;
    let mut compiled = codegen::generate(sema_result)?;
    compiled.asm = linker::link(&compiled.asm, &opts.lib_dirs);
    Ok(compiled)
}
