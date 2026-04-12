/// Code generator: SemaResult -> Hack assembly text.
///
/// Memory layout (nand2tetris convention):
///   RAM[0]  = SP      stack pointer, init 256
///   RAM[1]  = LCL     local base for current frame
///   RAM[2]  = ARG     arg base for current frame
///   RAM[3]  = THIS
///   RAM[4]  = THAT
///   RAM[5-12]  = temp
///   RAM[13] = R13     scratch
///   RAM[14] = R14     scratch
///   RAM[15] = R15     scratch / mul result
///   RAM[16+]  = static/global variables
///   RAM[256+] = stack
///
/// Calling convention: Jack VM (nand2tetris Ch.8).
///   Caller pushes args, then calls.
///   Callee frame: [saved LCL, saved ARG, saved THIS, saved THAT, return addr, locals...]
///   Actually Jack VM saves: return-addr, LCL, ARG, THIS, THAT (5 words of frame metadata).
///
/// Runtime helpers use R3 as the return address register.

/// RAM address used as character output port.
/// The emulator intercepts writes to this address and outputs to stdout.
pub const HACK_OUTPUT_PORT: usize = 32767;

/// Base RAM address of the 8×11 font table (96 chars × 11 rows = 1056 words).
/// Placed just below screen memory: 16384 - 1056 = 15328.
pub const FONT_BASE: usize = 15328;

/// 8×11 bitmap font for ASCII 32–127, sourced from the nand2tetris Jack OS Output.jack.
/// Each entry is 11 bytes, one per screen row, bits 0-5 are the visible pixels
/// (MSB convention; bit-reversed on write to match Hack's LSB-leftmost screen layout).
/// Row 10 (index 10) is always 0 (inter-line spacing).
const FONT_8X11: [[u8; 11]; 96] = [
    [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],  // 32 ' '
    [12,30,30,30,12,12, 0,12,12, 0, 0],  // 33 '!'
    [54,54,20, 0, 0, 0, 0, 0, 0, 0, 0],  // 34 '"'
    [ 0,18,18,63,18,18,63,18,18, 0, 0],  // 35 '#'
    [12,30,51, 3,30,48,51,30,12,12, 0],  // 36 '$'
    [ 0, 0,35,51,24,12, 6,51,49, 0, 0],  // 37 '%'
    [12,30,30,12,54,27,27,27,54, 0, 0],  // 38 '&'
    [12,12, 6, 0, 0, 0, 0, 0, 0, 0, 0],  // 39 '\''
    [24,12, 6, 6, 6, 6, 6,12,24, 0, 0],  // 40 '('
    [ 6,12,24,24,24,24,24,12, 6, 0, 0],  // 41 ')'
    [ 0, 0, 0,51,30,63,30,51, 0, 0, 0],  // 42 '*'
    [ 0, 0, 0,12,12,63,12,12, 0, 0, 0],  // 43 '+'
    [ 0, 0, 0, 0, 0, 0, 0,12,12, 6, 0],  // 44 ','
    [ 0, 0, 0, 0, 0,63, 0, 0, 0, 0, 0],  // 45 '-'
    [ 0, 0, 0, 0, 0, 0, 0,12,12, 0, 0],  // 46 '.'
    [ 0, 0,32,48,24,12, 6, 3, 1, 0, 0],  // 47 '/'
    [12,30,51,51,51,51,51,30,12, 0, 0],  // 48 '0'
    [12,14,15,12,12,12,12,12,63, 0, 0],  // 49 '1'
    [30,51,48,24,12, 6, 3,51,63, 0, 0],  // 50 '2'
    [30,51,48,48,28,48,48,51,30, 0, 0],  // 51 '3'
    [16,24,28,26,25,63,24,24,60, 0, 0],  // 52 '4'
    [63, 3, 3,31,48,48,48,51,30, 0, 0],  // 53 '5'
    [28, 6, 3, 3,31,51,51,51,30, 0, 0],  // 54 '6'
    [63,49,48,48,24,12,12,12,12, 0, 0],  // 55 '7'
    [30,51,51,51,30,51,51,51,30, 0, 0],  // 56 '8'
    [30,51,51,51,62,48,48,24,14, 0, 0],  // 57 '9'
    [ 0, 0,12,12, 0, 0,12,12, 0, 0, 0],  // 58 ':'
    [ 0, 0,12,12, 0, 0,12,12, 6, 0, 0],  // 59 ';'
    [ 0, 0,24,12, 6, 3, 6,12,24, 0, 0],  // 60 '<'
    [ 0, 0, 0,63, 0, 0,63, 0, 0, 0, 0],  // 61 '='
    [ 0, 0, 3, 6,12,24,12, 6, 3, 0, 0],  // 62 '>'
    [30,51,51,24,12,12, 0,12,12, 0, 0],  // 63 '?'
    [30,51,51,59,59,59,27, 3,30, 0, 0],  // 64 '@'
    [12,30,51,51,63,51,51,51,51, 0, 0],  // 65 'A'
    [31,51,51,51,31,51,51,51,31, 0, 0],  // 66 'B'
    [28,54,35, 3, 3, 3,35,54,28, 0, 0],  // 67 'C'
    [15,27,51,51,51,51,51,27,15, 0, 0],  // 68 'D'
    [63,51,35,11,15,11,35,51,63, 0, 0],  // 69 'E'
    [63,51,35,11,15,11, 3, 3, 3, 0, 0],  // 70 'F'
    [28,54,35, 3,59,51,51,54,44, 0, 0],  // 71 'G'
    [51,51,51,51,63,51,51,51,51, 0, 0],  // 72 'H'
    [30,12,12,12,12,12,12,12,30, 0, 0],  // 73 'I'
    [60,24,24,24,24,24,27,27,14, 0, 0],  // 74 'J'
    [51,51,51,27,15,27,51,51,51, 0, 0],  // 75 'K'
    [ 3, 3, 3, 3, 3, 3,35,51,63, 0, 0],  // 76 'L'
    [33,51,63,63,51,51,51,51,51, 0, 0],  // 77 'M'
    [51,51,55,55,63,59,59,51,51, 0, 0],  // 78 'N'
    [30,51,51,51,51,51,51,51,30, 0, 0],  // 79 'O'
    [31,51,51,51,31, 3, 3, 3, 3, 0, 0],  // 80 'P'
    [30,51,51,51,51,51,63,59,30,48, 0],  // 81 'Q'
    [31,51,51,51,31,27,51,51,51, 0, 0],  // 82 'R'
    [30,51,51, 6,28,48,51,51,30, 0, 0],  // 83 'S'
    [63,63,45,12,12,12,12,12,30, 0, 0],  // 84 'T'
    [51,51,51,51,51,51,51,51,30, 0, 0],  // 85 'U'
    [51,51,51,51,51,30,30,12,12, 0, 0],  // 86 'V'
    [51,51,51,51,51,63,63,63,18, 0, 0],  // 87 'W'
    [51,51,30,30,12,30,30,51,51, 0, 0],  // 88 'X'
    [51,51,51,51,30,12,12,12,30, 0, 0],  // 89 'Y'
    [63,51,49,24,12, 6,35,51,63, 0, 0],  // 90 'Z'
    [30, 6, 6, 6, 6, 6, 6, 6,30, 0, 0],  // 91 '['
    [ 0, 0, 1, 3, 6,12,24,48,32, 0, 0],  // 92 '\'
    [30,24,24,24,24,24,24,24,30, 0, 0],  // 93 ']'
    [ 8,28,54, 0, 0, 0, 0, 0, 0, 0, 0],  // 94 '^'
    [ 0, 0, 0, 0, 0, 0, 0, 0, 0,63, 0],  // 95 '_'
    [ 6,12,24, 0, 0, 0, 0, 0, 0, 0, 0],  // 96 '`'
    [ 0, 0, 0,14,24,30,27,27,54, 0, 0],  // 97 'a'
    [ 3, 3, 3,15,27,51,51,51,30, 0, 0],  // 98 'b'
    [ 0, 0, 0,30,51, 3, 3,51,30, 0, 0],  // 99 'c'
    [48,48,48,60,54,51,51,51,30, 0, 0],  // 100 'd'
    [ 0, 0, 0,30,51,63, 3,51,30, 0, 0],  // 101 'e'
    [28,54,38, 6,15, 6, 6, 6,15, 0, 0],  // 102 'f'
    [ 0, 0,30,51,51,51,62,48,51,30, 0],  // 103 'g'
    [ 3, 3, 3,27,55,51,51,51,51, 0, 0],  // 104 'h'
    [12,12, 0,14,12,12,12,12,30, 0, 0],  // 105 'i'
    [48,48, 0,56,48,48,48,48,51,30, 0],  // 106 'j'
    [ 3, 3, 3,51,27,15,15,27,51, 0, 0],  // 107 'k'
    [14,12,12,12,12,12,12,12,30, 0, 0],  // 108 'l'
    [ 0, 0, 0,29,63,43,43,43,43, 0, 0],  // 109 'm'
    [ 0, 0, 0,29,51,51,51,51,51, 0, 0],  // 110 'n'
    [ 0, 0, 0,30,51,51,51,51,30, 0, 0],  // 111 'o'
    [ 0, 0, 0,30,51,51,51,31, 3, 3, 0],  // 112 'p'
    [ 0, 0, 0,30,51,51,51,62,48,48, 0],  // 113 'q'
    [ 0, 0, 0,29,55,51, 3, 3, 7, 0, 0],  // 114 'r'
    [ 0, 0, 0,30,51, 6,24,51,30, 0, 0],  // 115 's'
    [ 4, 6, 6,15, 6, 6, 6,54,28, 0, 0],  // 116 't'
    [ 0, 0, 0,27,27,27,27,27,54, 0, 0],  // 117 'u'
    [ 0, 0, 0,51,51,51,51,30,12, 0, 0],  // 118 'v'
    [ 0, 0, 0,51,51,51,63,63,18, 0, 0],  // 119 'w'
    [ 0, 0, 0,51,30,12,12,30,51, 0, 0],  // 120 'x'
    [ 0, 0, 0,51,51,51,62,48,24,15, 0],  // 121 'y'
    [ 0, 0, 0,63,27,12, 6,51,63, 0, 0],  // 122 'z'
    [56,12,12,12, 7,12,12,12,56, 0, 0],  // 123 '{'
    [12,12,12,12,12,12,12,12,12, 0, 0],  // 124 '|'
    [ 7,12,12,12,56,12,12,12, 7, 0, 0],  // 125 '}'
    [38,45,25, 0, 0, 0, 0, 0, 0, 0, 0],  // 126 '~'
    [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],  // 127 DEL
];

use std::collections::{HashMap, HashSet};
use thiserror::Error;
use crate::sema::{SemaResult, AnnotatedFunc, VarInfo, VarStorage, type_size};
use crate::parser::{Expr, Stmt, BinOp, UnOp, Type, SwitchLabel};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinKind {
    Puts,
    Strlen,
    DrawPixel,
    ClearPixel,
    FillScreen,
    ClearScreen,
    DrawChar,
    DrawString,
    Mul,
    Div,
    // String functions
    Strcpy,
    Strcmp,
    Strcat,
    // I/O
    Itoa,
}

/// A single RAM pre-initialisation entry produced by the compiler.
#[derive(Debug, Clone)]
pub struct DataInit {
    pub address: u16,
    pub value: i16,
}

/// Full result of code generation returned by [`generate`].
pub struct CompiledProgram {
    /// Hack assembly text containing a `// __DATA_INIT_HERE__` marker in the bootstrap
    /// section where data-initialisation code should be inserted for asm/hack formats.
    pub asm: String,
    /// RAM data initialisations (globals, string literals, font table if used).
    pub data: Vec<DataInit>,
}

#[derive(Debug, Error)]
#[error("codegen error: {0}")]
pub struct CodegenError(pub String);

impl CodegenError {
    fn new(msg: impl Into<String>) -> Self { Self(msg.into()) }
}

struct Gen {
    out: Vec<String>,
    label_id: usize,
    string_map: HashMap<String, usize>,
    struct_defs: HashMap<String, Vec<(String, Type)>>,
    used_builtins: HashSet<BuiltinKind>,
    loop_ctx: Vec<(String, String)>,   // (break_label, continue_label)
}

impl Gen {
    fn new(
        string_map: HashMap<String, usize>,
        struct_defs: HashMap<String, Vec<(String, Type)>>,
        used_builtins: HashSet<BuiltinKind>,
    ) -> Self {
        Self { out: Vec::new(), label_id: 0, string_map, struct_defs, used_builtins, loop_ctx: Vec::new() }
    }

    fn emit(&mut self, s: impl Into<String>) {
        self.out.push(s.into());
    }

    fn label(&mut self) -> usize {
        let id = self.label_id;
        self.label_id += 1;
        id
    }

    // ── Stack helpers ────────────────────────────────────────────────────

    /// Push D onto the stack
    fn push_d(&mut self) {
        self.emit("@SP");
        self.emit("A=M");
        self.emit("M=D");
        self.emit("@SP");
        self.emit("M=M+1");
    }

    /// Pop top of stack into D
    fn pop_d(&mut self) {
        self.emit("@SP");
        self.emit("M=M-1");
        self.emit("A=M");
        self.emit("D=M");
    }

    /// Pop top of stack into R13
    fn pop_r13(&mut self) {
        self.pop_d();
        self.emit("@R13");
        self.emit("M=D");
    }

    fn emit_stride_mul(&mut self, stride: usize) {
        if stride == 1 { return; }
        let id = self.label();
        let l_loop = format!("__stride_loop_{}", id);
        let l_done = format!("__stride_done_{}", id);
        self.emit("@R13");
        self.emit("M=D");       // R13 = idx
        self.emit("@R14");
        self.emit("M=0");       // R14 = accumulator = 0
        self.emit(&format!("@{}", stride));
        self.emit("D=A");
        self.emit("@R15");
        self.emit("M=D");       // R15 = stride (loop counter)
        self.emit(&format!("({})", l_loop));
        self.emit("@R15");
        self.emit("D=M");
        self.emit(&format!("@{}", l_done));
        self.emit("D;JEQ");
        self.emit("@R13");
        self.emit("D=M");
        self.emit("@R14");
        self.emit("M=D+M");     // R14 += idx
        self.emit("@R15");
        self.emit("M=M-1");
        self.emit(&format!("@{}", l_loop));
        self.emit("0;JMP");
        self.emit(&format!("({})", l_done));
        self.emit("@R14");
        self.emit("D=M");       // D = idx * stride
    }

