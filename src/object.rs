/// Hack Object File (.hobj) — intermediate format produced by `hack_cc -c`.
///
/// An object file contains the compiled function bodies for one C translation
/// unit, without the bootstrap code.  Multiple object files are combined by
/// `hack_ld` into a final executable.
///
/// # Text format
///
/// ```text
/// HACK_OBJ 1
/// PROVIDES main print_int
/// DATA
/// 16 72
/// 17 101
/// ASM
/// (main)
/// @ARG
/// ...
/// END_HACK_OBJ
/// ```
///
/// * `PROVIDES` — space-separated list of function labels defined in this TU.
/// * `DATA` section — one `address value` pair per line; non-zero RAM
///   initialisations (global variable initialisations and string literal data).
/// * `ASM` section — raw Hack assembly for all reachable functions.
///
/// # Global address limitation
///
/// Global variables are compiled with absolute RAM addresses starting from
/// RAM[16].  If two independently-compiled TUs both define global variables,
/// their addresses will conflict.  In that case, compile all files together
/// with `hack_cc file1.c file2.c ...` rather than using separate `-c` steps.
/// Functions that use only local/stack variables (no file-scope globals) are
/// always safe to compile separately.

use crate::codegen::DataInit;
use thiserror::Error;

#[derive(Debug, Error)]
#[error("object file error: {0}")]
pub struct ObjError(pub String);

impl ObjError {
    fn new(msg: impl Into<String>) -> Self { Self(msg.into()) }
}

/// The contents of a single `.hobj` file.
#[derive(Debug, Clone)]
pub struct ObjectFile {
    /// Labels defined in this translation unit (user function names).
    pub provides: Vec<String>,
    /// RAM data initialisations (global variable initial values, string literals).
    pub data: Vec<DataInit>,
    /// Hack assembly text for all compiled functions (no bootstrap).
    pub asm_body: String,
}

impl ObjectFile {
    /// Serialise to the text `.hobj` format.
    pub fn serialize(&self) -> String {
        let mut out = String::from("HACK_OBJ 1\n");

        // PROVIDES
        out.push_str("PROVIDES");
        for p in &self.provides {
            out.push(' ');
            out.push_str(p);
        }
        out.push('\n');

        // DATA
        out.push_str("DATA\n");
        for d in &self.data {
            out.push_str(&format!("{} {}\n", d.address, d.value));
        }

        // ASM
        out.push_str("ASM\n");
        out.push_str(&self.asm_body);
        if !self.asm_body.ends_with('\n') {
            out.push('\n');
        }

        out.push_str("END_HACK_OBJ\n");
        out
    }

    /// Parse a `.hobj` text file.
    pub fn parse(text: &str) -> Result<Self, ObjError> {
        let mut lines = text.lines();

        // Header
        let header = lines.next().ok_or_else(|| ObjError::new("empty file"))?;
        if header.trim() != "HACK_OBJ 1" {
            return Err(ObjError::new(format!("unrecognised header: {:?}", header)));
        }

        // PROVIDES
        let prov_line = lines.next().ok_or_else(|| ObjError::new("missing PROVIDES"))?;
        let provides: Vec<String> = prov_line
            .strip_prefix("PROVIDES")
            .ok_or_else(|| ObjError::new("expected PROVIDES line"))?
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        // DATA section
        let data_hdr = lines.next().ok_or_else(|| ObjError::new("missing DATA"))?;
        if data_hdr.trim() != "DATA" {
            return Err(ObjError::new(format!("expected DATA, got {:?}", data_hdr)));
        }
        let mut data: Vec<DataInit> = Vec::new();
        loop {
            let line = lines.next().ok_or_else(|| ObjError::new("unterminated DATA section"))?;
            if line.trim() == "ASM" { break; }
            let mut parts = line.split_whitespace();
            let addr: u16 = parts.next()
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| ObjError::new(format!("bad DATA line: {:?}", line)))?;
            let val: i16 = parts.next()
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| ObjError::new(format!("bad DATA line: {:?}", line)))?;
            data.push(DataInit { address: addr, value: val });
        }

        // ASM section (everything until END_HACK_OBJ)
        let mut asm_lines: Vec<&str> = Vec::new();
        for line in lines {
            if line.trim() == "END_HACK_OBJ" { break; }
            asm_lines.push(line);
        }
        let asm_body = asm_lines.join("\n") + "\n";

        Ok(ObjectFile { provides, data, asm_body })
    }
}
