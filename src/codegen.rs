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

/// Base RAM address of the 8×8 font table (96 chars × 8 rows = 768 words).
/// Placed at the top of general-purpose RAM, just below screen memory (16384).
/// 15616 + 768 = 16384 (screen base). Valid Hack RAM is 0-16383; screen is 16384-24575.
pub const FONT_BASE: usize = 15616;

/// 8×8 bitmap font for ASCII 32-127.
/// Each entry is 8 bytes, one per screen row, MSB = leftmost pixel
/// (standard convention; bytes are bit-reversed on write to match Hack's
/// LSB-leftmost screen layout).
const FONT_8X8: [[u8; 8]; 96] = [
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00], // 32 ' '
    [0x18,0x3C,0x3C,0x18,0x18,0x00,0x18,0x00], // 33 '!'
    [0x36,0x36,0x00,0x00,0x00,0x00,0x00,0x00], // 34 '"'
    [0x36,0x36,0x7F,0x36,0x7F,0x36,0x36,0x00], // 35 '#'
    [0x0C,0x3E,0x60,0x3C,0x06,0x7C,0x18,0x00], // 36 '$'
    [0x00,0x63,0x33,0x18,0x0C,0x66,0x63,0x00], // 37 '%'
    [0x1C,0x36,0x1C,0x6E,0x3B,0x33,0x6E,0x00], // 38 '&'
    [0x18,0x18,0x30,0x00,0x00,0x00,0x00,0x00], // 39 '\''
    [0x0C,0x18,0x30,0x30,0x30,0x18,0x0C,0x00], // 40 '('
    [0x30,0x18,0x0C,0x0C,0x0C,0x18,0x30,0x00], // 41 ')'
    [0x00,0x66,0x3C,0xFF,0x3C,0x66,0x00,0x00], // 42 '*'
    [0x00,0x18,0x18,0x7E,0x18,0x18,0x00,0x00], // 43 '+'
    [0x00,0x00,0x00,0x00,0x00,0x18,0x18,0x30], // 44 ','
    [0x00,0x00,0x00,0x7E,0x00,0x00,0x00,0x00], // 45 '-'
    [0x00,0x00,0x00,0x00,0x00,0x18,0x18,0x00], // 46 '.'
    [0x03,0x06,0x0C,0x18,0x30,0x60,0x40,0x00], // 47 '/'
    [0x3E,0x63,0x63,0x6B,0x63,0x63,0x3E,0x00], // 48 '0'
    [0x18,0x38,0x18,0x18,0x18,0x18,0x7E,0x00], // 49 '1'
    [0x3C,0x66,0x06,0x1C,0x30,0x66,0x7E,0x00], // 50 '2'
    [0x3C,0x66,0x06,0x1C,0x06,0x66,0x3C,0x00], // 51 '3'
    [0x0E,0x1E,0x36,0x66,0x7F,0x06,0x06,0x00], // 52 '4'
    [0x7E,0x60,0x7C,0x06,0x06,0x66,0x3C,0x00], // 53 '5'
    [0x1C,0x30,0x60,0x7C,0x66,0x66,0x3C,0x00], // 54 '6'
    [0x7E,0x66,0x06,0x0C,0x18,0x18,0x18,0x00], // 55 '7'
    [0x3C,0x66,0x66,0x3C,0x66,0x66,0x3C,0x00], // 56 '8'
    [0x3C,0x66,0x66,0x3E,0x06,0x0C,0x38,0x00], // 57 '9'
    [0x00,0x18,0x18,0x00,0x00,0x18,0x18,0x00], // 58 ':'
    [0x00,0x18,0x18,0x00,0x00,0x18,0x18,0x30], // 59 ';'
    [0x06,0x0C,0x18,0x30,0x18,0x0C,0x06,0x00], // 60 '<'
    [0x00,0x00,0x7E,0x00,0x00,0x7E,0x00,0x00], // 61 '='
    [0x60,0x30,0x18,0x0C,0x18,0x30,0x60,0x00], // 62 '>'
    [0x3C,0x66,0x06,0x0C,0x18,0x00,0x18,0x00], // 63 '?'
    [0x3E,0x63,0x6F,0x69,0x6F,0x60,0x3C,0x00], // 64 '@'
    [0x18,0x3C,0x66,0x66,0x7E,0x66,0x66,0x00], // 65 'A'
    [0x7C,0x66,0x66,0x7C,0x66,0x66,0x7C,0x00], // 66 'B'
    [0x3C,0x66,0x60,0x60,0x60,0x66,0x3C,0x00], // 67 'C'
    [0x78,0x6C,0x66,0x66,0x66,0x6C,0x78,0x00], // 68 'D'
    [0x7E,0x60,0x60,0x78,0x60,0x60,0x7E,0x00], // 69 'E'
    [0x7E,0x60,0x60,0x78,0x60,0x60,0x60,0x00], // 70 'F'
    [0x3C,0x66,0x60,0x6E,0x66,0x66,0x3C,0x00], // 71 'G'
    [0x66,0x66,0x66,0x7E,0x66,0x66,0x66,0x00], // 72 'H'
    [0x3C,0x18,0x18,0x18,0x18,0x18,0x3C,0x00], // 73 'I'
    [0x1E,0x0C,0x0C,0x0C,0x0C,0x6C,0x38,0x00], // 74 'J'
    [0x66,0x6C,0x78,0x70,0x78,0x6C,0x66,0x00], // 75 'K'
    [0x60,0x60,0x60,0x60,0x60,0x60,0x7E,0x00], // 76 'L'
    [0xC3,0xE7,0xFF,0xDB,0xC3,0xC3,0xC3,0x00], // 77 'M'
    [0xC3,0xE3,0xF3,0xDB,0xCF,0xC7,0xC3,0x00], // 78 'N'
    [0x3C,0x66,0x66,0x66,0x66,0x66,0x3C,0x00], // 79 'O'
    [0x7C,0x66,0x66,0x7C,0x60,0x60,0x60,0x00], // 80 'P'
    [0x3C,0x66,0x66,0x66,0x66,0x3C,0x0E,0x00], // 81 'Q'
    [0x7C,0x66,0x66,0x7C,0x78,0x6C,0x66,0x00], // 82 'R'
    [0x3C,0x66,0x60,0x3C,0x06,0x66,0x3C,0x00], // 83 'S'
    [0x7E,0x18,0x18,0x18,0x18,0x18,0x18,0x00], // 84 'T'
    [0x66,0x66,0x66,0x66,0x66,0x66,0x3C,0x00], // 85 'U'
    [0x66,0x66,0x66,0x66,0x66,0x3C,0x18,0x00], // 86 'V'
    [0xC3,0xC3,0xC3,0xDB,0xFF,0xE7,0xC3,0x00], // 87 'W'
    [0xC3,0x66,0x3C,0x18,0x3C,0x66,0xC3,0x00], // 88 'X'
    [0x66,0x66,0x66,0x3C,0x18,0x18,0x18,0x00], // 89 'Y'
    [0x7E,0x06,0x0C,0x18,0x30,0x60,0x7E,0x00], // 90 'Z'
    [0x3C,0x30,0x30,0x30,0x30,0x30,0x3C,0x00], // 91 '['
    [0x80,0x40,0x20,0x10,0x08,0x04,0x02,0x00], // 92 '\'
    [0x3C,0x0C,0x0C,0x0C,0x0C,0x0C,0x3C,0x00], // 93 ']'
    [0x10,0x38,0x6C,0xC6,0x00,0x00,0x00,0x00], // 94 '^'
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0xFF], // 95 '_'
    [0x30,0x18,0x0C,0x00,0x00,0x00,0x00,0x00], // 96 '`'
    [0x00,0x00,0x3C,0x06,0x3E,0x66,0x3E,0x00], // 97 'a'
    [0x60,0x60,0x7C,0x66,0x66,0x66,0x7C,0x00], // 98 'b'
    [0x00,0x00,0x3C,0x60,0x60,0x60,0x3C,0x00], // 99 'c'
    [0x06,0x06,0x3E,0x66,0x66,0x66,0x3E,0x00], // 100 'd'
    [0x00,0x00,0x3C,0x66,0x7E,0x60,0x3C,0x00], // 101 'e'
    [0x1C,0x30,0x30,0x7C,0x30,0x30,0x30,0x00], // 102 'f'
    [0x00,0x00,0x3E,0x66,0x66,0x3E,0x06,0x3C], // 103 'g'
    [0x60,0x60,0x7C,0x66,0x66,0x66,0x66,0x00], // 104 'h'
    [0x18,0x00,0x38,0x18,0x18,0x18,0x3C,0x00], // 105 'i'
    [0x06,0x00,0x06,0x06,0x06,0x06,0x66,0x3C], // 106 'j'
    [0x60,0x60,0x66,0x6C,0x78,0x6C,0x66,0x00], // 107 'k'
    [0x38,0x18,0x18,0x18,0x18,0x18,0x3C,0x00], // 108 'l'
    [0x00,0x00,0xCC,0xFE,0xFE,0xD6,0xC6,0x00], // 109 'm'
    [0x00,0x00,0x7C,0x66,0x66,0x66,0x66,0x00], // 110 'n'
    [0x00,0x00,0x3C,0x66,0x66,0x66,0x3C,0x00], // 111 'o'
    [0x00,0x00,0x7C,0x66,0x66,0x7C,0x60,0x60], // 112 'p'
    [0x00,0x00,0x3E,0x66,0x66,0x3E,0x06,0x06], // 113 'q'
    [0x00,0x00,0x6C,0x76,0x60,0x60,0x60,0x00], // 114 'r'
    [0x00,0x00,0x3C,0x60,0x3C,0x06,0x7C,0x00], // 115 's'
    [0x18,0x18,0x7E,0x18,0x18,0x18,0x0E,0x00], // 116 't'
    [0x00,0x00,0x66,0x66,0x66,0x66,0x3E,0x00], // 117 'u'
    [0x00,0x00,0x66,0x66,0x66,0x3C,0x18,0x00], // 118 'v'
    [0x00,0x00,0xC6,0xD6,0xFE,0xFE,0x6C,0x00], // 119 'w'
    [0x00,0x00,0x66,0x3C,0x18,0x3C,0x66,0x00], // 120 'x'
    [0x00,0x00,0x66,0x66,0x66,0x3E,0x06,0x3C], // 121 'y'
    [0x00,0x00,0x7E,0x0C,0x18,0x30,0x7E,0x00], // 122 'z'
    [0x0E,0x18,0x18,0x70,0x18,0x18,0x0E,0x00], // 123 '{'
    [0x18,0x18,0x18,0x00,0x18,0x18,0x18,0x00], // 124 '|'
    [0x70,0x18,0x18,0x0E,0x18,0x18,0x70,0x00], // 125 '}'
    [0x76,0xDC,0x00,0x00,0x00,0x00,0x00,0x00], // 126 '~'
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00], // 127 DEL
];