    // ── Load/Store variables ─────────────────────────────────────────────

    /// Push the VALUE of a variable onto the stack
    fn load_var(&mut self, info: &VarInfo) {
        match &info.storage {
            VarStorage::Local(idx) => {
                let idx = *idx;
                if idx == 0 {
                    self.emit("@LCL");
                    self.emit("A=M");
                    self.emit("D=M");
                } else {
                    self.emit("@LCL");
                    self.emit("D=M");
                    self.emit(&format!("@{}", idx));
                    self.emit("A=D+A");
                    self.emit("D=M");
                }
                self.push_d();
            }
            VarStorage::Param(idx) => {
                let idx = *idx;
                if idx == 0 {
                    self.emit("@ARG");
                    self.emit("A=M");
                    self.emit("D=M");
                } else {
                    self.emit("@ARG");
                    self.emit("D=M");
                    self.emit(&format!("@{}", idx));
                    self.emit("A=D+A");
                    self.emit("D=M");
                }
                self.push_d();
            }
            VarStorage::Global(addr) => {
                self.emit(&format!("@{}", addr));
                self.emit("D=M");
                self.push_d();
            }
        }
    }

    /// Push the ADDRESS of a variable onto the stack (for lvalue operations)
    fn addr_of_var(&mut self, info: &VarInfo) {
        match &info.storage {
            VarStorage::Local(idx) => {
                let idx = *idx;
                if idx == 0 {
                    self.emit("@LCL");
                    self.emit("D=M");
                } else {
                    self.emit("@LCL");
                    self.emit("D=M");
                    self.emit(&format!("@{}", idx));
                    self.emit("D=D+A");
                }
                self.push_d();
            }
            VarStorage::Param(idx) => {
                let idx = *idx;
                if idx == 0 {
                    self.emit("@ARG");
                    self.emit("D=M");
                } else {
                    self.emit("@ARG");
                    self.emit("D=M");
                    self.emit(&format!("@{}", idx));
                    self.emit("D=D+A");
                }
                self.push_d();
            }
            VarStorage::Global(addr) => {
                self.emit(&format!("@{}", addr));
                self.emit("D=A");
                self.push_d();
            }
        }
    }

    // ── Struct helpers ───────────────────────────────────────────────────

    /// Compute the size (in Hack words) of a type, resolving structs via self.struct_defs.
    fn type_size(&self, ty: &Type) -> usize {
        type_size(ty, &self.struct_defs)
    }

    /// Compute the byte offset of a named field within a named struct.
    fn field_offset(&self, struct_name: &str, field_name: &str) -> Result<usize, CodegenError> {
        let fields = self.struct_defs.get(struct_name).ok_or_else(|| {
            CodegenError::new(format!("unknown struct '{}'", struct_name))
        })?;
        let mut offset = 0usize;
        for (fname, fty) in fields {
            if fname == field_name {
                return Ok(offset);
            }
            offset += self.type_size(fty);
        }
        Err(CodegenError::new(format!("struct '{}' has no field '{}'", struct_name, field_name)))
    }

    /// Infer the type of an expression without generating code.
    fn expr_type(&self, expr: &Expr, vars: &HashMap<String, VarInfo>) -> Option<Type> {
        match expr {
            Expr::Num(_) => Some(Type::Int),
            Expr::StringLit(_) => Some(Type::Ptr(Box::new(Type::Char))),
            Expr::Sizeof(_) => Some(Type::Int),
            Expr::Ident(name) => vars.get(name).map(|v| v.ty.clone()),
            Expr::UnOp(UnOp::Deref, inner) => match self.expr_type(inner, vars)? {
                Type::Ptr(t) => Some(*t),
                Type::Array(t, _) => Some(*t),
                _ => None,
            },
            Expr::UnOp(UnOp::Addr, inner) => {
                self.expr_type(inner, vars).map(|t| Type::Ptr(Box::new(t)))
            }
            Expr::Member(base, field) => {
                if let Some(Type::Struct(sname)) = self.expr_type(base, vars) {
                    self.struct_defs.get(&sname)
                        .and_then(|fields| fields.iter().find(|(fn_, _)| fn_ == field))
                        .map(|(_, ty)| ty.clone())
                } else {
                    None
                }
            }
            Expr::Index(base, _) => match self.expr_type(base, vars)? {
                Type::Ptr(t) => Some(*t),
                Type::Array(t, _) => Some(*t),
                _ => None,
            },
            _ => None,
        }
    }

    // ── Expression codegen ───────────────────────────────────────────────

    /// Compile expr; leaves one value on top of stack.
    fn gen_expr(
        &mut self,
        expr: &Expr,
        vars: &HashMap<String, VarInfo>,
    ) -> Result<(), CodegenError> {
        match expr {
            Expr::Num(n) => {
                let n = *n;
                if n == 0 {
                    self.emit("D=0");
                } else if n == 1 {
                    self.emit("D=1");
                } else if n == -1 {
                    self.emit("D=-1");
                } else if n > 0 {
                    self.emit(&format!("@{}", n));
                    self.emit("D=A");
                } else {
                    // negative: load abs then negate
                    self.emit(&format!("@{}", -n));
                    self.emit("D=-A");
                }
                self.push_d();
            }

            Expr::Sizeof(ty) => {
                let sz = self.type_size(ty).max(1) as i32;
                self.emit(&format!("@{}", sz));
                self.emit("D=A");
                self.push_d();
            }

            Expr::StringLit(s) => {
                let addr = *self.string_map.get(s).ok_or_else(|| {
                    CodegenError::new(format!("unknown string literal {:?}", s))
                })?;
                self.emit(&format!("@{}", addr));
                self.emit("D=A");
                self.push_d();
            }

            Expr::Ident(name) => {
                let info = vars.get(name).ok_or_else(|| {
                    CodegenError::new(format!("undefined variable '{}'", name))
                })?.clone();
                // Arrays decay to a pointer to their first element (C semantics).
                if matches!(info.ty, crate::parser::Type::Array(..)) {
                    self.addr_of_var(&info);
                } else {
                    self.load_var(&info);
                }
            }

            Expr::UnOp(op, inner) => {
                match op {
                    UnOp::Addr => {
                        // push address of inner lvalue
                        self.gen_addr(inner, vars)?;
                    }
                    UnOp::Deref => {
                        self.gen_expr(inner, vars)?;
                        // pop address, read memory at that address
                        self.pop_d();
                        self.emit("A=D");
                        self.emit("D=M");
                        self.push_d();
                    }
                    UnOp::Neg => {
                        self.gen_expr(inner, vars)?;
                        self.pop_d();
                        self.emit("D=-D");
                        self.push_d();
                    }
                    UnOp::Not => {
                        self.gen_expr(inner, vars)?;
                        // !x: if x==0 push 1, else push 0
                        self.pop_d();
                        let id = self.label();
                        let lfalse = format!("__not_f_{}", id);
                        let lend   = format!("__not_e_{}", id);
                        self.emit(&format!("@{}", lfalse));
                        self.emit("D;JNE");
                        // x was 0 => result 1
                        self.emit("D=1");
                        self.emit(&format!("@{}", lend));
                        self.emit("0;JMP");
                        self.emit(&format!("({})", lfalse));
                        self.emit("D=0");
                        self.emit(&format!("({})", lend));
                        self.push_d();
                    }
                    UnOp::BitNot => {
                        self.gen_expr(inner, vars)?;
                        self.pop_d();
                        self.emit("D=!D");
                        self.push_d();
                    }
                }
            }

            Expr::BinOp(op, lhs, rhs) => {
                self.gen_binop(op, lhs, rhs, vars)?;
            }

            Expr::Call(name, args) => {
                self.gen_call(name, args, vars)?;
            }

            Expr::Index(base, idx) => {
                let base_ty = self.expr_type(base, vars);
                let stride = match &base_ty {
                    Some(Type::Array(elem_ty, _)) | Some(Type::Ptr(elem_ty)) => {
                        self.type_size(elem_ty).max(1)
                    }
                    _ => 1,
                };
                let elem_is_array = matches!(
                    &base_ty,
                    Some(Type::Array(e, _)) if matches!(e.as_ref(), Type::Array(_, _))
                );

                self.gen_expr(base, vars)?;
                self.gen_expr(idx, vars)?;

                self.pop_d();   // D = idx
                if stride == 1 {
                    self.emit("@R14");
                    self.emit("M=D");
                    self.pop_d();
                    self.emit("@R14");
                    self.emit("D=D+M");   // D = base + idx
                } else {
                    self.emit_stride_mul(stride);
                    self.emit("@R14");
                    self.emit("M=D");   // save idx*stride
                    self.pop_d();       // D = base
                    self.emit("@R14");
                    self.emit("D=D+M"); // D = base + idx*stride
                }

                if elem_is_array {
                    self.push_d();
                } else {
                    self.emit("A=D");
                    self.emit("D=M");   // D = value at address
                    self.push_d();
                }
            }

            Expr::Member(_, _) => {
                // Load value at field address: gen_addr gives the address, then deref
                self.gen_addr(expr, vars)?;
                self.pop_d();
                self.emit("A=D");
                self.emit("D=M");
                self.push_d();
            }

            Expr::Ternary(cond, then_e, else_e) => {
                let id = self.label();
                let l_false = format!("__tern_f_{}", id);
                let l_end   = format!("__tern_e_{}", id);
                self.gen_expr(cond, vars)?;
                self.pop_d();
                self.emit(&format!("@{}", l_false));
                self.emit("D;JEQ");
                self.gen_expr(then_e, vars)?;
                self.emit(&format!("@{}", l_end));
                self.emit("0;JMP");
                self.emit(&format!("({})", l_false));
                self.gen_expr(else_e, vars)?;
                self.emit(&format!("({})", l_end));
            }

            Expr::PostInc(inner) => {
                self.gen_addr(inner, vars)?;
                self.pop_d();
                self.emit("@R13");
                self.emit("M=D");       // R13 = address
                self.emit("@R13");
                self.emit("A=M");       // A = address
                self.emit("D=M");       // D = old value
                self.push_d();          // push old value (expression result)
                self.emit("@R13");
                self.emit("A=M");       // A = address
                self.emit("M=M+1");     // increment in place
            }
            Expr::PostDec(inner) => {
                self.gen_addr(inner, vars)?;
                self.pop_d();
                self.emit("@R13");
                self.emit("M=D");       // R13 = address
                self.emit("@R13");
                self.emit("A=M");       // A = address
                self.emit("D=M");       // D = old value
                self.push_d();          // push old value (expression result)
                self.emit("@R13");
                self.emit("A=M");       // A = address
                self.emit("M=M-1");     // decrement in place
            }
            Expr::Cast(ty, inner) => {
                self.gen_expr(inner, vars)?;
                if matches!(ty, Type::Char) {
                    self.pop_d();
                    self.emit("@255");
                    self.emit("D=D&A");
                    self.push_d();
                }
                // All other casts: no-op
            }
            Expr::InitList(items) => {
                if let Some(first) = items.first() {
                    self.gen_expr(first, vars)?;
                } else {
                    self.emit("D=0");
                    self.push_d();
                }
            }
        }
        Ok(())
    }

    /// Push the address of an lvalue expression.
    fn gen_addr(
        &mut self,
        expr: &Expr,
        vars: &HashMap<String, VarInfo>,
    ) -> Result<(), CodegenError> {
        match expr {
            Expr::Ident(name) => {
                let info = vars.get(name).ok_or_else(|| {
                    CodegenError::new(format!("undefined variable '{}'", name))
                })?.clone();
                self.addr_of_var(&info);
            }
            Expr::UnOp(UnOp::Deref, inner) => {
                // &(*p) = p
                self.gen_expr(inner, vars)?;
            }
            Expr::Index(base, idx) => {
                let base_ty = self.expr_type(base, vars);
                let stride = match &base_ty {
                    Some(Type::Array(elem_ty, _)) | Some(Type::Ptr(elem_ty)) => {
                        self.type_size(elem_ty).max(1)
                    }
                    _ => 1,
                };
                self.gen_expr(base, vars)?;
                self.gen_expr(idx, vars)?;
                self.pop_d();   // D = idx
                if stride == 1 {
                    self.emit("@R14");
                    self.emit("M=D");
                    self.pop_d();       // D = base
                    self.emit("@R14");
                    self.emit("D=D+M");
                } else {
                    self.emit_stride_mul(stride);
                    self.emit("@R14");
                    self.emit("M=D");   // save idx*stride
                    self.pop_d();       // D = base
                    self.emit("@R14");
                    self.emit("D=D+M");
                }
                self.push_d();
            }
            Expr::Member(base, field) => {
                // &(expr.field) = &base + field_offset
                let base_ty = self.expr_type(base, vars)
                    .ok_or_else(|| CodegenError::new("cannot determine type for member access"))?;
                let struct_name = match &base_ty {
                    Type::Struct(name) => name.clone(),
                    _ => return Err(CodegenError::new(
                        format!("member access on non-struct type {:?}", base_ty)
                    )),
                };
                let offset = self.field_offset(&struct_name, field)?;
                self.gen_addr(base, vars)?; // pushes address of base (struct start)
                if offset > 0 {
                    self.pop_d();
                    self.emit(&format!("@{}", offset));
                    self.emit("D=D+A");
                    self.push_d();
                }
            }
            _ => return Err(CodegenError::new(format!("not an lvalue: {:?}", expr))),
        }
        Ok(())
    }

