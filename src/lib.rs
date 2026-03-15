pub mod lexer;
pub mod parser;
pub mod sema;
pub mod codegen;
pub mod assembler;
pub mod output;

pub use codegen::{FONT_BASE, DataInit, CompiledProgram};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
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
    let tokens = lexer::lex(source)?;
    let program = parser::parse(tokens)?;
    let sema_result = sema::analyze(program)?;
    let compiled = codegen::generate(sema_result)?;
    Ok(compiled)
}