use std::collections::HashMap;
use thiserror::Error;
use crate::sema::{SemaResult, AnnotatedFunc, VarInfo, VarStorage, type_size};
use crate::parser::{Expr, Stmt, BinOp, UnOp, Type};

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
}

impl Gen {
    fn new(
        _func_sigs: HashMap<String, (Type, usize)>,
        string_map: HashMap<String, usize>,
        struct_defs: HashMap<String, Vec<(String, Type)>>,
    ) -> Self {
        Self { out: Vec::new(), label_id: 0, string_map, struct_defs }
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
                self.load_var(&info);
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
                // arr[i] = *(arr + i)
                self.gen_expr(base, vars)?;
                self.gen_expr(idx, vars)?;
                // stack: [..., base, idx]
                self.pop_d();             // D = idx
                self.emit("@R14");
                self.emit("M=D");         // R14 = idx
                self.pop_d();             // D = base
                self.emit("@R14");
                self.emit("D=D+M");       // D = base + idx (address)
                self.emit("A=D");
                self.emit("D=M");         // D = RAM[base+idx]
                self.push_d();
            }

            Expr::Member(_, _) => {
                // Load value at field address: gen_addr gives the address, then deref
                self.gen_addr(expr, vars)?;
                self.pop_d();
                self.emit("A=D");
                self.emit("D=M");
                self.push_d();
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
                // &arr[i] = arr + i
                self.gen_expr(base, vars)?;
                self.gen_expr(idx, vars)?;
                self.pop_d();
                self.emit("@R14");
                self.emit("M=D");
                self.emit("@SP");
                self.emit("M=M-1");
                self.emit("A=M");
                self.emit("D=M");
                self.emit("@R14");
                self.emit("D=D+M");
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
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                self.gen_cmp(op)?;
            }
            BinOp::Assign | BinOp::AddAssign | BinOp::SubAssign | BinOp::And | BinOp::Or => {
                unreachable!()
            }
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
                    // eval init, store directly to local (same as assign)
                    let info = vars.get(name).ok_or_else(|| {
                        CodegenError::new(format!("undefined local '{}'", name))
                    })?.clone();
                    self.gen_expr(init_expr, vars)?;
                    self.pop_d();
                    self.emit("@R13");
                    self.emit("M=D");
                    self.store_var_from_r13(&info);
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
                self.emit(&format!("({})", l_top));
                self.gen_expr(cond, vars)?;
                self.pop_d();
                self.emit(&format!("@{}", l_end));
                self.emit("D;JEQ");
                self.gen_stmt(body, vars, func_name)?;
                self.emit(&format!("@{}", l_top));
                self.emit("0;JMP");
                self.emit(&format!("({})", l_end));
            }
            Stmt::For { init, cond, incr, body } => {
                let id = self.label();
                let l_top = format!("__for_top_{}", id);
                let l_end = format!("__for_end_{}", id);
                if let Some(s) = init {
                    self.gen_stmt(s, vars, func_name)?;
                }
                self.emit(&format!("({})", l_top));
                if let Some(c) = cond {
                    self.gen_expr(c, vars)?;
                    self.pop_d();
                    self.emit(&format!("@{}", l_end));
                    self.emit("D;JEQ");
                }
                self.gen_stmt(body, vars, func_name)?;
                if let Some(inc) = incr {
                    self.gen_expr(inc, vars)?;
                    self.pop_d(); // discard
                }
                self.emit(&format!("@{}", l_top));
                self.emit("0;JMP");
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
        // __mul: R13 * R14, result in R13. Return via R3.
        self.emit("");
        self.emit("// === Runtime: __mul ===");
        self.emit("(__mul)");
        // Result in R15
        self.emit("@R15");
        self.emit("M=0");
        // Check for zero operands
        self.emit("@R13");
        self.emit("D=M");
        self.emit("@__mul_end");
        self.emit("D;JEQ");
        self.emit("@R14");
        self.emit("D=M");
        self.emit("@__mul_end");
        self.emit("D;JEQ");
        // Determine sign: R5 = sign flag (0 = positive)
        self.emit("@R5");
        self.emit("M=0");
        // Make R13 positive
        self.emit("@R13");
        self.emit("D=M");
        self.emit("@__mul_r13p");
        self.emit("D;JGE");
        self.emit("@R5");
        self.emit("M=!M");
        self.emit("@R13");
        self.emit("M=-M");
        self.emit("(__mul_r13p)");
        // Make R14 positive
        self.emit("@R14");
        self.emit("D=M");
        self.emit("@__mul_r14p");
        self.emit("D;JGE");
        self.emit("@R5");
        self.emit("M=!M");
        self.emit("@R14");
        self.emit("M=-M");
        self.emit("(__mul_r14p)");
        // R15 = 0; while R14 > 0: R15 += R13; R14--
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
        // Apply sign
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

        // __div: R13 / R14 = quotient in R13, remainder in R15. Return via R3.
        self.emit("");
        self.emit("// === Runtime: __div ===");
        self.emit("(__div)");
        // Handle division by zero: result = 0
        self.emit("@R14");
        self.emit("D=M");
        self.emit("@__div_zero");
        self.emit("D;JEQ");
        // Sign handling
        self.emit("@R5");
        self.emit("M=0"); // sign of quotient
        self.emit("@R6");
        self.emit("M=0"); // sign of remainder
        // Make R13 positive
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
        // Make R14 positive
        self.emit("@R14");
        self.emit("D=M");
        self.emit("@__div_r14p");
        self.emit("D;JGE");
        self.emit("@R5");
        self.emit("M=!M");
        self.emit("@R14");
        self.emit("M=-M");
        self.emit("(__div_r14p)");
        // R15 = R13; R13 = 0 (quotient); subtract R14 from R15 repeatedly
        self.emit("@R13");
        self.emit("D=M");
        self.emit("@R15");
        self.emit("M=D"); // R15 = dividend (will become remainder)
        self.emit("@R13");
        self.emit("M=0"); // quotient = 0
        self.emit("(__div_loop)");
        self.emit("@R15");
        self.emit("D=M");
        self.emit("@R14");
        self.emit("D=D-M"); // D = R15 - R14
        self.emit("@__div_done");
        self.emit("D;JLT");
        self.emit("@R15");
        self.emit("M=D"); // R15 -= R14
        self.emit("@R13");
        self.emit("M=M+1"); // quotient++
        self.emit("@__div_loop");
        self.emit("0;JMP");
        self.emit("(__div_done)");
        // Apply sign to quotient
        self.emit("@R5");
        self.emit("D=M");
        self.emit("@__div_qpos");
        self.emit("D;JEQ");
        self.emit("@R13");
        self.emit("M=-M");
        self.emit("(__div_qpos)");
        // Apply sign to remainder
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

        // __puts: print null-terminated string at R13, then newline. Return via R3.
        self.emit("");
        self.emit("// === Runtime: __puts ===");
        self.emit("(__puts)");
        self.emit("@R13");
        self.emit("A=M");
        self.emit("D=M");           // D = *R13
        self.emit("@__puts_end");
        self.emit("D;JEQ");         // if *R13 == 0, done
        self.emit(&format!("@{}", HACK_OUTPUT_PORT));
        self.emit("M=D");           // output the char
        self.emit("@R13");
        self.emit("M=M+1");         // R13++
        self.emit("@__puts");
        self.emit("0;JMP");
        self.emit("(__puts_end)");
        self.emit("@10");           // '\n'
        self.emit("D=A");
        self.emit(&format!("@{}", HACK_OUTPUT_PORT));
        self.emit("M=D");
        self.emit("@R3");
        self.emit("A=M");
        self.emit("0;JMP");

        // __strlen: length of null-terminated string at R13. Result in R13. Return via R3.
        self.emit("");
        self.emit("// === Runtime: __strlen ===");
        self.emit("(__strlen)");
        self.emit("@R14");
        self.emit("M=0");           // R14 = count = 0
        self.emit("(__strlen_loop)");
        self.emit("@R13");
        self.emit("A=M");
        self.emit("D=M");           // D = *R13
        self.emit("@__strlen_end");
        self.emit("D;JEQ");         // if *R13 == 0, done
        self.emit("@R13");
        self.emit("M=M+1");         // R13++ (ptr advance)
        self.emit("@R14");
        self.emit("M=M+1");         // count++
        self.emit("@__strlen_loop");
        self.emit("0;JMP");
        self.emit("(__strlen_end)");
        self.emit("@R14");
        self.emit("D=M");
        self.emit("@R13");
        self.emit("M=D");           // R13 = count (result)
        self.emit("@R3");
        self.emit("A=M");
        self.emit("0;JMP");

        // __draw_pixel: set pixel (R13=x, R14=y) to black. Return via R3.
        // Pixel addr = 16384 + y*32 + x/16. Bit = x%16. RAM[addr] |= (1<<bit).
        // Uses R5 (y*32), R6 (x/16), R7 (x%16 / shift counter), R8 (addr), R9 (mask).
        self.emit("");
        self.emit("// === Runtime: __draw_pixel ===");
        self.emit("(__draw_pixel)");
        // y * 32 -> R5 (5 doublings)
        self.emit("@R14"); self.emit("D=M");
        self.emit("@R5");  self.emit("M=D");   // R5 = y
        for _ in 0..5 {
            self.emit("@R5"); self.emit("D=M"); self.emit("M=D+M"); // R5 *= 2
        }
        // x / 16 -> R6,  x % 16 -> R7
        self.emit("@R13"); self.emit("D=M");
        self.emit("@R7");  self.emit("M=D");   // R7 = x
        self.emit("@R6");  self.emit("M=0");   // R6 = quotient
        self.emit("(__dp_div16)");
        self.emit("@R7"); self.emit("D=M");
        self.emit("@16"); self.emit("D=D-A");
        self.emit("@__dp_div16_done"); self.emit("D;JLT");
        self.emit("@R7"); self.emit("M=D");    // R7 -= 16
        self.emit("@R6"); self.emit("M=M+1");  // R6++
        self.emit("@__dp_div16"); self.emit("0;JMP");
        self.emit("(__dp_div16_done)");        // R6=x/16, R7=x%16
        // mask = 1 << R7 -> R9
        self.emit("@R9"); self.emit("M=1");
        self.emit("(__dp_shift)");
        self.emit("@R7"); self.emit("D=M");
        self.emit("@__dp_shift_done"); self.emit("D;JEQ");
        self.emit("@R9"); self.emit("D=M"); self.emit("M=D+M"); // R9 <<= 1
        self.emit("@R7"); self.emit("M=M-1");
        self.emit("@__dp_shift"); self.emit("0;JMP");
        self.emit("(__dp_shift_done)");
        // addr = 16384 + R5 + R6 -> R8
        self.emit("@16384"); self.emit("D=A");
        self.emit("@R5");    self.emit("D=D+M");
        self.emit("@R6");    self.emit("D=D+M");
        self.emit("@R8");    self.emit("M=D");
        // RAM[R8] |= R9
        self.emit("@R8"); self.emit("A=M"); self.emit("D=M");
        self.emit("@R9"); self.emit("D=D|M");
        self.emit("@R8"); self.emit("A=M"); self.emit("M=D");
        self.emit("@R3"); self.emit("A=M"); self.emit("0;JMP");

        // __clear_pixel: set pixel (R13=x, R14=y) to white. Return via R3.
        // Same setup as __draw_pixel; final step: RAM[R8] &= ~R9.
        self.emit("");
        self.emit("// === Runtime: __clear_pixel ===");
        self.emit("(__clear_pixel)");
        // y * 32 -> R5
        self.emit("@R14"); self.emit("D=M");
        self.emit("@R5");  self.emit("M=D");
        for _ in 0..5 {
            self.emit("@R5"); self.emit("D=M"); self.emit("M=D+M");
        }
        // x / 16 -> R6,  x % 16 -> R7
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
        // mask = 1 << R7 -> R9
        self.emit("@R9"); self.emit("M=1");
        self.emit("(__cp_shift)");
        self.emit("@R7"); self.emit("D=M");
        self.emit("@__cp_shift_done"); self.emit("D;JEQ");
        self.emit("@R9"); self.emit("D=M"); self.emit("M=D+M");
        self.emit("@R7"); self.emit("M=M-1");
        self.emit("@__cp_shift"); self.emit("0;JMP");
        self.emit("(__cp_shift_done)");
        // addr = 16384 + R5 + R6 -> R8
        self.emit("@16384"); self.emit("D=A");
        self.emit("@R5");    self.emit("D=D+M");
        self.emit("@R6");    self.emit("D=D+M");
        self.emit("@R8");    self.emit("M=D");
        // RAM[R8] &= ~R9
        self.emit("@R9"); self.emit("D=!M");   // D = ~mask
        self.emit("@R8"); self.emit("A=M"); self.emit("D=D&M");
        self.emit("@R8"); self.emit("A=M"); self.emit("M=D");
        self.emit("@R3"); self.emit("A=M"); self.emit("0;JMP");

        // __fill_screen: fill all screen words with -1 (all pixels black). Return via R3.
        self.emit("");
        self.emit("// === Runtime: __fill_screen ===");
        self.emit("(__fill_screen)");
        self.emit("@16384"); self.emit("D=A");
        self.emit("@R13");   self.emit("M=D");  // R13 = current addr
        self.emit("(__fill_loop)");
        self.emit("@24576"); self.emit("D=A");
        self.emit("@R13");   self.emit("D=D-M");
        self.emit("@__fill_done"); self.emit("D;JLE"); // if R13 >= 24576, done
        self.emit("@R13"); self.emit("A=M"); self.emit("M=-1");
        self.emit("@R13"); self.emit("M=M+1");
        self.emit("@__fill_loop"); self.emit("0;JMP");
        self.emit("(__fill_done)");
        self.emit("@R3"); self.emit("A=M"); self.emit("0;JMP");

        // __clear_screen: fill all screen words with 0 (all pixels white). Return via R3.
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

        // __draw_char: draw character at text cell.
        // Inputs: R13=col (0-63), R14=row (0-31), R15=char_code. Return via R3.
        // Uses: R5(row_ctr), R6(font_ptr), R7(word_col), R8(within_word 0=low/1=high),
        //       R9(screen_addr), R10(font_byte), R11(temp), R12(saved_R3).
        // Screen layout: col/2 = screen word column, col%2: 0=low byte, 1=high byte.
        // Font bytes are stored bit-reversed (bit0=leftmost) to match Hack screen.
        self.emit("");
        self.emit("// === Runtime: __draw_char ===");
        self.emit("(__draw_char)");
        // Save return address to R12
        self.emit("@R3"); self.emit("D=M"); self.emit("@R12"); self.emit("M=D");
        // Compute word_col = col/2 -> R7; within_word = col%2 -> R8
        self.emit("@R13"); self.emit("D=M"); self.emit("@R7"); self.emit("M=0"); self.emit("@R8"); self.emit("M=D");
        self.emit("(__dc_div2)");
        self.emit("@R8"); self.emit("D=M"); self.emit("@2"); self.emit("D=D-A");
        self.emit("@__dc_div2_done"); self.emit("D;JLT");
        self.emit("@R8"); self.emit("M=D");   // R8 -= 2
        self.emit("@R7"); self.emit("M=M+1"); // R7++
        self.emit("@__dc_div2"); self.emit("0;JMP");
        self.emit("(__dc_div2_done)");        // R7 = col/2, R8 = col%2 (0 or 1)
        // Compute screen_addr = 16384 + row*256 + word_col -> R9
        // row*256 via 8 doublings
        self.emit("@R14"); self.emit("D=M"); self.emit("@R9"); self.emit("M=D");
        for _ in 0..8 {
            self.emit("@R9"); self.emit("D=M"); self.emit("M=D+M");
        }
        self.emit(&format!("@{}", 16384)); self.emit("D=A");
        self.emit("@R9"); self.emit("D=D+M");
        self.emit("@R7"); self.emit("D=D+M");
        self.emit("@R9"); self.emit("M=D"); // R9 = 16384 + row*256 + word_col
        // Compute font_ptr = FONT_BASE + (char-32)*8 -> R6
        self.emit("@R15"); self.emit("D=M");
        self.emit("@32");  self.emit("D=D-A");
        self.emit("@R6");  self.emit("M=D");  // R6 = char-32
        // (char-32)*8 via 3 doublings
        for _ in 0..3 {
            self.emit("@R6"); self.emit("D=M"); self.emit("M=D+M");
        }
        self.emit(&format!("@{}", FONT_BASE)); self.emit("D=A");
        self.emit("@R6"); self.emit("M=D+M"); // R6 = FONT_BASE + (char-32)*8
        // Row loop: draw 8 rows
        self.emit("@R5"); self.emit("M=0"); // R5 = row counter
        self.emit("(__dc_row_loop)");
        self.emit("@R5"); self.emit("D=M"); self.emit("@8"); self.emit("D=D-A");
        self.emit("@__dc_row_done"); self.emit("D;JGE");
        // Load font byte: R10 = RAM[R6]
        self.emit("@R6"); self.emit("A=M"); self.emit("D=M");
        self.emit("@R10"); self.emit("M=D");
        // Branch on within_word
        self.emit("@R8"); self.emit("D=M");
        self.emit("@__dc_high"); self.emit("D;JNE");
        // ── Even col: write font_byte to LOW byte (bits 0-7) of screen word ──
        // new_word = (screen_word & 0xFF00) | font_byte
        // D=!A with A=255 gives D=~255=0xFF00
        self.emit("@255"); self.emit("D=!A");
        self.emit("@R11"); self.emit("M=D");           // R11 = 0xFF00
        self.emit("@R9"); self.emit("A=M"); self.emit("D=M"); // D = screen_word
        self.emit("@R11"); self.emit("D=D&M");         // D = screen_word & 0xFF00
        self.emit("@R10"); self.emit("D=D|M");         // D |= font_byte
        self.emit("@R9"); self.emit("A=M"); self.emit("M=D");
        self.emit("@__dc_row_cont"); self.emit("0;JMP");
        // ── Odd col: write font_byte to HIGH byte (bits 8-15) of screen word ──
        // font_byte_shifted = font_byte * 256 (8 doublings) -> R11
        // new_word = (screen_word & 0xFF) | font_byte_shifted
        self.emit("(__dc_high)");
        self.emit("@R10"); self.emit("D=M"); self.emit("@R11"); self.emit("M=D");
        for _ in 0..8 {
            self.emit("@R11"); self.emit("D=M"); self.emit("M=D+M");
        }
        // (screen_word & 0xFF) | R11
        self.emit("@255"); self.emit("D=A");
        self.emit("@R9"); self.emit("A=M"); self.emit("D=D&M"); // D = screen_word & 0xFF
        self.emit("@R11"); self.emit("D=D|M");                  // D |= shifted_byte
        self.emit("@R9"); self.emit("A=M"); self.emit("M=D");
        // ── Advance to next font row and next screen row ──
        self.emit("(__dc_row_cont)");
        self.emit("@R6"); self.emit("M=M+1");                   // font_ptr++
        self.emit("@32"); self.emit("D=A"); self.emit("@R9"); self.emit("M=D+M"); // screen_addr+=32
        self.emit("@R5"); self.emit("M=M+1");                   // row++
        self.emit("@__dc_row_loop"); self.emit("0;JMP");
        self.emit("(__dc_row_done)");
        self.emit("@R12"); self.emit("A=M"); self.emit("0;JMP");

        // __draw_string: draw null-terminated string at text cell.
        // Inputs: R13=col, R14=row, R15=str_ptr. Return via R3.
        // Saves R3 to R4 (draw_char saves R3 to R12, so no conflict).
        // Saves R5,R6 to stack around each draw_char call (draw_char uses them).
        self.emit("");
        self.emit("// === Runtime: __draw_string ===");
        self.emit("(__draw_string)");
        self.emit("@R3"); self.emit("D=M"); self.emit("@R4"); self.emit("M=D"); // save R3->R4
        self.emit("@R15"); self.emit("D=M"); self.emit("@R5"); self.emit("M=D"); // R5=str_ptr
        self.emit("@R13"); self.emit("D=M"); self.emit("@R6"); self.emit("M=D"); // R6=col
        self.emit("(__ds_loop)");
        self.emit("@R5"); self.emit("A=M"); self.emit("D=M"); // D = *ptr
        self.emit("@__ds_done"); self.emit("D;JEQ");
        // Push R5, R6 to stack (draw_char clobbers them)
        self.emit("@R5"); self.emit("D=M"); self.emit("@SP"); self.emit("A=M"); self.emit("M=D"); self.emit("@SP"); self.emit("M=M+1");
        self.emit("@R6"); self.emit("D=M"); self.emit("@SP"); self.emit("A=M"); self.emit("M=D"); self.emit("@SP"); self.emit("M=M+1");
        // Set up draw_char args: R13=col, R14=row (already), R15=char
        self.emit("@R6"); self.emit("D=M"); self.emit("@R13"); self.emit("M=D");
        self.emit("@R5"); self.emit("A=M"); self.emit("D=M"); self.emit("@R15"); self.emit("M=D");
        self.emit("@__ds_char_ret"); self.emit("D=A"); self.emit("@R3"); self.emit("M=D");
        self.emit("@__draw_char"); self.emit("0;JMP");
        self.emit("(__ds_char_ret)");
        // Pop R6, R5 (reverse order)
        self.emit("@SP"); self.emit("M=M-1"); self.emit("A=M"); self.emit("D=M"); self.emit("@R6"); self.emit("M=D");
        self.emit("@SP"); self.emit("M=M-1"); self.emit("A=M"); self.emit("D=M"); self.emit("@R5"); self.emit("M=D");
        // Advance ptr and col
        self.emit("@R5"); self.emit("M=M+1"); // ptr++
        self.emit("@R6"); self.emit("M=M+1"); // col++
        self.emit("@__ds_loop"); self.emit("0;JMP");
        self.emit("(__ds_done)");
        self.emit("@R4"); self.emit("A=M"); self.emit("0;JMP");
    }
}