    fn gen_binop(
        &mut self,
        op: &BinOp,
        lhs: &Expr,
        rhs: &Expr,
        vars: &HashMap<String, VarInfo>,
    ) -> Result<(), CodegenError> {
        // Assignment
        if let BinOp::Assign = op {
            return self.gen_assign(lhs, rhs, vars);
        }
        // Compound assignment: desugar
        if let BinOp::AddAssign | BinOp::SubAssign = op {
            let arith_op = match op {
                BinOp::AddAssign => BinOp::Add,
                BinOp::SubAssign => BinOp::Sub,
                _ => unreachable!(),
            };
            // lhs = lhs op rhs
            let new_rhs = Expr::BinOp(arith_op, Box::new(lhs.clone()), Box::new(rhs.clone()));
            return self.gen_assign(lhs, &new_rhs, vars);
        }
        // New compound assignments
        if matches!(op, BinOp::MulAssign | BinOp::DivAssign | BinOp::ModAssign
            | BinOp::AndAssign | BinOp::OrAssign | BinOp::XorAssign
            | BinOp::ShlAssign | BinOp::ShrAssign) {
            let arith_op = match op {
                BinOp::MulAssign => BinOp::Mul,
                BinOp::DivAssign => BinOp::Div,
                BinOp::ModAssign => BinOp::Mod,
                BinOp::AndAssign => BinOp::BitAnd,
                BinOp::OrAssign  => BinOp::BitOr,
                BinOp::XorAssign => BinOp::BitXor,
                BinOp::ShlAssign => BinOp::Shl,
                BinOp::ShrAssign => BinOp::Shr,
                _ => unreachable!(),
            };
            let new_rhs = Expr::BinOp(arith_op, Box::new(lhs.clone()), Box::new(rhs.clone()));
            return self.gen_assign(lhs, &new_rhs, vars);
        }
        // Short-circuit logical AND
        if let BinOp::And = op {
            return self.gen_and(lhs, rhs, vars);
        }
        // Short-circuit logical OR
        if let BinOp::Or = op {
            return self.gen_or(lhs, rhs, vars);
        }

        // Evaluate both operands
        self.gen_expr(lhs, vars)?;
        self.gen_expr(rhs, vars)?;

        // pop rhs into R14, lhs into D (R13 for mul/div)
        self.pop_d();
        self.emit("@R14");
        self.emit("M=D"); // R14 = rhs
        self.pop_d();     // D = lhs

        match op {
            BinOp::Add => {
                self.emit("@R14");
                self.emit("D=D+M");
                self.push_d();
            }
            BinOp::Sub => {
                self.emit("@R14");
                self.emit("D=D-M");
                self.push_d();
            }
            BinOp::Mul => {
                // R13 = lhs (D), R14 already set
                self.emit("@R13");
                self.emit("M=D");
                let id = self.label();
                let ret_lbl = format!("__mul_ret_{}", id);
                self.emit(&format!("@{}", ret_lbl));
                self.emit("D=A");
                self.emit("@R3");
                self.emit("M=D");
                self.emit("@__mul");
                self.emit("0;JMP");
                self.emit(&format!("({})", ret_lbl));
                // result in R13
                self.emit("@R13");
                self.emit("D=M");
                self.push_d();
            }
            BinOp::Div => {
                self.emit("@R13");
                self.emit("M=D"); // R13 = lhs
                let id = self.label();
                let ret_lbl = format!("__div_ret_{}", id);
                self.emit(&format!("@{}", ret_lbl));
                self.emit("D=A");
                self.emit("@R3");
                self.emit("M=D");
                self.emit("@__div");
                self.emit("0;JMP");
                self.emit(&format!("({})", ret_lbl));
                self.emit("@R13");
                self.emit("D=M");
                self.push_d();
            }
            BinOp::Mod => {
                self.emit("@R13");
                self.emit("M=D");
                let id = self.label();
                let ret_lbl = format!("__mod_ret_{}", id);
                self.emit(&format!("@{}", ret_lbl));
                self.emit("D=A");
                self.emit("@R3");
                self.emit("M=D");
                self.emit("@__div");
                self.emit("0;JMP");
                self.emit(&format!("({})", ret_lbl));
                // remainder in R15
                self.emit("@R15");
                self.emit("D=M");
                self.push_d();
            }
            BinOp::BitAnd => {
                self.emit("@R14");
                self.emit("D=D&M");
                self.push_d();
            }
            BinOp::BitOr => {
                self.emit("@R14");
                self.emit("D=D|M");
                self.push_d();
            }
            BinOp::BitXor => {
                // D = lhs, R14 = rhs
                // XOR = (lhs | rhs) & ~(lhs & rhs)
                // Use R13 (lhs copy), R15 (~(lhs & rhs))
                self.emit("@R13");
                self.emit("M=D");       // R13 = lhs
                self.emit("@R14");
                self.emit("D=D&M");     // D = lhs & rhs
                self.emit("D=!D");      // D = ~(lhs & rhs)
                self.emit("@R15");
                self.emit("M=D");       // R15 = ~(lhs & rhs)
                self.emit("@R13");
                self.emit("D=M");       // D = lhs
                self.emit("@R14");
                self.emit("D=D|M");     // D = lhs | rhs
                self.emit("@R15");
                self.emit("D=D&M");     // D = XOR
                self.push_d();
            }
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                self.gen_cmp(op)?;
            }
            BinOp::Shl => {
                self.emit("@R13");
                self.emit("M=D");       // R13 = value (lhs), R14 = shift amount (rhs)
                let id = self.label();
                let l_loop = format!("__shl_loop_{}", id);
                let l_end  = format!("__shl_end_{}", id);
                self.emit(&format!("({})", l_loop));
                self.emit("@R14");
                self.emit("D=M");
                self.emit(&format!("@{}", l_end));
                self.emit("D;JEQ");
                self.emit("@R13");
                self.emit("D=M");
                self.emit("M=D+M");     // R13 = R13 * 2
                self.emit("@R14");
                self.emit("M=M-1");
                self.emit(&format!("@{}", l_loop));
                self.emit("0;JMP");
                self.emit(&format!("({})", l_end));
                self.emit("@R13");
                self.emit("D=M");
                self.push_d();
            }
            BinOp::Shr => {
                self.emit("@R13");
                self.emit("M=D");       // R13 = dividend (lhs)
                // R14 already has n (rhs)
                let id = self.label();
                let l_pow_loop = format!("__shr_pow_{}", id);
                let l_pow_end  = format!("__shr_pow_end_{}", id);
                self.emit("@R14");
                self.emit("D=M");       // D = n
                self.emit("@R15");
                self.emit("M=D");       // R15 = n
                self.emit("@R14");
                self.emit("M=1");       // R14 = 1 (will become 2^n)
                self.emit(&format!("({})", l_pow_loop));
                self.emit("@R15");
                self.emit("D=M");
                self.emit(&format!("@{}", l_pow_end));
                self.emit("D;JEQ");
                self.emit("@R14");
                self.emit("D=M");
                self.emit("M=D+M");     // R14 *= 2
                self.emit("@R15");
                self.emit("M=M-1");
                self.emit(&format!("@{}", l_pow_loop));
                self.emit("0;JMP");
                self.emit(&format!("({})", l_pow_end));
                // Now R13 = dividend, R14 = 2^n, call __div
                let ret_lbl = format!("__shr_div_ret_{}", id);
                self.emit(&format!("@{}", ret_lbl));
                self.emit("D=A");
                self.emit("@R3");
                self.emit("M=D");
                self.emit("@__div");
                self.emit("0;JMP");
                self.emit(&format!("({})", ret_lbl));
                self.emit("@R13");
                self.emit("D=M");
                self.push_d();
            }
            BinOp::Assign | BinOp::AddAssign | BinOp::SubAssign
            | BinOp::MulAssign | BinOp::DivAssign | BinOp::ModAssign
            | BinOp::AndAssign | BinOp::OrAssign | BinOp::XorAssign
            | BinOp::ShlAssign | BinOp::ShrAssign
            | BinOp::And | BinOp::Or => unreachable!(),
        }
        Ok(())
    }

    /// Generate comparison; D = lhs, R14 = rhs already set.
    fn gen_cmp(&mut self, op: &BinOp) -> Result<(), CodegenError> {
        let id = self.label();
        let l_true = format!("__cmp_t_{}", id);
        let l_end  = format!("__cmp_e_{}", id);
        // D = lhs - rhs (or rhs - lhs for Gt/Ge)
        match op {
            BinOp::Gt | BinOp::Ge => {
                // swap: D = rhs - lhs => treat as Lt/Le
                self.emit("@R14");
                self.emit("D=M-D"); // D = rhs - lhs
            }
            _ => {
                self.emit("@R14");
                self.emit("D=D-M"); // D = lhs - rhs
            }
        }
        let jump = match op {
            BinOp::Eq => "JEQ",
            BinOp::Ne => "JNE",
            BinOp::Lt => "JLT",
            BinOp::Le => "JLE",
            BinOp::Gt => "JLT", // we swapped
            BinOp::Ge => "JLE", // we swapped
            _ => unreachable!(),
        };
        self.emit(&format!("@{}", l_true));
        self.emit(&format!("D;{}", jump));
        // false
        self.emit("D=0");
        self.emit(&format!("@{}", l_end));
        self.emit("0;JMP");
        self.emit(&format!("({})", l_true));
        self.emit("D=1");
        self.emit(&format!("({})", l_end));
        self.push_d();
        Ok(())
    }

    fn gen_and(&mut self, lhs: &Expr, rhs: &Expr, vars: &HashMap<String, VarInfo>) -> Result<(), CodegenError> {
        let id = self.label();
        let l_false = format!("__and_f_{}", id);
        let l_end   = format!("__and_e_{}", id);
        self.gen_expr(lhs, vars)?;
        self.pop_d();
        self.emit(&format!("@{}", l_false));
        self.emit("D;JEQ");
        self.gen_expr(rhs, vars)?;
        self.pop_d();
        self.emit(&format!("@{}", l_false));
        self.emit("D;JEQ");
        self.emit("D=1");
        self.emit(&format!("@{}", l_end));
        self.emit("0;JMP");
        self.emit(&format!("({})", l_false));
        self.emit("D=0");
        self.emit(&format!("({})", l_end));
        self.push_d();
        Ok(())
    }

    fn gen_or(&mut self, lhs: &Expr, rhs: &Expr, vars: &HashMap<String, VarInfo>) -> Result<(), CodegenError> {
        let id = self.label();
        let l_true = format!("__or_t_{}", id);
        let l_end  = format!("__or_e_{}", id);
        self.gen_expr(lhs, vars)?;
        self.pop_d();
        self.emit(&format!("@{}", l_true));
        self.emit("D;JNE");
        self.gen_expr(rhs, vars)?;
        self.pop_d();
        self.emit(&format!("@{}", l_true));
        self.emit("D;JNE");
        self.emit("D=0");
        self.emit(&format!("@{}", l_end));
        self.emit("0;JMP");
        self.emit(&format!("({})", l_true));
        self.emit("D=1");
        self.emit(&format!("({})", l_end));
        self.push_d();
        Ok(())
    }

    /// Store R13's value into a variable directly (no stack intermediary).
    fn store_var_from_r13(&mut self, info: &VarInfo) {
        match &info.storage {
            VarStorage::Local(idx) => {
                let idx = *idx;
                // Compute target address into R14, then store from R13
                if idx == 0 {
                    self.emit("@LCL");
                    self.emit("D=M");       // D = LCL base
                } else {
                    self.emit("@LCL");
                    self.emit("D=M");
                    self.emit(&format!("@{}", idx));
                    self.emit("D=D+A");     // D = LCL + idx
                }
                self.emit("@R14");
                self.emit("M=D");           // R14 = target address
                self.emit("@R13");
                self.emit("D=M");           // D = value
                self.emit("@R14");
                self.emit("A=M");
                self.emit("M=D");
            }
            VarStorage::Param(idx) => {
                let idx = *idx;
                if idx == 0 {
                    self.emit("@ARG");
                    self.emit("D=M");
                } else {
                    self.emit("@ARG");
                    self.emit("D=M");
                    self.emit(&format!("@{}", idx));
                    self.emit("D=D+A");
                }
                self.emit("@R14");
                self.emit("M=D");
                self.emit("@R13");
                self.emit("D=M");
                self.emit("@R14");
                self.emit("A=M");
                self.emit("M=D");
            }
            VarStorage::Global(addr) => {
                // save target address in D, then store
                self.emit(&format!("@{}", addr));
                self.emit("D=A");           // D = global address
                self.emit("@R14");
                self.emit("M=D");           // R14 = global address (R13 has value)
                self.emit("@R13");
                self.emit("D=M");           // D = value
                self.emit("@R14");
                self.emit("A=M");           // A = global address
                self.emit("M=D");           // RAM[addr] = value
            }
        }
    }

    /// Generate assignment: lhs = rhs. Leaves assigned value on stack.
    /// Evaluates RHS first so function calls don't corrupt the call convention.
    fn gen_assign(
        &mut self,
        lhs: &Expr,
        rhs: &Expr,
        vars: &HashMap<String, VarInfo>,
    ) -> Result<(), CodegenError> {
        // 1. Evaluate rhs first (important: this must happen before we compute any addresses)
        self.gen_expr(rhs, vars)?;
        // 2. Pop value into R13
        self.pop_d();
        self.emit("@R13");
        self.emit("M=D");
        // 3. Store R13 to lhs, computing lhs address inline (no stack push)
        match lhs {
            Expr::Ident(name) => {
                let info = vars.get(name).ok_or_else(|| {
                    CodegenError::new(format!("undefined variable '{}'", name))
                })?.clone();
                self.store_var_from_r13(&info);
            }
            Expr::UnOp(UnOp::Deref, ptr_expr) => {
                // compute pointer value, then store through it
                self.gen_expr(ptr_expr, vars)?;
                self.pop_d();          // D = pointer address
                self.emit("@R14");
                self.emit("M=D");      // R14 = target address (@R13 would overwrite A)
                self.emit("@R13");
                self.emit("D=M");      // D = value
                self.emit("@R14");
                self.emit("A=M");      // A = target address
                self.emit("M=D");
            }
            Expr::Index(base, idx) => {
                // address = base + idx  (use R14 for idx, R13 already has value)
                self.gen_expr(base, vars)?;
                self.gen_expr(idx, vars)?;
                self.pop_d();
                self.emit("@R14");
                self.emit("M=D");       // R14 = idx
                self.pop_d();           // D = base
                self.emit("@R14");
                self.emit("D=D+M");     // D = base + idx (address)
                self.emit("@R14");
                self.emit("M=D");       // R14 = address (save before @R13 overwrites A)
                self.emit("@R13");
                self.emit("D=M");       // D = value
                self.emit("@R14");
                self.emit("A=M");       // A = address
                self.emit("M=D");
            }
            Expr::Member(base, field) => {
                // Compute field address via gen_addr, then store R13 there
                let base_ty = self.expr_type(base, vars)
                    .ok_or_else(|| CodegenError::new("cannot determine type for member assignment"))?;
                let struct_name = match &base_ty {
                    Type::Struct(name) => name.clone(),
                    _ => return Err(CodegenError::new(
                        format!("member access on non-struct type {:?}", base_ty)
                    )),
                };
                let offset = self.field_offset(&struct_name, field)?;
                self.gen_addr(base, vars)?; // pushes base address
                self.pop_d();               // D = base address
                if offset > 0 {
                    self.emit(&format!("@{}", offset));
                    self.emit("D=D+A");     // D = field address
                }
                self.emit("@R14");
                self.emit("M=D");           // R14 = field address (save before @R13 overwrites A)
                self.emit("@R13");
                self.emit("D=M");           // D = value
                self.emit("@R14");
                self.emit("A=M");           // A = field address
                self.emit("M=D");           // store
            }
            _ => return Err(CodegenError::new(format!("not a valid lvalue: {:?}", lhs))),
        }
        // 4. Push value as result of assignment expression
        self.emit("@R13");
        self.emit("D=M");
        self.push_d();
        Ok(())
    }

    fn gen_call(
        &mut self,
        name: &str,
        args: &[Expr],
        vars: &HashMap<String, VarInfo>,
    ) -> Result<(), CodegenError> {
        // ── Built-in functions ────────────────────────────────────────────
        match name {
            "putchar" => {
                // putchar(c) -> write char to output port, return the char
                if args.len() != 1 {
                    return Err(CodegenError::new("putchar expects 1 argument"));
                }
                self.gen_expr(&args[0], vars)?;
                self.pop_d();
                self.emit(&format!("@{}", HACK_OUTPUT_PORT));
                self.emit("M=D");
                self.push_d(); // return value = char written
                return Ok(());
            }
            "puts" => {
                // puts(s) -> print null-terminated string + newline, return 0
                if args.len() != 1 {
                    return Err(CodegenError::new("puts expects 1 argument"));
                }
                self.gen_expr(&args[0], vars)?;
                self.pop_d();
                self.emit("@R13");
                self.emit("M=D");
                let id = self.label();
                let ret_lbl = format!("__puts_ret_{}", id);
                self.emit(&format!("@{}", ret_lbl));
                self.emit("D=A");
                self.emit("@R3");
                self.emit("M=D");
                self.emit("@__puts");
                self.emit("0;JMP");
                self.emit(&format!("({})", ret_lbl));
                self.emit("D=0");
                self.push_d();
                return Ok(());
            }
            "strlen" => {
                // strlen(s) -> count chars until null, return length
                if args.len() != 1 {
                    return Err(CodegenError::new("strlen expects 1 argument"));
                }
                self.gen_expr(&args[0], vars)?;
                self.pop_d();
                self.emit("@R13");
                self.emit("M=D");
                let id = self.label();
                let ret_lbl = format!("__strlen_ret_{}", id);
                self.emit(&format!("@{}", ret_lbl));
                self.emit("D=A");
                self.emit("@R3");
                self.emit("M=D");
                self.emit("@__strlen");
                self.emit("0;JMP");
                self.emit(&format!("({})", ret_lbl));
                self.emit("@R13");
                self.emit("D=M");
                self.push_d();
                return Ok(());
            }
            "draw_pixel" => {
                // draw_pixel(x, y) -> set pixel (x,y) black, return 0
                if args.len() != 2 {
                    return Err(CodegenError::new("draw_pixel expects 2 arguments"));
                }
                self.gen_expr(&args[0], vars)?;
                self.pop_d();
                self.emit("@R13");
                self.emit("M=D"); // R13 = x
                self.gen_expr(&args[1], vars)?;
                self.pop_d();
                self.emit("@R14");
                self.emit("M=D"); // R14 = y
                let id = self.label();
                let ret_lbl = format!("__draw_pixel_ret_{}", id);
                self.emit(&format!("@{}", ret_lbl));
                self.emit("D=A");
                self.emit("@R3");
                self.emit("M=D");
                self.emit("@__draw_pixel");
                self.emit("0;JMP");
                self.emit(&format!("({})", ret_lbl));
                self.emit("D=0");
                self.push_d();
                return Ok(());
            }
            "clear_pixel" => {
                // clear_pixel(x, y) -> set pixel (x,y) white, return 0
                if args.len() != 2 {
                    return Err(CodegenError::new("clear_pixel expects 2 arguments"));
                }
                self.gen_expr(&args[0], vars)?;
                self.pop_d();
                self.emit("@R13");
                self.emit("M=D"); // R13 = x
                self.gen_expr(&args[1], vars)?;
                self.pop_d();
                self.emit("@R14");
                self.emit("M=D"); // R14 = y
                let id = self.label();
                let ret_lbl = format!("__clear_pixel_ret_{}", id);
                self.emit(&format!("@{}", ret_lbl));
                self.emit("D=A");
                self.emit("@R3");
                self.emit("M=D");
                self.emit("@__clear_pixel");
                self.emit("0;JMP");
                self.emit(&format!("({})", ret_lbl));
                self.emit("D=0");
                self.push_d();
                return Ok(());
            }
            "fill_screen" => {
                // fill_screen() -> set all pixels black, return 0
                if !args.is_empty() {
                    return Err(CodegenError::new("fill_screen expects 0 arguments"));
                }
                let id = self.label();
                let ret_lbl = format!("__fill_screen_ret_{}", id);
                self.emit(&format!("@{}", ret_lbl));
                self.emit("D=A");
                self.emit("@R3");
                self.emit("M=D");
                self.emit("@__fill_screen");
                self.emit("0;JMP");
                self.emit(&format!("({})", ret_lbl));
                self.emit("D=0");
                self.push_d();
                return Ok(());
            }
            "clear_screen" => {
                // clear_screen() -> set all pixels white, return 0
                if !args.is_empty() {
                    return Err(CodegenError::new("clear_screen expects 0 arguments"));
                }
                let id = self.label();
                let ret_lbl = format!("__clear_screen_ret_{}", id);
                self.emit(&format!("@{}", ret_lbl));
                self.emit("D=A");
                self.emit("@R3");
                self.emit("M=D");
                self.emit("@__clear_screen");
                self.emit("0;JMP");
                self.emit(&format!("({})", ret_lbl));
                self.emit("D=0");
                self.push_d();
                return Ok(());
            }
            "draw_char" => {
                // draw_char(col, row, c) -> draw char c at text cell (col 0-63, row 0-31), return 0
                if args.len() != 3 {
                    return Err(CodegenError::new("draw_char expects 3 arguments"));
                }
                self.gen_expr(&args[0], vars)?;
                self.pop_d();
                self.emit("@R13");
                self.emit("M=D"); // R13 = col
                self.gen_expr(&args[1], vars)?;
                self.pop_d();
                self.emit("@R14");
                self.emit("M=D"); // R14 = row
                self.gen_expr(&args[2], vars)?;
                self.pop_d();
                self.emit("@R15");
                self.emit("M=D"); // R15 = char_code
                let id = self.label();
                let ret_lbl = format!("__draw_char_ret_{}", id);
                self.emit(&format!("@{}", ret_lbl));
                self.emit("D=A");
                self.emit("@R3");
                self.emit("M=D");
                self.emit("@__draw_char");
                self.emit("0;JMP");
                self.emit(&format!("({})", ret_lbl));
                self.emit("D=0");
                self.push_d();
                return Ok(());
            }
            "draw_string" | "print_at" => {
                // draw_string(col, row, str) / print_at(col, row, str)
                // Draw null-terminated string starting at text cell (col, row), return 0
                if args.len() != 3 {
                    return Err(CodegenError::new(format!("{} expects 3 arguments", name)));
                }
                self.gen_expr(&args[0], vars)?;
                self.pop_d();
                self.emit("@R13");
                self.emit("M=D"); // R13 = col
                self.gen_expr(&args[1], vars)?;
                self.pop_d();
                self.emit("@R14");
                self.emit("M=D"); // R14 = row
                self.gen_expr(&args[2], vars)?;
                self.pop_d();
                self.emit("@R15");
                self.emit("M=D"); // R15 = str_ptr
                let id = self.label();
                let ret_lbl = format!("__draw_string_ret_{}", id);
                self.emit(&format!("@{}", ret_lbl));
                self.emit("D=A");
                self.emit("@R3");
                self.emit("M=D");
                self.emit("@__draw_string");
                self.emit("0;JMP");
                self.emit(&format!("({})", ret_lbl));
                self.emit("D=0");
                self.push_d();
                return Ok(());
            }
            // ── abs(x) — inline: if x < 0, negate it ────────────────────────
            "abs" => {
                if args.len() != 1 {
                    return Err(CodegenError::new("abs expects 1 argument"));
                }
                self.gen_expr(&args[0], vars)?;
                self.pop_d();
                let id = self.label();
                let done = format!("__abs_done_{}", id);
                self.emit(&format!("@{}", done));
                self.emit("D;JGE");
                self.emit("D=-D");
                self.emit(&format!("({})", done));
                self.push_d();
                return Ok(());
            }
            // ── min(a, b) — inline ──────────────────────────────────────────
            "min" => {
                if args.len() != 2 {
                    return Err(CodegenError::new("min expects 2 arguments"));
                }
                self.gen_expr(&args[0], vars)?;
                self.gen_expr(&args[1], vars)?;
                self.pop_d();           // D = b
                self.emit("@R13");
                self.emit("M=D");       // R13 = b
                self.pop_d();           // D = a
                let id = self.label();
                let use_b = format!("__min_b_{}", id);
                let done  = format!("__min_done_{}", id);
                self.emit("@R13");
                self.emit("D=D-M");     // D = a - b
                self.emit(&format!("@{}", use_b));
                self.emit("D;JGT");     // a - b > 0  =>  a > b  =>  use b
                // a <= b: use a (restore a = (a-b)+b = (a-b)+R13)
                self.emit("@R13");
                self.emit("D=D+M");     // D = a
                self.emit(&format!("@{}", done));
                self.emit("0;JMP");
                self.emit(&format!("({})", use_b));
                self.emit("@R13");
                self.emit("D=M");       // D = b
                self.emit(&format!("({})", done));
                self.push_d();
                return Ok(());
            }
            // ── max(a, b) — inline ──────────────────────────────────────────
            "max" => {
                if args.len() != 2 {
                    return Err(CodegenError::new("max expects 2 arguments"));
                }
                self.gen_expr(&args[0], vars)?;
                self.gen_expr(&args[1], vars)?;
                self.pop_d();           // D = b
                self.emit("@R13");
                self.emit("M=D");       // R13 = b
                self.pop_d();           // D = a
                let id = self.label();
                let use_a = format!("__max_a_{}", id);
                let done  = format!("__max_done_{}", id);
                self.emit("@R13");
                self.emit("D=D-M");     // D = a - b
                self.emit(&format!("@{}", use_a));
                self.emit("D;JGT");     // a > b => use a
                // a <= b: use b
                self.emit("@R13");
                self.emit("D=M");
                self.emit(&format!("@{}", done));
                self.emit("0;JMP");
                self.emit(&format!("({})", use_a));
                // restore a = (a-b) + b
                self.emit("@R13");
                self.emit("D=D+M");
                self.emit(&format!("({})", done));
                self.push_d();
                return Ok(());
            }
            // ── read_key() — read Hack keyboard port non-blocking ───────────
            "read_key" => {
                if !args.is_empty() {
                    return Err(CodegenError::new("read_key expects 0 arguments"));
                }
                self.emit("@KBD");
                self.emit("D=M");
                self.push_d();
                return Ok(());
            }
            // ── strcpy(dst, src) — subroutine ───────────────────────────────
            "strcpy" => {
                if args.len() != 2 {
                    return Err(CodegenError::new("strcpy expects 2 arguments"));
                }
                self.used_builtins.insert(BuiltinKind::Strcpy);
                self.gen_expr(&args[0], vars)?;
                self.pop_d();
                self.emit("@R13");
                self.emit("M=D");   // R13 = dst
                self.gen_expr(&args[1], vars)?;
                self.pop_d();
                self.emit("@R14");
                self.emit("M=D");   // R14 = src
                let id = self.label();
                let ret_lbl = format!("__strcpy_ret_{}", id);
                self.emit(&format!("@{}", ret_lbl));
                self.emit("D=A");
                self.emit("@R3");
                self.emit("M=D");
                self.emit("@__strcpy");
                self.emit("0;JMP");
                self.emit(&format!("({})", ret_lbl));
                self.emit("@R13");
                self.emit("D=M");   // return original dst (R13 restored by subroutine)
                self.push_d();
                return Ok(());
            }
            // ── strcmp(a, b) — subroutine ────────────────────────────────────
            "strcmp" => {
                if args.len() != 2 {
                    return Err(CodegenError::new("strcmp expects 2 arguments"));
                }
                self.used_builtins.insert(BuiltinKind::Strcmp);
                self.gen_expr(&args[0], vars)?;
                self.pop_d();
                self.emit("@R13");
                self.emit("M=D");   // R13 = a
                self.gen_expr(&args[1], vars)?;
                self.pop_d();
                self.emit("@R14");
                self.emit("M=D");   // R14 = b
                let id = self.label();
                let ret_lbl = format!("__strcmp_ret_{}", id);
                self.emit(&format!("@{}", ret_lbl));
                self.emit("D=A");
                self.emit("@R3");
                self.emit("M=D");
                self.emit("@__strcmp");
                self.emit("0;JMP");
                self.emit(&format!("({})", ret_lbl));
                self.emit("@R13");
                self.emit("D=M");   // result in R13
                self.push_d();
                return Ok(());
            }
            // ── strcat(dst, src) — subroutine ───────────────────────────────
            "strcat" => {
                if args.len() != 2 {
                    return Err(CodegenError::new("strcat expects 2 arguments"));
                }
                self.used_builtins.insert(BuiltinKind::Strcat);
                self.gen_expr(&args[0], vars)?;
                self.pop_d();
                self.emit("@R13");
                self.emit("M=D");   // R13 = dst
                self.gen_expr(&args[1], vars)?;
                self.pop_d();
                self.emit("@R14");
                self.emit("M=D");   // R14 = src
                let id = self.label();
                let ret_lbl = format!("__strcat_ret_{}", id);
                self.emit(&format!("@{}", ret_lbl));
                self.emit("D=A");
                self.emit("@R3");
                self.emit("M=D");
                self.emit("@__strcat");
                self.emit("0;JMP");
                self.emit(&format!("({})", ret_lbl));
                self.emit("@R13");
                self.emit("D=M");   // return original dst
                self.push_d();
                return Ok(());
            }
            // ── itoa(n, buf) — subroutine ────────────────────────────────────
            "itoa" => {
                if args.len() != 2 {
                    return Err(CodegenError::new("itoa expects 2 arguments"));
                }
                self.used_builtins.insert(BuiltinKind::Itoa);
                self.used_builtins.insert(BuiltinKind::Div);
                self.gen_expr(&args[0], vars)?;
                self.pop_d();
                self.emit("@R13");
                self.emit("M=D");   // R13 = n
                self.gen_expr(&args[1], vars)?;
                self.pop_d();
                self.emit("@R14");
                self.emit("M=D");   // R14 = buf
                let id = self.label();
                let ret_lbl = format!("__itoa_ret_{}", id);
                self.emit(&format!("@{}", ret_lbl));
                self.emit("D=A");
                self.emit("@R3");
                self.emit("M=D");
                self.emit("@__itoa");
                self.emit("0;JMP");
                self.emit(&format!("({})", ret_lbl));
                self.emit("@R14");
                self.emit("D=M");   // return buf
                self.push_d();
                return Ok(());
            }
            _ => {}
        }

        let id = self.label();
        let ret_lbl = format!("{}$ret_{}", name, id);
        let n_args = args.len();

        // nand2tetris Jack VM calling convention:
        //   1. Push arguments first (in caller's frame)
        //   2. Push call overhead: return-addr, LCL, ARG, THIS, THAT
        //   3. ARG = SP - nArgs - 5   (points back to arg0)
        //   4. LCL = SP               (start of callee's frame)
        //   5. goto callee

        // 1. Evaluate and push all arguments
        for arg in args {
            self.gen_expr(arg, vars)?;
        }

        // 2. Push call overhead
        self.emit(&format!("@{}", ret_lbl));
        self.emit("D=A");
        self.push_d(); // push return address

        self.emit("@LCL");
        self.emit("D=M");
        self.push_d(); // push saved LCL

        self.emit("@ARG");
        self.emit("D=M");
        self.push_d(); // push saved ARG

        self.emit("@THIS");
        self.emit("D=M");
        self.push_d(); // push saved THIS

        self.emit("@THAT");
        self.emit("D=M");
        self.push_d(); // push saved THAT

        // 3. ARG = SP - nArgs - 5
        self.emit("@SP");
        self.emit("D=M");
        self.emit(&format!("@{}", n_args + 5));
        self.emit("D=D-A");
        self.emit("@ARG");
        self.emit("M=D");

        // 4. LCL = SP
        self.emit("@SP");
        self.emit("D=M");
        self.emit("@LCL");
        self.emit("M=D");

        // 5. goto callee
        self.emit(&format!("@{}", name));
        self.emit("0;JMP");

        self.emit(&format!("({})", ret_lbl));
        // return value is now on top of the stack
        Ok(())
    }

    // ── Statement codegen ────────────────────────────────────────────────

    fn gen_stmt(
        &mut self,
        stmt: &Stmt,
        vars: &HashMap<String, VarInfo>,
        func_name: &str,
    ) -> Result<(), CodegenError> {
        match stmt {
            Stmt::Expr(e) => {
                self.gen_expr(e, vars)?;
                // discard result
                self.pop_d();
            }
            Stmt::Block(stmts) => {
                for s in stmts {
                    self.gen_stmt(s, vars, func_name)?;
                }
            }
            Stmt::Decl(_, name, init) => {
                if let Some(init_expr) = init {
                    let info = vars.get(name).ok_or_else(|| {
                        CodegenError::new(format!("undefined local '{}'", name))
                    })?.clone();
                    if let Expr::InitList(items) = init_expr {
                        for (i, item) in items.iter().enumerate() {
                            self.gen_expr(item, vars)?;
                            self.pop_d();
                            self.emit("@R13");
                            self.emit("M=D");
                            match &info.storage {
                                VarStorage::Local(base) => {
                                    let idx = base + i;
                                    self.emit("@LCL");
                                    self.emit("D=M");
                                    if idx > 0 {
                                        self.emit(&format!("@{}", idx));
                                        self.emit("D=D+A");
                                    }
                                    self.emit("@R14");
                                    self.emit("M=D");
                                    self.emit("@R13");
                                    self.emit("D=M");
                                    self.emit("@R14");
                                    self.emit("A=M");
                                    self.emit("M=D");
                                }
                                VarStorage::Global(base) => {
                                    let addr = base + i;
                                    self.emit(&format!("@{}", addr));
                                    self.emit("D=A");
                                    self.emit("@R14");
                                    self.emit("M=D");
                                    self.emit("@R13");
                                    self.emit("D=M");
                                    self.emit("@R14");
                                    self.emit("A=M");
                                    self.emit("M=D");
                                }
                                VarStorage::Param(base) => {
                                    let idx = base + i;
                                    self.emit("@ARG");
                                    self.emit("D=M");
                                    if idx > 0 {
                                        self.emit(&format!("@{}", idx));
                                        self.emit("D=D+A");
                                    }
                                    self.emit("@R14");
                                    self.emit("M=D");
                                    self.emit("@R13");
                                    self.emit("D=M");
                                    self.emit("@R14");
                                    self.emit("A=M");
                                    self.emit("M=D");
                                }
                            }
                        }
                    } else {
                        self.gen_expr(init_expr, vars)?;
                        self.pop_d();
                        self.emit("@R13");
                        self.emit("M=D");
                        self.store_var_from_r13(&info);
                    }
                }
            }
            Stmt::Return(expr) => {
                if let Some(e) = expr {
                    self.gen_expr(e, vars)?;
                } else {
                    // void return: push 0 as dummy
                    self.emit("D=0");
                    self.push_d();
                }
                self.emit(&format!("@{}$return", func_name));
                self.emit("0;JMP");
            }
            Stmt::If(cond, then, els) => {
                let id = self.label();
                let l_else = format!("__if_else_{}", id);
                let l_end  = format!("__if_end_{}", id);
                self.gen_expr(cond, vars)?;
                self.pop_d();
                self.emit(&format!("@{}", if els.is_some() { &l_else } else { &l_end }));
                self.emit("D;JEQ");
                self.gen_stmt(then, vars, func_name)?;
                if let Some(else_stmt) = els {
                    self.emit(&format!("@{}", l_end));
                    self.emit("0;JMP");
                    self.emit(&format!("({})", l_else));
                    self.gen_stmt(else_stmt, vars, func_name)?;
                }
                self.emit(&format!("({})", l_end));
            }
            Stmt::While(cond, body) => {
                let id = self.label();
                let l_top = format!("__while_top_{}", id);
                let l_end = format!("__while_end_{}", id);
                self.loop_ctx.push((l_end.clone(), l_top.clone()));
                self.emit(&format!("({})", l_top));
                self.gen_expr(cond, vars)?;
                self.pop_d();
                self.emit(&format!("@{}", l_end));
                self.emit("D;JEQ");
                self.gen_stmt(body, vars, func_name)?;
                self.emit(&format!("@{}", l_top));
                self.emit("0;JMP");
                self.emit(&format!("({})", l_end));
                self.loop_ctx.pop();
            }
            Stmt::For { init, cond, incr, body } => {
                let id = self.label();
                let l_top  = format!("__for_top_{}", id);
                let l_incr = format!("__for_incr_{}", id);
                let l_end  = format!("__for_end_{}", id);
                if let Some(s) = init {
                    self.gen_stmt(s, vars, func_name)?;
                }
                self.loop_ctx.push((l_end.clone(), l_incr.clone()));
                self.emit(&format!("({})", l_top));
                if let Some(c) = cond {
                    self.gen_expr(c, vars)?;
                    self.pop_d();
                    self.emit(&format!("@{}", l_end));
                    self.emit("D;JEQ");
                }
                self.gen_stmt(body, vars, func_name)?;
                self.emit(&format!("({})", l_incr));
                if let Some(inc) = incr {
                    self.gen_expr(inc, vars)?;
                    self.pop_d();
                }
                self.emit(&format!("@{}", l_top));
                self.emit("0;JMP");
                self.emit(&format!("({})", l_end));
                self.loop_ctx.pop();
            }

            Stmt::DoWhile(body, cond) => {
                let id = self.label();
                let l_top  = format!("__dowhile_top_{}", id);
                let l_cond = format!("__dowhile_cond_{}", id);
                let l_end  = format!("__dowhile_end_{}", id);
                self.loop_ctx.push((l_end.clone(), l_cond.clone()));
                self.emit(&format!("({})", l_top));
                self.gen_stmt(body, vars, func_name)?;
                self.emit(&format!("({})", l_cond));
                self.gen_expr(cond, vars)?;
                self.pop_d();
                self.emit(&format!("@{}", l_end));
                self.emit("D;JEQ");
                self.emit(&format!("@{}", l_top));
                self.emit("0;JMP");
                self.emit(&format!("({})", l_end));
                self.loop_ctx.pop();
            }
            Stmt::Break => {
                if let Some((l_break, _)) = self.loop_ctx.last().cloned() {
                    self.emit(&format!("@{}", l_break));
                    self.emit("0;JMP");
                }
            }
            Stmt::Continue => {
                if let Some((_, l_continue)) = self.loop_ctx.last().cloned() {
                    if !l_continue.is_empty() {
                        self.emit(&format!("@{}", l_continue));
                        self.emit("0;JMP");
                    }
                }
            }
            Stmt::Switch { expr, arms } => {
                let id = self.label();
                let l_end = format!("__switch_end_{}", id);

                self.gen_expr(expr, vars)?;
                self.pop_d();
                self.emit("@R13");
                self.emit("M=D");

                let arm_labels: Vec<String> = (0..arms.len())
                    .map(|i| format!("__switch_arm_{}_{}", id, i))
                    .collect();

                let mut default_label: Option<String> = None;

                for (i, arm) in arms.iter().enumerate() {
                    for label in &arm.labels {
                        match label {
                            SwitchLabel::Case(val) => {
                                let val = *val;
                                self.emit("@R13");
                                self.emit("D=M");
                                if val == 0 {
                                    // compare with 0: D=value already
                                } else if val > 0 {
                                    self.emit(&format!("@{}", val));
                                    self.emit("D=D-A");
                                } else {
                                    self.emit(&format!("@{}", -val));
                                    self.emit("D=D+A");
                                }
                                self.emit(&format!("@{}", arm_labels[i]));
                                self.emit("D;JEQ");
                            }
                            SwitchLabel::Default => {
                                default_label = Some(arm_labels[i].clone());
                            }
                        }
                    }
                }
                if let Some(ref dl) = default_label {
                    self.emit(&format!("@{}", dl));
                    self.emit("0;JMP");
                } else {
                    self.emit(&format!("@{}", l_end));
                    self.emit("0;JMP");
                }

                self.loop_ctx.push((l_end.clone(), String::new()));

                for (i, arm) in arms.iter().enumerate() {
                    self.emit(&format!("({})", arm_labels[i]));
                    for s in &arm.stmts {
                        self.gen_stmt(s, vars, func_name)?;
                    }
                }

                self.loop_ctx.pop();
                self.emit(&format!("({})", l_end));
            }
        }
        Ok(())
    }

    // ── Function codegen ─────────────────────────────────────────────────

    fn gen_func(&mut self, f: &AnnotatedFunc) -> Result<(), CodegenError> {
        let n_locals = f.n_locals;
        self.emit(&format!("// function {} ({} locals)", f.name, n_locals));
        self.emit(&format!("({})", f.name));

        // Initialize locals to 0
        for _ in 0..n_locals {
            self.emit("@SP");
            self.emit("A=M");
            self.emit("M=0");
            self.emit("@SP");
            self.emit("M=M+1");
        }

        // Generate body
        for stmt in &f.body {
            self.gen_stmt(stmt, &f.vars, &f.name)?;
        }

        // Implicit return 0 for functions that fall off the end without a return.
        // Explicit `return` statements jump directly to (func$return), bypassing this.
        self.emit("D=0");
        self.emit("@SP");
        self.emit("A=M");
        self.emit("M=D");
        self.emit("@SP");
        self.emit("M=M+1");

        // Return label for early exits
        self.emit(&format!("({}$return)", f.name));

        // Jack VM return sequence
        // FRAME(R13) = LCL
        self.emit("@LCL");
        self.emit("D=M");
        self.emit("@R13");
        self.emit("M=D");

        // RET(R14) = *(FRAME-5)
        self.emit("@5");
        self.emit("A=D-A");
        self.emit("D=M");
        self.emit("@R14");
        self.emit("M=D");

        // *ARG = return value (top of stack)
        self.pop_d();
        self.emit("@ARG");
        self.emit("A=M");
        self.emit("M=D");

        // SP = ARG + 1
        self.emit("@ARG");
        self.emit("D=M+1");
        self.emit("@SP");
        self.emit("M=D");

        // THAT = *(FRAME-1)
        self.emit("@R13");
        self.emit("AM=M-1");
        self.emit("D=M");
        self.emit("@THAT");
        self.emit("M=D");

        // THIS = *(FRAME-2)
        self.emit("@R13");
        self.emit("AM=M-1");
        self.emit("D=M");
        self.emit("@THIS");
        self.emit("M=D");

        // ARG = *(FRAME-3)
        self.emit("@R13");
        self.emit("AM=M-1");
        self.emit("D=M");
        self.emit("@ARG");
        self.emit("M=D");

        // LCL = *(FRAME-4)
        self.emit("@R13");
        self.emit("AM=M-1");
        self.emit("D=M");
        self.emit("@LCL");
        self.emit("M=D");

        // goto RET
        self.emit("@R14");
        self.emit("A=M");
        self.emit("0;JMP");

        Ok(())
    }

    // ── Runtime subroutines ──────────────────────────────────────────────

    fn emit_runtime(&mut self) {
        let need_mul          = self.used_builtins.contains(&BuiltinKind::Mul);
        let need_div          = self.used_builtins.contains(&BuiltinKind::Div);
        let need_puts         = self.used_builtins.contains(&BuiltinKind::Puts);
        let need_strlen       = self.used_builtins.contains(&BuiltinKind::Strlen);
        let need_draw_pixel   = self.used_builtins.contains(&BuiltinKind::DrawPixel);
        let need_clear_pixel  = self.used_builtins.contains(&BuiltinKind::ClearPixel);
        let need_fill_screen  = self.used_builtins.contains(&BuiltinKind::FillScreen);
        let need_clear_screen = self.used_builtins.contains(&BuiltinKind::ClearScreen);
        let need_draw_char    = self.used_builtins.contains(&BuiltinKind::DrawChar);
        let need_draw_string  = self.used_builtins.contains(&BuiltinKind::DrawString);

        if need_mul {
            // __mul: R13 * R14, result in R13. Return via R3.
            self.emit("");
            self.emit("// === Runtime: __mul ===");
            self.emit("(__mul)");
            self.emit("@R15");
            self.emit("M=0");
            self.emit("@R13");
            self.emit("D=M");
            self.emit("@__mul_end");
            self.emit("D;JEQ");
            self.emit("@R14");
            self.emit("D=M");
            self.emit("@__mul_end");
            self.emit("D;JEQ");
            self.emit("@R5");
            self.emit("M=0");
            self.emit("@R13");
            self.emit("D=M");
            self.emit("@__mul_r13p");
            self.emit("D;JGE");
            self.emit("@R5");
            self.emit("M=!M");
            self.emit("@R13");
            self.emit("M=-M");
            self.emit("(__mul_r13p)");
            self.emit("@R14");
            self.emit("D=M");
            self.emit("@__mul_r14p");
            self.emit("D;JGE");
            self.emit("@R5");
            self.emit("M=!M");
            self.emit("@R14");
            self.emit("M=-M");
            self.emit("(__mul_r14p)");
            self.emit("(__mul_loop)");
            self.emit("@R14");
            self.emit("D=M");
            self.emit("@__mul_done");
            self.emit("D;JEQ");
            self.emit("@R13");
            self.emit("D=M");
            self.emit("@R15");
            self.emit("M=M+D");
            self.emit("@R14");
            self.emit("M=M-1");
            self.emit("@__mul_loop");
            self.emit("0;JMP");
            self.emit("(__mul_done)");
            self.emit("@R5");
            self.emit("D=M");
            self.emit("@__mul_pos");
            self.emit("D;JEQ");
            self.emit("@R15");
            self.emit("M=-M");
            self.emit("(__mul_pos)");
            self.emit("(__mul_end)");
            self.emit("@R15");
            self.emit("D=M");
            self.emit("@R13");
            self.emit("M=D");
            self.emit("@R3");
            self.emit("A=M");
            self.emit("0;JMP");
        }

        if need_div {
            // __div: R13 / R14 = quotient in R13, remainder in R15. Return via R3.
            self.emit("");
            self.emit("// === Runtime: __div ===");
            self.emit("(__div)");
            self.emit("@R14");
            self.emit("D=M");
            self.emit("@__div_zero");
            self.emit("D;JEQ");
            self.emit("@R5");
            self.emit("M=0");
            self.emit("@R6");
            self.emit("M=0");
            self.emit("@R13");
            self.emit("D=M");
            self.emit("@__div_r13p");
            self.emit("D;JGE");
            self.emit("@R5");
            self.emit("M=!M");
            self.emit("@R6");
            self.emit("M=!M");
            self.emit("@R13");
            self.emit("M=-M");
            self.emit("(__div_r13p)");
            self.emit("@R14");
            self.emit("D=M");
            self.emit("@__div_r14p");
            self.emit("D;JGE");
            self.emit("@R5");
            self.emit("M=!M");
            self.emit("@R14");
            self.emit("M=-M");
            self.emit("(__div_r14p)");
            self.emit("@R13");
            self.emit("D=M");
            self.emit("@R15");
            self.emit("M=D");
            self.emit("@R13");
            self.emit("M=0");
            self.emit("(__div_loop)");
            self.emit("@R15");
            self.emit("D=M");
            self.emit("@R14");
            self.emit("D=D-M");
            self.emit("@__div_done");
            self.emit("D;JLT");
            self.emit("@R15");
            self.emit("M=D");
            self.emit("@R13");
            self.emit("M=M+1");
            self.emit("@__div_loop");
            self.emit("0;JMP");
            self.emit("(__div_done)");
            self.emit("@R5");
            self.emit("D=M");
            self.emit("@__div_qpos");
            self.emit("D;JEQ");
            self.emit("@R13");
            self.emit("M=-M");
            self.emit("(__div_qpos)");
            self.emit("@R6");
            self.emit("D=M");
            self.emit("@__div_rpos");
            self.emit("D;JEQ");
            self.emit("@R15");
            self.emit("M=-M");
            self.emit("(__div_rpos)");
            self.emit("@R3");
            self.emit("A=M");
            self.emit("0;JMP");
            self.emit("(__div_zero)");
            self.emit("@R13");
            self.emit("M=0");
            self.emit("@R15");
            self.emit("M=0");
            self.emit("@R3");
            self.emit("A=M");
            self.emit("0;JMP");
        }

        if need_puts {
            // __puts: print null-terminated string at R13, then newline. Return via R3.
            self.emit("");
            self.emit("// === Runtime: __puts ===");
            self.emit("(__puts)");
            self.emit("@R13");
            self.emit("A=M");
            self.emit("D=M");
            self.emit("@__puts_end");
            self.emit("D;JEQ");
            self.emit(&format!("@{}", HACK_OUTPUT_PORT));
            self.emit("M=D");
            self.emit("@R13");
            self.emit("M=M+1");
            self.emit("@__puts");
            self.emit("0;JMP");
            self.emit("(__puts_end)");
            self.emit("@10");
            self.emit("D=A");
            self.emit(&format!("@{}", HACK_OUTPUT_PORT));
            self.emit("M=D");
            self.emit("@R3");
            self.emit("A=M");
            self.emit("0;JMP");
        }

        if need_strlen {
            // __strlen: length of null-terminated string at R13. Result in R13. Return via R3.
            self.emit("");
            self.emit("// === Runtime: __strlen ===");
            self.emit("(__strlen)");
            self.emit("@R14");
            self.emit("M=0");
            self.emit("(__strlen_loop)");
            self.emit("@R13");
            self.emit("A=M");
            self.emit("D=M");
            self.emit("@__strlen_end");
            self.emit("D;JEQ");
            self.emit("@R13");
            self.emit("M=M+1");
            self.emit("@R14");
            self.emit("M=M+1");
            self.emit("@__strlen_loop");
            self.emit("0;JMP");
            self.emit("(__strlen_end)");
            self.emit("@R14");
            self.emit("D=M");
            self.emit("@R13");
            self.emit("M=D");
            self.emit("@R3");
            self.emit("A=M");
            self.emit("0;JMP");
        }

        if need_draw_pixel {
            // __draw_pixel: set pixel (R13=x, R14=y) to black. Return via R3.
            self.emit("");
            self.emit("// === Runtime: __draw_pixel ===");
            self.emit("(__draw_pixel)");
            self.emit("@R14"); self.emit("D=M");
            self.emit("@R5");  self.emit("M=D");
            for _ in 0..5 {
                self.emit("@R5"); self.emit("D=M"); self.emit("M=D+M");
            }
            self.emit("@R13"); self.emit("D=M");
            self.emit("@R7");  self.emit("M=D");
            self.emit("@R6");  self.emit("M=0");
            self.emit("(__dp_div16)");
            self.emit("@R7"); self.emit("D=M");
            self.emit("@16"); self.emit("D=D-A");
            self.emit("@__dp_div16_done"); self.emit("D;JLT");
            self.emit("@R7"); self.emit("M=D");
            self.emit("@R6"); self.emit("M=M+1");
            self.emit("@__dp_div16"); self.emit("0;JMP");
            self.emit("(__dp_div16_done)");
            self.emit("@R9"); self.emit("M=1");
            self.emit("(__dp_shift)");
            self.emit("@R7"); self.emit("D=M");
            self.emit("@__dp_shift_done"); self.emit("D;JEQ");
            self.emit("@R9"); self.emit("D=M"); self.emit("M=D+M");
            self.emit("@R7"); self.emit("M=M-1");
            self.emit("@__dp_shift"); self.emit("0;JMP");
            self.emit("(__dp_shift_done)");
            self.emit("@16384"); self.emit("D=A");
            self.emit("@R5");    self.emit("D=D+M");
            self.emit("@R6");    self.emit("D=D+M");
            self.emit("@R8");    self.emit("M=D");
            self.emit("@R8"); self.emit("A=M"); self.emit("D=M");
            self.emit("@R9"); self.emit("D=D|M");
            self.emit("@R8"); self.emit("A=M"); self.emit("M=D");
            self.emit("@R3"); self.emit("A=M"); self.emit("0;JMP");
        }

        if need_clear_pixel {
            // __clear_pixel: set pixel (R13=x, R14=y) to white. Return via R3.
            self.emit("");
            self.emit("// === Runtime: __clear_pixel ===");
            self.emit("(__clear_pixel)");
            self.emit("@R14"); self.emit("D=M");
            self.emit("@R5");  self.emit("M=D");
            for _ in 0..5 {
                self.emit("@R5"); self.emit("D=M"); self.emit("M=D+M");
            }
            self.emit("@R13"); self.emit("D=M");
            self.emit("@R7");  self.emit("M=D");
            self.emit("@R6");  self.emit("M=0");
            self.emit("(__cp_div16)");
            self.emit("@R7"); self.emit("D=M");
            self.emit("@16"); self.emit("D=D-A");
            self.emit("@__cp_div16_done"); self.emit("D;JLT");
            self.emit("@R7"); self.emit("M=D");
            self.emit("@R6"); self.emit("M=M+1");
            self.emit("@__cp_div16"); self.emit("0;JMP");
            self.emit("(__cp_div16_done)");
            self.emit("@R9"); self.emit("M=1");
            self.emit("(__cp_shift)");
            self.emit("@R7"); self.emit("D=M");
            self.emit("@__cp_shift_done"); self.emit("D;JEQ");
            self.emit("@R9"); self.emit("D=M"); self.emit("M=D+M");
            self.emit("@R7"); self.emit("M=M-1");
            self.emit("@__cp_shift"); self.emit("0;JMP");
            self.emit("(__cp_shift_done)");
            self.emit("@16384"); self.emit("D=A");
            self.emit("@R5");    self.emit("D=D+M");
            self.emit("@R6");    self.emit("D=D+M");
            self.emit("@R8");    self.emit("M=D");
            self.emit("@R9"); self.emit("D=!M");
            self.emit("@R8"); self.emit("A=M"); self.emit("D=D&M");
            self.emit("@R8"); self.emit("A=M"); self.emit("M=D");
            self.emit("@R3"); self.emit("A=M"); self.emit("0;JMP");
        }

        if need_fill_screen {
            // __fill_screen: fill all screen words with -1. Return via R3.
            self.emit("");
            self.emit("// === Runtime: __fill_screen ===");
            self.emit("(__fill_screen)");
            self.emit("@16384"); self.emit("D=A");
            self.emit("@R13");   self.emit("M=D");
            self.emit("(__fill_loop)");
            self.emit("@24576"); self.emit("D=A");
            self.emit("@R13");   self.emit("D=D-M");
            self.emit("@__fill_done"); self.emit("D;JLE");
            self.emit("@R13"); self.emit("A=M"); self.emit("M=-1");
            self.emit("@R13"); self.emit("M=M+1");
            self.emit("@__fill_loop"); self.emit("0;JMP");
            self.emit("(__fill_done)");
            self.emit("@R3"); self.emit("A=M"); self.emit("0;JMP");
        }

        if need_clear_screen {
            // __clear_screen: fill all screen words with 0. Return via R3.
            self.emit("");
            self.emit("// === Runtime: __clear_screen ===");
            self.emit("(__clear_screen)");
            self.emit("@16384"); self.emit("D=A");
            self.emit("@R13");   self.emit("M=D");
            self.emit("(__clrscr_loop)");
            self.emit("@24576"); self.emit("D=A");
            self.emit("@R13");   self.emit("D=D-M");
            self.emit("@__clrscr_done"); self.emit("D;JLE");
            self.emit("@R13"); self.emit("A=M"); self.emit("M=0");
            self.emit("@R13"); self.emit("M=M+1");
            self.emit("@__clrscr_loop"); self.emit("0;JMP");
            self.emit("(__clrscr_done)");
            self.emit("@R3"); self.emit("A=M"); self.emit("0;JMP");
        }

        if need_draw_char {
            // __draw_char: draw character at text cell.
            // Inputs: R13=col (0-63), R14=row (0-22), R15=char_code. Return via R3.
            // Each character cell is 8 pixels wide × 11 rows tall.
            self.emit("");
            self.emit("// === Runtime: __draw_char ===");
            self.emit("(__draw_char)");
            self.emit("@R3"); self.emit("D=M"); self.emit("@R12"); self.emit("M=D");
            self.emit("@R13"); self.emit("D=M"); self.emit("@R7"); self.emit("M=0"); self.emit("@R8"); self.emit("M=D");
            self.emit("(__dc_div2)");
            self.emit("@R8"); self.emit("D=M"); self.emit("@2"); self.emit("D=D-A");
            self.emit("@__dc_div2_done"); self.emit("D;JLT");
            self.emit("@R8"); self.emit("M=D");
            self.emit("@R7"); self.emit("M=M+1");
            self.emit("@__dc_div2"); self.emit("0;JMP");
            self.emit("(__dc_div2_done)");
            // row * 32 * 11 = row * 352
            self.emit("@R14"); self.emit("D=M"); self.emit("@R9"); self.emit("M=D");
            // multiply R9 by 352 = 256 + 64 + 32
            // R9 * 256
            self.emit("@R9"); self.emit("D=M");
            for _ in 0..8 { self.emit("@R9"); self.emit("D=M"); self.emit("M=D+M"); }
            // now R9 = row * 256; we need row * (256+64+32) = row*256 + row*64 + row*32
            // save row * 256 in R10, restore original row
            self.emit("@R9"); self.emit("D=M"); self.emit("@R10"); self.emit("M=D");
            self.emit("@R14"); self.emit("D=M"); self.emit("@R9"); self.emit("M=D");
            // row * 64
            for _ in 0..6 { self.emit("@R9"); self.emit("D=M"); self.emit("M=D+M"); }
            self.emit("@R10"); self.emit("D=M"); self.emit("@R9"); self.emit("D=D+M"); self.emit("@R10"); self.emit("M=D");
            self.emit("@R14"); self.emit("D=M"); self.emit("@R9"); self.emit("M=D");
            // row * 32
            for _ in 0..5 { self.emit("@R9"); self.emit("D=M"); self.emit("M=D+M"); }
            self.emit("@R10"); self.emit("D=M"); self.emit("@R9"); self.emit("D=D+M");
            // D = row * 352
            self.emit(&format!("@{}", 16384)); self.emit("D=A+D");
            self.emit("@R7"); self.emit("D=D+M");
            self.emit("@R9"); self.emit("M=D");
            // font pointer: (char_code - 32) * 11 + FONT_BASE
            self.emit("@R15"); self.emit("D=M");
            self.emit("@32");  self.emit("D=D-A");
            self.emit("@R6");  self.emit("M=D");
            // multiply R6 by 11 = 8 + 2 + 1
            self.emit("@R6"); self.emit("D=M"); self.emit("@R11"); self.emit("M=D");
            for _ in 0..3 { self.emit("@R6"); self.emit("D=M"); self.emit("M=D+M"); }
            // R6 = (char-32)*8, add (char-32)*2
            self.emit("@R11"); self.emit("D=M"); self.emit("@R6"); self.emit("D=D+M");
            // D = (char-32)*3; no wait: R11=(char-32), R6=(char-32)*8
            // We want *11: 8+2+1
            // R6 = char*8
            // D = R11 + R6 = char + char*8 = char*9; not right
            // Let me just multiply by 11 properly in a loop for simplicity
            // Reset: R6 = char - 32, multiply by 11
            self.emit("@R15"); self.emit("D=M");
            self.emit("@32");  self.emit("D=D-A");
            self.emit("@R6");  self.emit("M=D");     // R6 = char - 32
            self.emit("@R11"); self.emit("M=D");     // R11 = char - 32 (accumulator)
            self.emit("@10");  self.emit("D=A");
            self.emit("@R10"); self.emit("M=D");     // R10 = 10 (loop counter)
            self.emit("(__dc_mul11)");
            self.emit("@R10"); self.emit("D=M");
            self.emit("@__dc_mul11_done"); self.emit("D;JEQ");
            self.emit("@R6"); self.emit("D=M");
            self.emit("@R11"); self.emit("M=D+M");
            self.emit("@R10"); self.emit("M=M-1");
            self.emit("@__dc_mul11"); self.emit("0;JMP");
            self.emit("(__dc_mul11_done)");
            self.emit(&format!("@{}", FONT_BASE)); self.emit("D=A");
            self.emit("@R11"); self.emit("D=D+M");
            self.emit("@R6"); self.emit("M=D");
            self.emit("@R5"); self.emit("M=0");
            self.emit("(__dc_row_loop)");
            self.emit("@R5"); self.emit("D=M"); self.emit("@11"); self.emit("D=D-A");
            self.emit("@__dc_row_done"); self.emit("D;JGE");
            self.emit("@R6"); self.emit("A=M"); self.emit("D=M");
            self.emit("@R10"); self.emit("M=D");
            self.emit("@R8"); self.emit("D=M");
            self.emit("@__dc_high"); self.emit("D;JNE");
            self.emit("@255"); self.emit("D=!A");
            self.emit("@R11"); self.emit("M=D");
            self.emit("@R9"); self.emit("A=M"); self.emit("D=M");
            self.emit("@R11"); self.emit("D=D&M");
            self.emit("@R10"); self.emit("D=D|M");
            self.emit("@R9"); self.emit("A=M"); self.emit("M=D");
            self.emit("@__dc_row_cont"); self.emit("0;JMP");
            self.emit("(__dc_high)");
            self.emit("@R10"); self.emit("D=M"); self.emit("@R11"); self.emit("M=D");
            for _ in 0..8 {
                self.emit("@R11"); self.emit("D=M"); self.emit("M=D+M");
            }
            self.emit("@255"); self.emit("D=A");
            self.emit("@R9"); self.emit("A=M"); self.emit("D=D&M");
            self.emit("@R11"); self.emit("D=D|M");
            self.emit("@R9"); self.emit("A=M"); self.emit("M=D");
            self.emit("(__dc_row_cont)");
            self.emit("@R6"); self.emit("M=M+1");
            self.emit("@32"); self.emit("D=A"); self.emit("@R9"); self.emit("M=D+M");
            self.emit("@R5"); self.emit("M=M+1");
            self.emit("@__dc_row_loop"); self.emit("0;JMP");
            self.emit("(__dc_row_done)");
            self.emit("@R12"); self.emit("A=M"); self.emit("0;JMP");
        }

        if need_draw_string {
            // __draw_string: draw null-terminated string at text cell.
            // Inputs: R13=col, R14=row, R15=str_ptr. Return via R3.
            self.emit("");
            self.emit("// === Runtime: __draw_string ===");
            self.emit("(__draw_string)");
            self.emit("@R3"); self.emit("D=M"); self.emit("@R4"); self.emit("M=D");
            self.emit("@R15"); self.emit("D=M"); self.emit("@R5"); self.emit("M=D");
            self.emit("@R13"); self.emit("D=M"); self.emit("@R6"); self.emit("M=D");
            self.emit("(__ds_loop)");
            self.emit("@R5"); self.emit("A=M"); self.emit("D=M");
            self.emit("@__ds_done"); self.emit("D;JEQ");
            self.emit("@R5"); self.emit("D=M"); self.emit("@SP"); self.emit("A=M"); self.emit("M=D"); self.emit("@SP"); self.emit("M=M+1");
            self.emit("@R6"); self.emit("D=M"); self.emit("@SP"); self.emit("A=M"); self.emit("M=D"); self.emit("@SP"); self.emit("M=M+1");
            self.emit("@R6"); self.emit("D=M"); self.emit("@R13"); self.emit("M=D");
            self.emit("@R5"); self.emit("A=M"); self.emit("D=M"); self.emit("@R15"); self.emit("M=D");
            self.emit("@__ds_char_ret"); self.emit("D=A"); self.emit("@R3"); self.emit("M=D");
            self.emit("@__draw_char"); self.emit("0;JMP");
            self.emit("(__ds_char_ret)");
            self.emit("@SP"); self.emit("M=M-1"); self.emit("A=M"); self.emit("D=M"); self.emit("@R6"); self.emit("M=D");
            self.emit("@SP"); self.emit("M=M-1"); self.emit("A=M"); self.emit("D=M"); self.emit("@R5"); self.emit("M=D");
            self.emit("@R5"); self.emit("M=M+1");
            self.emit("@R6"); self.emit("M=M+1");
            self.emit("@__ds_loop"); self.emit("0;JMP");
            self.emit("(__ds_done)");
            self.emit("@R4"); self.emit("A=M"); self.emit("0;JMP");
        }

        let need_strcpy  = self.used_builtins.contains(&BuiltinKind::Strcpy);
        let need_strcmp  = self.used_builtins.contains(&BuiltinKind::Strcmp);
        let need_strcat  = self.used_builtins.contains(&BuiltinKind::Strcat);
        let need_itoa    = self.used_builtins.contains(&BuiltinKind::Itoa);

        if need_strcpy {
            // __strcpy: copy src (R14) to dst (R13), including null terminator.
            // R13 = dst (preserved), R14 = src (advances). Return via R3.
            // Uses R5 as running dst ptr.
            self.emit("");
            self.emit("// === Runtime: __strcpy ===");
            self.emit("(__strcpy)");
            self.emit("@R13"); self.emit("D=M"); self.emit("@R5"); self.emit("M=D");
            self.emit("(__strcpy_loop)");
            self.emit("@R14"); self.emit("A=M"); self.emit("D=M"); // D = *src
            self.emit("@R5"); self.emit("A=M"); self.emit("M=D");  // *dst_ptr = D
            self.emit("@R14"); self.emit("M=M+1");                  // src++
            self.emit("@R5"); self.emit("M=M+1");                   // dst_ptr++
            self.emit("@__strcpy_loop"); self.emit("D;JNE");        // loop until null
            self.emit("@R3"); self.emit("A=M"); self.emit("0;JMP");
        }

        if need_strcmp {
            // __strcmp: compare strings at R13 (a) and R14 (b).
            // Result in R13: negative if a<b, 0 if equal, positive if a>b. Return via R3.
            // Uses R6 as temp to save *a.
            self.emit("");
            self.emit("// === Runtime: __strcmp ===");
            self.emit("(__strcmp)");
            self.emit("(__strcmp_loop)");
            self.emit("@R13"); self.emit("A=M"); self.emit("D=M"); // D = *a
            self.emit("@R6"); self.emit("M=D");                    // R6 = *a
            self.emit("@R14"); self.emit("A=M"); self.emit("D=M"); // D = *b
            self.emit("@R6"); self.emit("D=M-D");                  // D = *a - *b
            self.emit("@__strcmp_ne"); self.emit("D;JNE");
            // equal so far — check for null
            self.emit("@R13"); self.emit("A=M"); self.emit("D=M"); // D = *a
            self.emit("@__strcmp_done"); self.emit("D;JEQ");       // both null → equal
            self.emit("@R13"); self.emit("M=M+1");
            self.emit("@R14"); self.emit("M=M+1");
            self.emit("@__strcmp_loop"); self.emit("0;JMP");
            self.emit("(__strcmp_ne)");
            self.emit("(__strcmp_done)");
            self.emit("@R13"); self.emit("M=D");
            self.emit("@R3"); self.emit("A=M"); self.emit("0;JMP");
        }

        if need_strcat {
            // __strcat: append src (R14) to end of dst (R13). Return via R3. R13 unchanged.
            // Uses R5 as running ptr to find and then fill the end of dst.
            self.emit("");
            self.emit("// === Runtime: __strcat ===");
            self.emit("(__strcat)");
            self.emit("@R13"); self.emit("D=M"); self.emit("@R5"); self.emit("M=D");
            self.emit("(__strcat_find_end)");
            self.emit("@R5"); self.emit("A=M"); self.emit("D=M");   // D = *ptr
            self.emit("@__strcat_copy"); self.emit("D;JEQ");        // found end
            self.emit("@R5"); self.emit("M=M+1");
            self.emit("@__strcat_find_end"); self.emit("0;JMP");
            self.emit("(__strcat_copy)");
            self.emit("@R14"); self.emit("A=M"); self.emit("D=M");  // D = *src
            self.emit("@R5"); self.emit("A=M"); self.emit("M=D");   // *dst_end = D
            self.emit("@R14"); self.emit("M=M+1");
            self.emit("@R5"); self.emit("M=M+1");
            self.emit("@__strcat_copy"); self.emit("D;JNE");
            self.emit("@R3"); self.emit("A=M"); self.emit("0;JMP");
        }

        if need_itoa {
            // __itoa: convert int R13 to decimal string in buffer at R14.
            // Returns original buf address in R14. Return via R3.
            // R7=write_ptr, R8=buf_start, R9=left_ptr, R10=right_ptr, R11=swap_tmp, R12=sign
            self.emit("");
            self.emit("// === Runtime: __itoa ===");
            self.emit("(__itoa)");
            // Save buf_start in R8; init write_ptr R7 = buf_start
            self.emit("@R14"); self.emit("D=M"); self.emit("@R8"); self.emit("M=D");
            self.emit("@R8");  self.emit("D=M"); self.emit("@R7"); self.emit("M=D");
            // Determine sign
            self.emit("@R12"); self.emit("M=0");      // sign = 0 (positive)
            self.emit("@R13"); self.emit("D=M");
            self.emit("@__itoa_pos"); self.emit("D;JGE");
            self.emit("@R12"); self.emit("M=1");      // sign = 1 (negative)
            self.emit("@R13"); self.emit("M=-M");     // n = abs(n)
            self.emit("(__itoa_pos)");
            // Special case: n == 0
            self.emit("@R13"); self.emit("D=M");
            self.emit("@__itoa_zero"); self.emit("D;JEQ");
            // Extract digits (in reverse) using repeated division by 10
            self.emit("(__itoa_dloop)");
            self.emit("@R13"); self.emit("D=M");
            self.emit("@__itoa_dloop_done"); self.emit("D;JEQ");
            self.emit("@10"); self.emit("D=A"); self.emit("@R14"); self.emit("M=D"); // R14=10
            self.emit("@__itoa_dr"); self.emit("D=A"); self.emit("@R3"); self.emit("M=D");
            self.emit("@__div"); self.emit("0;JMP");
            self.emit("(__itoa_dr)");
            // R13=quotient, R15=remainder(digit)
            self.emit("@R15"); self.emit("D=M"); self.emit("@48"); self.emit("D=D+A"); // '0'+digit
            self.emit("@R7"); self.emit("A=M"); self.emit("M=D"); // *write_ptr = char
            self.emit("@R7"); self.emit("M=M+1");
            self.emit("@__itoa_dloop"); self.emit("0;JMP");
            self.emit("(__itoa_dloop_done)");
            // Append '-' if negative
            self.emit("@R12"); self.emit("D=M");
            self.emit("@__itoa_rev"); self.emit("D;JEQ");
            self.emit("@45"); self.emit("D=A"); // '-'
            self.emit("@R7"); self.emit("A=M"); self.emit("M=D");
            self.emit("@R7"); self.emit("M=M+1");
            self.emit("(__itoa_rev)");
            // Null-terminate
            self.emit("@R7"); self.emit("A=M"); self.emit("M=0");
            // Reverse: R9=left=buf_start value, R10=right=write_ptr-1 value
            self.emit("@R8"); self.emit("D=M"); self.emit("@R9"); self.emit("M=D");
            self.emit("@R7"); self.emit("D=M"); self.emit("D=D-1"); self.emit("@R10"); self.emit("M=D");
            self.emit("(__itoa_rev_loop)");
            self.emit("@R9"); self.emit("D=M"); self.emit("@R10"); self.emit("D=D-M");
            self.emit("@__itoa_rev_done"); self.emit("D;JGE"); // left >= right → done
            // swap *R9 and *R10
            self.emit("@R9"); self.emit("A=M"); self.emit("D=M");  // D = *left
            self.emit("@R11"); self.emit("M=D");                    // R11 = *left
            self.emit("@R10"); self.emit("A=M"); self.emit("D=M"); // D = *right
            self.emit("@R9"); self.emit("A=M"); self.emit("M=D");  // *left = *right
            self.emit("@R11"); self.emit("D=M");
            self.emit("@R10"); self.emit("A=M"); self.emit("M=D"); // *right = old *left
            self.emit("@R9"); self.emit("M=M+1");
            self.emit("@R10"); self.emit("M=M-1");
            self.emit("@__itoa_rev_loop"); self.emit("0;JMP");
            self.emit("(__itoa_rev_done)");
            // Restore R14 = buf_start for caller to return
            self.emit("@R8"); self.emit("D=M"); self.emit("@R14"); self.emit("M=D");
            self.emit("@R3"); self.emit("A=M"); self.emit("0;JMP");
            // Special case: n==0 → write "0\0"
            self.emit("(__itoa_zero)");
            self.emit("@48"); self.emit("D=A"); // '0'
            self.emit("@R7"); self.emit("A=M"); self.emit("M=D");
            self.emit("@R7"); self.emit("M=M+1");
            self.emit("@R7"); self.emit("A=M"); self.emit("M=0"); // null
            self.emit("@R8"); self.emit("D=M"); self.emit("@R14"); self.emit("M=D");
            self.emit("@R3"); self.emit("A=M"); self.emit("0;JMP");
        }
    }
}

