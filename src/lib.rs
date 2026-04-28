pub mod lexer;
pub mod parser;
pub mod preprocessor;
pub mod sema;
pub mod codegen;
pub mod assembler;
pub mod output;
pub mod linker;
pub mod object;

pub use codegen::{FONT_BASE, DataInit, CompiledProgram};
pub use object::ObjectFile;

use thiserror::Error;
use std::collections::HashMap;

/// Options for a full compilation.
#[derive(Debug, Default)]
pub struct CompileOptions {
    /// Extra `-I` include directories for `#include <...>`.
    pub include_dirs: Vec<std::path::PathBuf>,
    /// Pre-defined macros (equivalent to `-D NAME=VALUE` on the command line).
    pub defines: HashMap<String, String>,
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
    compiled.asm = linker::link(&compiled.asm);
    Ok(compiled)
}

pub fn compile_with_path(source: &str, base_dir: Option<&std::path::Path>) -> Result<CompiledProgram, Error> {
    compile_with_options(source, base_dir, &[])
}

/// Like [`compile_with_path`] but also accepts extra `-I` include directories.
pub fn compile_with_options(
    source: &str,
    base_dir: Option<&std::path::Path>,
    include_dirs: &[std::path::PathBuf],
) -> Result<CompiledProgram, Error> {
    let expanded = preprocessor::preprocess_with_dirs(source, base_dir, include_dirs)?;
    let tokens = lexer::lex(&expanded)?;
    let program = parser::parse(tokens)?;
    let sema_result = sema::analyze(program)?;
    let mut compiled = codegen::generate(sema_result)?;
    compiled.asm = linker::link(&compiled.asm);
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
    compiled.asm = linker::link(&compiled.asm);
    Ok(compiled)
}

/// Compile a single C source file to an object file (`.hobj`).
///
/// The resulting [`ObjectFile`] contains only function bodies — no bootstrap
/// code and no entry-point call.  Use `hack_ld` to link one or more object
/// files into a final executable.
pub fn compile_to_object(source: &str, base_dir: Option<&std::path::Path>) -> Result<ObjectFile, Error> {
    let expanded = preprocessor::preprocess(source, base_dir)?;
    let tokens = lexer::lex(&expanded)?;
    let program = parser::parse(tokens)?;

    // Collect forward-declared function names (declared in headers but defined elsewhere).
    // These are valid call targets in separate compilation — the linker provides them.
    let decl_names: Vec<String> = program.funcs.iter()
        .filter(|f| f.is_decl)
        .map(|f| f.name.clone())
        .collect();
    let decl_refs: Vec<&str> = decl_names.iter().map(|s| s.as_str()).collect();

    // Collect function names before sema (we'll use them as PROVIDES list).
    let provides: Vec<String> = program.funcs.iter()
        .filter(|f| !f.is_decl)
        .map(|f| f.name.clone())
        .collect();

    let sema_result = sema::analyze_for_object_file(program, &decl_refs)?;
    // Body-only: no bootstrap, no entry-point call to main.
    let compiled = codegen::generate_body_only(sema_result)?;
    // Note: runtime modules are NOT linked here; hack_ld handles that.
    // (We don't call linker::link so the .hobj stays as function bodies only.)

    Ok(ObjectFile {
        provides,
        data: compiled.data,
        asm_body: compiled.asm,
    })
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
    include_dirs: &[std::path::PathBuf],
) -> Result<CompiledProgram, Error> {
    let mut merged = parser::Program {
        struct_defs: vec![],
        globals: vec![],
        funcs: vec![],
    };

    let mut seen_structs: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut seen_globals: std::collections::HashSet<String> = std::collections::HashSet::new();
    // Track which function names already have a full definition so we can
    // suppress duplicate forward-declarations from headers.
    let mut defined_funcs: std::collections::HashSet<String> = std::collections::HashSet::new();

    // First pass: collect all definitions so we know what's defined.
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

    // Second pass: merge, deduplicating as we go.
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
            // Drop forward declarations for functions that are defined
            // somewhere in the set — sema will find the definition.
            if f.is_decl && defined_funcs.contains(&f.name) {
                continue;
            }
            merged.funcs.push(f);
        }
    }

    let sema_result = sema::analyze(merged)?;
    let mut compiled = codegen::generate(sema_result)?;
    compiled.asm = linker::link(&compiled.asm);
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
    compiled.asm = linker::link(&compiled.asm);
    Ok(compiled)
}