pub fn generate(sema: SemaResult) -> Result<String, CodegenError> {
    let mut g = Gen::new(sema.func_sigs.clone(), sema.string_map.clone(), sema.struct_defs.clone());

    // Bootstrap
    g.emit("// Bootstrap");
    g.emit("@256");
    g.emit("D=A");
    g.emit("@SP");
    g.emit("M=D");

    // Initialize global variables BEFORE calling main (so main sees them)
    if !sema.globals.is_empty() {
        g.emit("// Global variable initialization");
        for (name, addr, _ty, init_val) in &sema.globals {
            if let Some(val) = init_val {
                g.emit(&format!("// init global {} at {}", name, addr));
                if *val == 0 {
                    // zero is the default; skip explicit write
                } else if *val == 1 {
                    g.emit("D=1");
                    g.emit(&format!("@{}", addr));
                    g.emit("M=D");
                } else if *val == -1 {
                    g.emit("D=-1");
                    g.emit(&format!("@{}", addr));
                    g.emit("M=D");
                } else if *val > 0 {
                    g.emit(&format!("@{}", val));
                    g.emit("D=A");
                    g.emit(&format!("@{}", addr));
                    g.emit("M=D");
                } else {
                    g.emit(&format!("@{}", -val));
                    g.emit("D=-A");
                    g.emit(&format!("@{}", addr));
                    g.emit("M=D");
                }
            }
        }
    }

    // Initialize string literals in RAM BEFORE calling main
    if !sema.string_literals.is_empty() {
        g.emit("// String literal initialization");
        for (addr, chars) in &sema.string_literals {
            for (i, &ch) in chars.iter().enumerate() {
                if ch == 0 { continue; } // null terminator: RAM is already 0
                g.emit(&format!("@{}", ch));
                g.emit("D=A");
                g.emit(&format!("@{}", addr + i));
                g.emit("M=D");
            }
            // Null terminator slot (addr+len) is already 0 (zero-init RAM)
        }
    }

    // Initialize font table at FONT_BASE (bit-reversed for Hack LSB-leftmost screen layout)
    g.emit("// Font table initialization");
    for ch_idx in 0..96usize {
        for row in 0..8usize {
            let byte = FONT_8X8[ch_idx][row];
            let reversed = byte.reverse_bits(); // MSB-first → LSB-first for Hack screen
            if reversed == 0 { continue; }
            let addr = FONT_BASE + ch_idx * 8 + row;
            g.emit(&format!("@{}", reversed));
            g.emit("D=A");
            g.emit(&format!("@{}", addr));
            g.emit("M=D");
        }
    }

    // Call main (using call convention)
    let id = g.label();
    let ret_lbl = format!("main$ret_{}", id);
    g.emit(&format!("@{}", ret_lbl));
    g.emit("D=A");
    g.emit("@SP");
    g.emit("A=M");
    g.emit("M=D");
    g.emit("@SP");
    g.emit("M=M+1");
    // push LCL=0, ARG=0, THIS=0, THAT=0
    for reg in &["LCL", "ARG", "THIS", "THAT"] {
        g.emit(&format!("@{}", reg));
        g.emit("D=M");
        g.emit("@SP");
        g.emit("A=M");
        g.emit("M=D");
        g.emit("@SP");
        g.emit("M=M+1");
    }
    // ARG = SP - 0 - 5
    g.emit("@SP");
    g.emit("D=M");
    g.emit("@5");
    g.emit("D=D-A");
    g.emit("@ARG");
    g.emit("M=D");
    // LCL = SP
    g.emit("@SP");
    g.emit("D=M");
    g.emit("@LCL");
    g.emit("M=D");
    g.emit("@main");
    g.emit("0;JMP");
    g.emit(&format!("({})", ret_lbl));
    // Infinite loop after main returns
    g.emit("(__end)");
    g.emit("@__end");
    g.emit("0;JMP");
    g.emit("");

    // Generate functions
    for f in &sema.funcs {
        g.emit("");
        g.gen_func(f)?;
    }

    // Runtime subroutines
    g.emit_runtime();

    Ok(g.out.join("\n") + "\n")
}