// ── Pre-scan helpers (call graph + builtin usage analysis) ───────────────────

fn collect_calls_from_stmts(stmts: &[Stmt]) -> HashSet<String> {
    let mut calls = HashSet::new();
    for s in stmts { collect_calls_stmt(s, &mut calls); }
    calls
}

fn collect_calls_stmt(s: &Stmt, calls: &mut HashSet<String>) {
    match s {
        Stmt::Expr(e)           => collect_calls_expr(e, calls),
        Stmt::Return(Some(e))   => collect_calls_expr(e, calls),
        Stmt::Decl(_, _, Some(e)) => collect_calls_expr(e, calls),
        Stmt::Block(ss)         => ss.iter().for_each(|s| collect_calls_stmt(s, calls)),
        Stmt::If(c, t, e) => {
            collect_calls_expr(c, calls);
            collect_calls_stmt(t, calls);
            if let Some(e) = e { collect_calls_stmt(e, calls); }
        }
        Stmt::While(c, b) => {
            collect_calls_expr(c, calls);
            collect_calls_stmt(b, calls);
        }
        Stmt::DoWhile(b, c) => {
            collect_calls_stmt(b, calls);
            collect_calls_expr(c, calls);
        }
        Stmt::For { init, cond, incr, body } => {
            if let Some(s) = init { collect_calls_stmt(s, calls); }
            if let Some(e) = cond { collect_calls_expr(e, calls); }
            if let Some(e) = incr { collect_calls_expr(e, calls); }
            collect_calls_stmt(body, calls);
        }
        Stmt::Switch { expr, arms } => {
            collect_calls_expr(expr, calls);
            for arm in arms {
                for s in &arm.stmts { collect_calls_stmt(s, calls); }
            }
        }
        Stmt::Break | Stmt::Continue => {}
        _ => {}
    }
}

fn collect_calls_expr(e: &Expr, calls: &mut HashSet<String>) {
    match e {
        Expr::Call(name, args) => {
            calls.insert(name.clone());
            for a in args { collect_calls_expr(a, calls); }
        }
        Expr::BinOp(_, l, r) => { collect_calls_expr(l, calls); collect_calls_expr(r, calls); }
        Expr::UnOp(_, inner)  => collect_calls_expr(inner, calls),
        Expr::Index(a, b)     => { collect_calls_expr(a, calls); collect_calls_expr(b, calls); }
        Expr::Member(b, _)    => collect_calls_expr(b, calls),
        Expr::Ternary(c, t, e) => {
            collect_calls_expr(c, calls);
            collect_calls_expr(t, calls);
            collect_calls_expr(e, calls);
        }
        Expr::Cast(_, e) => collect_calls_expr(e, calls),
        Expr::PostInc(e) | Expr::PostDec(e) => collect_calls_expr(e, calls),
        Expr::InitList(items) => {
            for item in items { collect_calls_expr(item, calls); }
        }
        _ => {}
    }
}

fn scan_builtins_from_stmts(stmts: &[Stmt], used: &mut HashSet<BuiltinKind>) {
    for s in stmts { scan_builtins_stmt(s, used); }
}

fn scan_builtins_stmt(s: &Stmt, used: &mut HashSet<BuiltinKind>) {
    match s {
        Stmt::Expr(e)           => scan_builtins_expr(e, used),
        Stmt::Return(Some(e))   => scan_builtins_expr(e, used),
        Stmt::Decl(_, _, Some(e)) => scan_builtins_expr(e, used),
        Stmt::Block(ss)         => ss.iter().for_each(|s| scan_builtins_stmt(s, used)),
        Stmt::If(c, t, e) => {
            scan_builtins_expr(c, used);
            scan_builtins_stmt(t, used);
            if let Some(e) = e { scan_builtins_stmt(e, used); }
        }
        Stmt::While(c, b) => {
            scan_builtins_expr(c, used);
            scan_builtins_stmt(b, used);
        }
        Stmt::DoWhile(b, c) => {
            scan_builtins_stmt(b, used);
            scan_builtins_expr(c, used);
        }
        Stmt::For { init, cond, incr, body } => {
            if let Some(s) = init { scan_builtins_stmt(s, used); }
            if let Some(e) = cond { scan_builtins_expr(e, used); }
            if let Some(e) = incr { scan_builtins_expr(e, used); }
            scan_builtins_stmt(body, used);
        }
        Stmt::Switch { expr, arms } => {
            scan_builtins_expr(expr, used);
            for arm in arms {
                for s in &arm.stmts { scan_builtins_stmt(s, used); }
            }
        }
        Stmt::Break | Stmt::Continue => {}
        _ => {}
    }
}

fn scan_builtins_expr(e: &Expr, used: &mut HashSet<BuiltinKind>) {
    match e {
        Expr::Call(name, args) => {
            match name.as_str() {
                "puts"                   => { used.insert(BuiltinKind::Puts); }
                "strlen"                 => { used.insert(BuiltinKind::Strlen); }
                "draw_pixel"             => { used.insert(BuiltinKind::DrawPixel); }
                "clear_pixel"            => { used.insert(BuiltinKind::ClearPixel); }
                "fill_screen"            => { used.insert(BuiltinKind::FillScreen); }
                "clear_screen"           => { used.insert(BuiltinKind::ClearScreen); }
                "draw_char"              => { used.insert(BuiltinKind::DrawChar); }
                "draw_string" | "print_at" => {
                    used.insert(BuiltinKind::DrawString);
                    used.insert(BuiltinKind::DrawChar); // transitive dependency
                }
                "strcpy" => { used.insert(BuiltinKind::Strcpy); }
                "strcmp" => { used.insert(BuiltinKind::Strcmp); }
                "strcat" => { used.insert(BuiltinKind::Strcat); }
                "itoa"   => { used.insert(BuiltinKind::Itoa); used.insert(BuiltinKind::Div); }
                _ => {}
            }
            for a in args { scan_builtins_expr(a, used); }
        }
        Expr::BinOp(op, l, r) => {
            match op {
                BinOp::Mul | BinOp::MulAssign => { used.insert(BuiltinKind::Mul); }
                BinOp::Div | BinOp::Mod | BinOp::Shr
                | BinOp::DivAssign | BinOp::ModAssign | BinOp::ShrAssign => { used.insert(BuiltinKind::Div); }
                _ => {}
            }
            scan_builtins_expr(l, used);
            scan_builtins_expr(r, used);
        }
        Expr::UnOp(_, inner) => scan_builtins_expr(inner, used),
        Expr::Index(a, b)    => { scan_builtins_expr(a, used); scan_builtins_expr(b, used); }
        Expr::Member(b, _)   => scan_builtins_expr(b, used),
        Expr::Ternary(c, t, e) => {
            scan_builtins_expr(c, used);
            scan_builtins_expr(t, used);
            scan_builtins_expr(e, used);
        }
        Expr::Cast(_, e) => scan_builtins_expr(e, used),
        Expr::PostInc(e) | Expr::PostDec(e) => scan_builtins_expr(e, used),
        Expr::InitList(items) => {
            for item in items { scan_builtins_expr(item, used); }
        }
        _ => {}
    }
}

// ── Entry point ──────────────────────────────────────────────────────────────

/// Compile all functions including the bootstrap (full program, ready to link and emit).
pub fn generate(sema: SemaResult) -> Result<CompiledProgram, CodegenError> {
    generate_inner(sema, false)
}

/// Compile function bodies only — no bootstrap, no entry-point call to main.
/// Used when producing `.hobj` object files for later linking by `hack_ld`.
pub fn generate_body_only(sema: SemaResult) -> Result<CompiledProgram, CodegenError> {
    generate_inner(sema, true)
}

/// Return the Hack assembly bootstrap that initialises the stack pointer,
/// calls `main`, and halts.  A `// __DATA_INIT_HERE__` marker is embedded so
/// `output::emit` can splice in global/string initialisations.
pub fn gen_bootstrap() -> String {
    let mut lines: Vec<&str> = Vec::new();
    let body = [
        "// Bootstrap",
        "@256", "D=A", "@SP", "M=D",
        "// __DATA_INIT_HERE__",
        "@__ld_main_ret", "D=A",
        "@SP", "A=M", "M=D", "@SP", "M=M+1",
        "@LCL",  "D=M", "@SP", "A=M", "M=D", "@SP", "M=M+1",
        "@ARG",  "D=M", "@SP", "A=M", "M=D", "@SP", "M=M+1",
        "@THIS", "D=M", "@SP", "A=M", "M=D", "@SP", "M=M+1",
        "@THAT", "D=M", "@SP", "A=M", "M=D", "@SP", "M=M+1",
        "@SP", "D=M", "@5", "D=D-A", "@ARG", "M=D",
        "@SP", "D=M", "@LCL", "M=D",
        "@main", "0;JMP",
        "(__ld_main_ret)",
        "(__end)", "@__end", "0;JMP",
        "",
    ];
    lines.extend_from_slice(&body);
    lines.join("\n")
}

fn generate_inner(sema: SemaResult, body_only: bool) -> Result<CompiledProgram, CodegenError> {
    // ── Phase 1: Build call graph; find reachable user functions from main ──
    let func_names: HashSet<String> = sema.funcs.iter().map(|f| f.name.clone()).collect();
    let mut call_graph: HashMap<String, HashSet<String>> = HashMap::new();
    for f in &sema.funcs {
        let all_calls = collect_calls_from_stmts(&f.body);
        let user_calls: HashSet<String> = all_calls.into_iter()
            .filter(|name| func_names.contains(name))
            .collect();
        call_graph.insert(f.name.clone(), user_calls);
    }
    let mut reachable: HashSet<String> = HashSet::new();
    // In body_only mode (separate compilation), every function is a potential
    // entry point, so seed the BFS with all defined functions.
    // In whole-program mode, start only from main.
    let seeds: Vec<String> = if body_only {
        func_names.iter().cloned().collect()
    } else {
        vec!["main".to_string()]
    };
    let mut queue = seeds;
    while let Some(name) = queue.pop() {
        if reachable.contains(&name) { continue; }
        reachable.insert(name.clone());
        if let Some(callees) = call_graph.get(&name) {
            for callee in callees {
                if !reachable.contains(callee) { queue.push(callee.clone()); }
            }
        }
    }

    // ── Phase 2: Determine which runtime helpers are needed ──────────────────
    let mut used_builtins: HashSet<BuiltinKind> = HashSet::new();
    for f in &sema.funcs {
        if reachable.contains(&f.name) {
            scan_builtins_from_stmts(&f.body, &mut used_builtins);
        }
    }
    // Transitive: draw_string always requires draw_char
    if used_builtins.contains(&BuiltinKind::DrawString) {
        used_builtins.insert(BuiltinKind::DrawChar);
    }

    // ── Phase 3: Generate code ───────────────────────────────────────────────
    let mut g = Gen::new(sema.string_map.clone(), sema.struct_defs.clone(), used_builtins.clone());

    if !body_only {
        g.emit("// Bootstrap");
        g.emit("@256");
        g.emit("D=A");
        g.emit("@SP");
        g.emit("M=D");
        // Marker: output module inserts data-init code here for asm/hack formats
        g.emit("// __DATA_INIT_HERE__");

        // Call main (Jack VM calling convention)
        let id = g.label();
        let ret_lbl = format!("main$ret_{}", id);
        g.emit(&format!("@{}", ret_lbl));
        g.emit("D=A");
        g.emit("@SP");
        g.emit("A=M");
        g.emit("M=D");
        g.emit("@SP");
        g.emit("M=M+1");
        for reg in &["LCL", "ARG", "THIS", "THAT"] {
            g.emit(&format!("@{}", reg));
            g.emit("D=M");
            g.emit("@SP");
            g.emit("A=M");
            g.emit("M=D");
            g.emit("@SP");
            g.emit("M=M+1");
        }
        g.emit("@SP");
        g.emit("D=M");
        g.emit("@5");
        g.emit("D=D-A");
        g.emit("@ARG");
        g.emit("M=D");
        g.emit("@SP");
        g.emit("D=M");
        g.emit("@LCL");
        g.emit("M=D");
        g.emit("@main");
        g.emit("0;JMP");
        g.emit(&format!("({})", ret_lbl));
        g.emit("(__end)");
        g.emit("@__end");
        g.emit("0;JMP");
        g.emit("");
    }

    // Emit only reachable user-defined functions
    for f in &sema.funcs {
        if reachable.contains(&f.name) {
            g.emit("");
            g.gen_func(f)?;
        }
    }

    // Runtime subroutines are now resolved by the linker (linker.rs).
    // emit_runtime() is no longer called here.


    let asm = g.out.join("\n") + "\n";

    // ── Phase 4: Collect DataInit entries ────────────────────────────────────
    let mut data: Vec<DataInit> = Vec::new();

    // Non-zero global variable initialisations
    for (_name, addr, _ty, init_val) in &sema.globals {
        if let Some(val) = init_val {
            if *val != 0 {
                data.push(DataInit { address: *addr as u16, value: *val as i16 });
            }
        }
    }

    // String literal content (null terminators are zero so we skip them)
    for (addr, chars) in &sema.string_literals {
        for (i, &ch) in chars.iter().enumerate() {
            if ch != 0 {
                data.push(DataInit { address: (*addr + i) as u16, value: ch });
            }
        }
    }

    // Font table — only if draw_char or draw_string is used
    if used_builtins.contains(&BuiltinKind::DrawChar) {
        for ch_idx in 0..96usize {
            for row in 0..11usize {
                let byte = FONT_8X11[ch_idx][row];
                // Jack OS font already uses bit-0=leftmost, matching Hack screen format.
                // No reversal needed.
                if byte == 0 { continue; }
                let addr = (FONT_BASE + ch_idx * 11 + row) as u16;
                data.push(DataInit { address: addr, value: byte as i16 });
            }
        }
    }

    Ok(CompiledProgram { asm, data })
}

