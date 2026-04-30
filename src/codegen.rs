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

/// A single RAM pre-initialisation entry produced by the compiler.
#[derive(Debug, Clone)]
pub struct DataInit {
    pub address: u16,
    pub value: i16,
}

/// Full result of code generation returned by [`generate`].
pub struct CompiledProgram {
    /// Hack assembly text with bootstrap code (including data initialization).
    pub asm: String,
    /// RAM data initialisations for the font table only (globals/strings are
    /// initialized inline in the bootstrap code).
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
    string_map: HashMap<String, String>,
    struct_defs: HashMap<String, Vec<(String, Type)>>,
    loop_ctx: Vec<(String, String)>,   // (break_label, continue_label)
    /// When true, call sites and function returns jump to shared trampolines
    /// (__vm_call / __vm_return) instead of emitting inline sequences.
    use_trampolines: bool,
    need_call_trampoline: bool,
    need_return_trampoline: bool,
    func_return_types: HashMap<String, Type>,
    current_ret_ty: Type,
    need_return_long_trampoline: bool,
}

impl Gen {
    fn new(
        string_map: HashMap<String, String>,
        struct_defs: HashMap<String, Vec<(String, Type)>>,
        use_trampolines: bool,
        func_return_types: HashMap<String, Type>,
    ) -> Self {
        Self {
            out: Vec::new(),
            label_id: 0,
            string_map,
            struct_defs,
            loop_ctx: Vec::new(),
            use_trampolines,
            need_call_trampoline: false,
            need_return_trampoline: false,
            func_return_types,
            current_ret_ty: Type::Int,
            need_return_long_trampoline: false,
        }
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
            VarStorage::Global(sym) => {
                self.emit(&format!("@{}", sym));
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
            VarStorage::Global(sym) => {
                self.emit(&format!("@{}", sym));
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

    /// Emit D = val (any i16, avoiding @N for N > 32767)
    fn emit_d_load_i16(&mut self, val: i16) {
        match val {
            0 => self.emit("D=0"),
            1 => self.emit("D=1"),
            -1 => self.emit("D=-1"),
            v if v == i16::MIN => {
                self.emit("@32767");
                self.emit("D=-A");
                self.emit("D=D-1");
            }
            v if v > 0 => {
                self.emit(&format!("@{}", v));
                self.emit("D=A");
            }
            v => {
                self.emit(&format!("@{}", -(v as i32)));
                self.emit("D=-A");
            }
        }
    }

    /// Push hi then lo for a Long variable (lo ends up on top of stack)
    fn load_var_long(&mut self, info: &VarInfo) {
        match &info.storage {
            VarStorage::Local(idx) => {
                let idx = *idx;
                // push hi (idx)
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
                // push lo (idx+1)
                let idx1 = idx + 1;
                self.emit("@LCL");
                self.emit("D=M");
                self.emit(&format!("@{}", idx1));
                self.emit("A=D+A");
                self.emit("D=M");
                self.push_d();
            }
            VarStorage::Param(idx) => {
                let idx = *idx;
                // push hi (idx)
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
                // push lo (idx+1)
                let idx1 = idx + 1;
                self.emit("@ARG");
                self.emit("D=M");
                self.emit(&format!("@{}", idx1));
                self.emit("A=D+A");
                self.emit("D=M");
                self.push_d();
            }
            VarStorage::Global(sym) => {
                let sym = sym.clone();
                self.emit(&format!("@{}", sym));
                self.emit("D=M");
                self.push_d(); // hi
                let sym_lo = format!("{}_1", sym);
                self.emit(&format!("@{}", sym_lo));
                self.emit("D=M");
                self.push_d(); // lo
            }
        }
    }

    /// Store hi (R13) and lo (R14) to consecutive Long variable slots
    fn store_var_long_r13r14(&mut self, info: &VarInfo) {
        match &info.storage {
            VarStorage::Local(idx) => {
                let idx = *idx;
                // Store hi (R13) to LCL[idx]
                self.emit("@LCL");
                self.emit("D=M");
                if idx > 0 {
                    self.emit(&format!("@{}", idx));
                    self.emit("D=D+A");
                }
                self.emit("@R15");
                self.emit("M=D");
                self.emit("@R13");
                self.emit("D=M");
                self.emit("@R15");
                self.emit("A=M");
                self.emit("M=D");
                // Store lo (R14) to LCL[idx+1]
                let idx1 = idx + 1;
                self.emit("@LCL");
                self.emit("D=M");
                self.emit(&format!("@{}", idx1));
                self.emit("D=D+A");
                self.emit("@R15");
                self.emit("M=D");
                self.emit("@R14");
                self.emit("D=M");
                self.emit("@R15");
                self.emit("A=M");
                self.emit("M=D");
            }
            VarStorage::Param(idx) => {
                let idx = *idx;
                // Store hi (R13) to ARG[idx]
                self.emit("@ARG");
                self.emit("D=M");
                if idx > 0 {
                    self.emit(&format!("@{}", idx));
                    self.emit("D=D+A");
                }
                self.emit("@R15");
                self.emit("M=D");
                self.emit("@R13");
                self.emit("D=M");
                self.emit("@R15");
                self.emit("A=M");
                self.emit("M=D");
                // Store lo (R14) to ARG[idx+1]
                let idx1 = idx + 1;
                self.emit("@ARG");
                self.emit("D=M");
                self.emit(&format!("@{}", idx1));
                self.emit("D=D+A");
                self.emit("@R15");
                self.emit("M=D");
                self.emit("@R14");
                self.emit("D=M");
                self.emit("@R15");
                self.emit("A=M");
                self.emit("M=D");
            }
            VarStorage::Global(sym) => {
                let sym = sym.clone();
                self.emit("@R13");
                self.emit("D=M");
                self.emit(&format!("@{}", sym));
                self.emit("M=D");
                self.emit("@R14");
                self.emit("D=M");
                self.emit(&format!("@{}_1", sym));
                self.emit("M=D");
            }
        }
    }

    /// Load a 2-word Long from address held in D.
    /// Saves addr to R13. Stack after: [..., hi, lo] with lo on top.
    fn load_long_from_addr_d(&mut self) {
        self.emit("@R13");
        self.emit("M=D");       // R13 = addr
        self.emit("A=D");
        self.emit("D=M");       // D = hi = mem[addr]
        self.push_d();
        self.emit("@R13");
        self.emit("D=M+1");     // D = addr+1
        self.emit("A=D");
        self.emit("D=M");       // D = lo = mem[addr+1]
        self.push_d();
    }

    /// Store Long (R5=hi, R6=lo) to address held in D. Uses R15 as scratch.
    fn store_long_r5r6_at_addr_d(&mut self) {
        self.emit("@R15");
        self.emit("M=D");       // R15 = addr
        self.emit("@R5");
        self.emit("D=M");       // D = hi
        self.emit("@R15");
        self.emit("A=M");
        self.emit("M=D");       // mem[addr] = hi
        self.emit("@R6");
        self.emit("D=M");       // D = lo
        self.emit("@R15");
        self.emit("A=M+1");     // A = addr+1
        self.emit("M=D");       // mem[addr+1] = lo
    }

    /// Sign-extend top-of-stack (1-word int) to 2-word Long.
    /// Stack before: [... val]; after: [... hi lo] with lo on top.
    fn sign_extend_to_long(&mut self) {
        self.pop_d(); // D = value (lo word)
        self.emit("@R13");
        self.emit("M=D"); // R13 = lo
        let id = self.label();
        let l_neg = format!("__signext_n_{}", id);
        let l_end = format!("__signext_e_{}", id);
        self.emit(&format!("@{}", l_neg));
        self.emit("D;JLT");
        self.emit("D=0");
        self.emit(&format!("@{}", l_end));
        self.emit("0;JMP");
        self.emit(&format!("({})", l_neg));
        self.emit("D=-1");
        self.emit(&format!("({})", l_end));
        self.push_d(); // push hi
        self.emit("@R13");
        self.emit("D=M");
        self.push_d(); // push lo
    }

    /// Pop lo then hi from stack (lo on top), store to R5=hi, R6=lo
    fn pop_long_to_r56(&mut self) {
        self.pop_d();
        self.emit("@R6");
        self.emit("M=D"); // R6 = lo
        self.pop_d();
        self.emit("@R5");
        self.emit("M=D"); // R5 = hi
    }

    /// Pop two longs: b on top [lo_b, hi_b, lo_a, hi_a from top to bottom]
    /// Results: R5=hi_a, R6=lo_a, R7=hi_b, R8=lo_b
    fn pop_long_pair_to_r5678(&mut self) {
        self.pop_d(); self.emit("@R8"); self.emit("M=D"); // lo_b
        self.pop_d(); self.emit("@R7"); self.emit("M=D"); // hi_b
        self.pop_d(); self.emit("@R6"); self.emit("M=D"); // lo_a
        self.pop_d(); self.emit("@R5"); self.emit("M=D"); // hi_a
    }

    /// Push R5:R6 as Long (hi=R5, lo=R6)
    fn push_long_from_r56(&mut self) {
        self.emit("@R5"); self.emit("D=M"); self.push_d();
        self.emit("@R6"); self.emit("D=M"); self.push_d();
    }

    /// Call an R3-convention helper (set up return label, jump to helper)
    fn call_r3_helper(&mut self, name: &str) {
        let id = self.label();
        let ret = format!("{}_ret_{}", name, id);
        self.emit(&format!("@{}", ret));
        self.emit("D=A");
        self.emit("@R3");
        self.emit("M=D");
        self.emit(&format!("@{}", name));
        self.emit("0;JMP");
        self.emit(&format!("({})", ret));
    }

    /// Evaluate expr and ensure 2 words on stack (sign-extend if needed)
    fn gen_expr_as_long(&mut self, expr: &Expr, vars: &HashMap<String, VarInfo>) -> Result<(), CodegenError> {
        let ty = self.expr_type(expr, vars).unwrap_or(Type::Int);
        self.gen_expr(expr, vars)?;
        if !matches!(ty, Type::Long) {
            self.sign_extend_to_long();
        }
        Ok(())
    }

    /// Get the type of an lvalue expression
    fn lvalue_type(&self, expr: &Expr, vars: &HashMap<String, VarInfo>) -> Option<Type> {
        match expr {
            Expr::Ident(name) => vars.get(name).map(|v| v.ty.clone()),
            Expr::UnOp(UnOp::Deref, inner) => match self.expr_type(inner, vars)? {
                Type::Ptr(t) => Some(*t),
                Type::Array(t, _) => Some(*t),
                _ => None,
            },
            Expr::Index(base, _) => match self.expr_type(base, vars)? {
                Type::Ptr(t) => Some(*t),
                Type::Array(t, _) => Some(*t),
                _ => None,
            },
            Expr::Member(base, field) => {
                if let Some(Type::Struct(sname)) = self.expr_type(base, vars) {
                    self.struct_defs.get(&sname)
                        .and_then(|fields| fields.iter().find(|(fn_, _)| fn_ == field))
                        .map(|(_, ty)| ty.clone())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Evaluate condition expr; collapses Long to single word in D (hi|lo).
    /// After this, D is 0 for false, nonzero for true.
    fn gen_cond_d(&mut self, cond_expr: &Expr, vars: &HashMap<String, VarInfo>) -> Result<(), CodegenError> {
        let ty = self.expr_type(cond_expr, vars).unwrap_or(Type::Int);
        self.gen_expr(cond_expr, vars)?;
        if matches!(ty, Type::Long) {
            self.pop_d();
            self.emit("@R13");
            self.emit("M=D");  // R13 = lo
            self.pop_d();      // D = hi
            self.emit("@R13");
            self.emit("D=D|M"); // D = hi | lo
        } else {
            self.pop_d();
        }
        Ok(())
    }

    /// Infer the type of an expression without generating code.
    fn expr_type(&self, expr: &Expr, vars: &HashMap<String, VarInfo>) -> Option<Type> {
        match expr {
            Expr::Num(n) => {
                let n = *n;
                if n > i16::MAX as i32 || n < i16::MIN as i32 {
                    Some(Type::Long)
                } else {
                    Some(Type::Int)
                }
            }
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
            Expr::BinOp(op, lhs, rhs) => {
                match op {
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le
                    | BinOp::Gt | BinOp::Ge | BinOp::And | BinOp::Or => Some(Type::Int),
                    _ => {
                        let lt = self.expr_type(lhs, vars)?;
                        let rt = self.expr_type(rhs, vars)?;
                        if matches!(lt, Type::Long) || matches!(rt, Type::Long) {
                            Some(Type::Long)
                        } else {
                            Some(lt)
                        }
                    }
                }
            }
            Expr::Call(name, _) => self.func_return_types.get(name).cloned(),
            Expr::Cast(ty, _) => Some(ty.clone()),
            Expr::Ternary(_, then_e, _) => self.expr_type(then_e, vars),
            Expr::PostInc(inner) | Expr::PostDec(inner) => self.expr_type(inner, vars),
            Expr::UnOp(UnOp::Neg, inner) | Expr::UnOp(UnOp::BitNot, inner) => self.expr_type(inner, vars),
            Expr::UnOp(UnOp::Not, _) => Some(Type::Int),
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
                if n > i16::MAX as i32 || n < i16::MIN as i32 {
                    // Large value: push as Long (2 words, hi then lo)
                    let hi = ((n as u32 >> 16) & 0xFFFF) as i16;
                    let lo = (n as u16) as i16;
                    self.emit_d_load_i16(hi);
                    self.push_d();
                    self.emit_d_load_i16(lo);
                    self.push_d();
                } else if n == 0 {
                    self.emit("D=0");
                    self.push_d();
                } else if n == 1 {
                    self.emit("D=1");
                    self.push_d();
                } else if n == -1 {
                    self.emit("D=-1");
                    self.push_d();
                } else if n > 0 {
                    self.emit(&format!("@{}", n));
                    self.emit("D=A");
                    self.push_d();
                } else {
                    self.emit(&format!("@{}", -n));
                    self.emit("D=-A");
                    self.push_d();
                }
            }

            Expr::Sizeof(ty) => {
                let sz = self.type_size(ty).max(1) as i32;
                self.emit(&format!("@{}", sz));
                self.emit("D=A");
                self.push_d();
            }

            Expr::StringLit(s) => {
                let sym = self.string_map.get(s).ok_or_else(|| {
                    CodegenError::new(format!("unknown string literal {:?}", s))
                })?.clone();
                self.emit(&format!("@{}", sym));
                self.emit("D=A");
                self.push_d();
            }

            Expr::Ident(name) => {
                let info = vars.get(name).ok_or_else(|| {
                    CodegenError::new(format!("undefined variable '{}'", name))
                })?.clone();
                if matches!(info.ty, crate::parser::Type::Array(..)) {
                    self.addr_of_var(&info);
                } else if matches!(info.ty, Type::Long) {
                    self.load_var_long(&info);
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
                        let pointee_ty = self.expr_type(inner, vars)
                            .and_then(|t| if let Type::Ptr(e) = t { Some(*e) } else { None });
                        self.gen_expr(inner, vars)?;
                        self.pop_d();
                        if matches!(pointee_ty, Some(Type::Long)) {
                            self.load_long_from_addr_d();
                        } else {
                            self.emit("A=D");
                            self.emit("D=M");
                            self.push_d();
                        }
                    }
                    UnOp::Neg => {
                        let inner_ty = self.expr_type(inner, vars).unwrap_or(Type::Int);
                        self.gen_expr(inner, vars)?;
                        if matches!(inner_ty, Type::Long) {
                            self.pop_long_to_r56();
                            self.call_r3_helper("__lneg");
                            self.push_long_from_r56();
                        } else {
                            self.pop_d();
                            self.emit("D=-D");
                            self.push_d();
                        }
                    }
                    UnOp::Not => {
                        let inner_ty = self.expr_type(inner, vars).unwrap_or(Type::Int);
                        self.gen_expr(inner, vars)?;
                        if matches!(inner_ty, Type::Long) {
                            // Collapse 2 words: if hi|lo != 0, result is 0; else 1
                            self.pop_d();
                            self.emit("@R13");
                            self.emit("M=D"); // R13 = lo
                            self.pop_d();
                            self.emit("@R13");
                            self.emit("D=D|M"); // D = hi | lo
                        } else {
                            self.pop_d();
                        }
                        // Now D = value to test
                        let id = self.label();
                        let lfalse = format!("__not_f_{}", id);
                        let lend   = format!("__not_e_{}", id);
                        self.emit(&format!("@{}", lfalse));
                        self.emit("D;JNE");
                        self.emit("D=1");
                        self.emit(&format!("@{}", lend));
                        self.emit("0;JMP");
                        self.emit(&format!("({})", lfalse));
                        self.emit("D=0");
                        self.emit(&format!("({})", lend));
                        self.push_d();
                    }
                    UnOp::BitNot => {
                        let inner_ty = self.expr_type(inner, vars).unwrap_or(Type::Int);
                        self.gen_expr(inner, vars)?;
                        if matches!(inner_ty, Type::Long) {
                            // Stack: [hi, lo] with lo on top
                            self.pop_d();
                            self.emit("D=!D");
                            self.emit("@R14");
                            self.emit("M=D"); // R14 = ~lo
                            self.pop_d();
                            self.emit("D=!D");
                            self.push_d(); // push ~hi
                            self.emit("@R14");
                            self.emit("D=M");
                            self.push_d(); // push ~lo
                        } else {
                            self.pop_d();
                            self.emit("D=!D");
                            self.push_d();
                        }
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
                let elem_is_long = matches!(
                    &base_ty,
                    Some(Type::Array(e, _)) | Some(Type::Ptr(e)) if matches!(e.as_ref(), Type::Long)
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
                } else if elem_is_long {
                    self.load_long_from_addr_d();
                } else {
                    self.emit("A=D");
                    self.emit("D=M");   // D = value at address
                    self.push_d();
                }
            }

            Expr::Member(_, _) => {
                // Load value at field address: gen_addr gives the address, then deref
                let field_ty = self.lvalue_type(expr, vars);
                self.gen_addr(expr, vars)?;
                self.pop_d();
                if matches!(field_ty, Some(Type::Long)) {
                    self.load_long_from_addr_d();
                } else {
                    self.emit("A=D");
                    self.emit("D=M");
                    self.push_d();
                }
            }

            Expr::Ternary(cond, then_e, else_e) => {
                let id = self.label();
                let l_false = format!("__tern_f_{}", id);
                let l_end   = format!("__tern_e_{}", id);
                // Evaluate condition - handle Long condition
                let cond_ty = self.expr_type(cond, vars).unwrap_or(Type::Int);
                self.gen_expr(cond, vars)?;
                if matches!(cond_ty, Type::Long) {
                    self.pop_d();
                    self.emit("@R13");
                    self.emit("M=D");
                    self.pop_d();
                    self.emit("@R13");
                    self.emit("D=D|M");
                } else {
                    self.pop_d();
                }
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
                let inner_ty = self.lvalue_type(inner, vars);
                if matches!(inner_ty, Some(Type::Long)) {
                    self.gen_addr(inner, vars)?;
                    self.pop_d();
                    self.emit("@R15"); self.emit("M=D");    // R15 = address
                    self.emit("A=D");  self.emit("D=M");    // D = old hi
                    self.emit("@R5");  self.emit("M=D");    // R5 = old hi
                    self.push_d();                           // push old hi (result)
                    self.emit("@R15"); self.emit("A=M+1"); self.emit("D=M"); // D = old lo
                    self.emit("@R6");  self.emit("M=D");    // R6 = old lo
                    self.push_d();                           // push old lo (result)
                    // compute old + 1 using __ladd: R5/R6 already set, R7=0, R8=1
                    self.emit("@R7");  self.emit("M=0");
                    self.emit("@R8");  self.emit("M=1");
                    self.call_r3_helper("__ladd");           // new value in R5/R6
                    self.emit("@R5"); self.emit("D=M");
                    self.emit("@R15"); self.emit("A=M");  self.emit("M=D");  // mem[addr] = new hi
                    self.emit("@R6"); self.emit("D=M");
                    self.emit("@R15"); self.emit("A=M+1"); self.emit("M=D"); // mem[addr+1] = new lo
                } else {
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
            }
            Expr::PostDec(inner) => {
                let inner_ty = self.lvalue_type(inner, vars);
                if matches!(inner_ty, Some(Type::Long)) {
                    self.gen_addr(inner, vars)?;
                    self.pop_d();
                    self.emit("@R15"); self.emit("M=D");    // R15 = address
                    self.emit("A=D");  self.emit("D=M");    // D = old hi
                    self.emit("@R5");  self.emit("M=D");    // R5 = old hi
                    self.push_d();                           // push old hi (result)
                    self.emit("@R15"); self.emit("A=M+1"); self.emit("D=M"); // D = old lo
                    self.emit("@R6");  self.emit("M=D");    // R6 = old lo
                    self.push_d();                           // push old lo (result)
                    // compute old - 1 using __lsub: R5/R6 already set, R7=0, R8=1
                    self.emit("@R7");  self.emit("M=0");
                    self.emit("@R8");  self.emit("M=1");
                    self.call_r3_helper("__lsub");           // new value in R5/R6
                    self.emit("@R5"); self.emit("D=M");
                    self.emit("@R15"); self.emit("A=M");  self.emit("M=D");  // mem[addr] = new hi
                    self.emit("@R6"); self.emit("D=M");
                    self.emit("@R15"); self.emit("A=M+1"); self.emit("M=D"); // mem[addr+1] = new lo
                } else {
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
            }
            Expr::Cast(ty, inner) => {
                let inner_ty = self.expr_type(inner, vars).unwrap_or(Type::Int);
                match ty {
                    Type::Long => {
                        if matches!(inner_ty, Type::Long) {
                            self.gen_expr(inner, vars)?;
                        } else {
                            self.gen_expr(inner, vars)?;
                            self.sign_extend_to_long();
                        }
                    }
                    Type::Char => {
                        self.gen_expr(inner, vars)?;
                        if matches!(inner_ty, Type::Long) {
                            // Take lo word, discard hi
                            self.pop_d();
                            self.emit("@R13");
                            self.emit("M=D"); // save lo
                            self.pop_d();     // discard hi
                            self.emit("@R13");
                            self.emit("D=M");
                            self.emit("@255");
                            self.emit("D=D&A");
                            self.push_d();
                        } else {
                            self.pop_d();
                            self.emit("@255");
                            self.emit("D=D&A");
                            self.push_d();
                        }
                    }
                    _ => {
                        self.gen_expr(inner, vars)?;
                        if matches!(inner_ty, Type::Long) {
                            // Long -> narrower: take lo word
                            self.pop_d();     // lo (on top)
                            self.emit("@R13");
                            self.emit("M=D");
                            self.pop_d();     // hi (discard)
                            self.emit("@R13");
                            self.emit("D=M");
                            self.push_d();
                        }
                        // else no-op
                    }
                }
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

        // Check for Long binary operation
        let lhs_ty = self.expr_type(lhs, vars).unwrap_or(Type::Int);
        let rhs_ty = self.expr_type(rhs, vars).unwrap_or(Type::Int);
        if matches!(lhs_ty, Type::Long) || matches!(rhs_ty, Type::Long) {
            return self.gen_binop_long(op, lhs, rhs, vars);
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
                // Arithmetic right shift (sign-extending).
                // D = lhs (already on stack), R14 = n (rhs).
                let id = self.label();
                let l_sat      = format!("__shr_sat_{}", id);
                let l_sat_neg  = format!("__shr_sat_neg_{}", id);
                let l_normal   = format!("__shr_normal_{}", id);
                let l_pow_loop = format!("__shr_pow_{}", id);
                let l_pow_end  = format!("__shr_pow_end_{}", id);
                let l_end      = format!("__shr_end_{}", id);
                let ret_lbl    = format!("__shr_div_ret_{}", id);
                self.emit("@R13");
                self.emit("M=D");           // R13 = lhs
                // If n >= 15: saturate (sign extension fills all bits)
                self.emit("@R14");
                self.emit("D=M");           // D = n
                self.emit("@15");
                self.emit("D=D-A");
                self.emit(&format!("@{}", l_normal));
                self.emit("D;JLT");         // n < 15 → normal path
                self.emit(&format!("({})", l_sat));
                // n >= 15: result = (lhs < 0) ? -1 : 0
                self.emit("@R13");
                self.emit("D=M");
                self.emit(&format!("@{}", l_sat_neg));
                self.emit("D;JLT");
                self.emit("D=0");
                self.emit("@R13");
                self.emit("M=D");
                self.emit(&format!("@{}", l_end));
                self.emit("0;JMP");
                self.emit(&format!("({})", l_sat_neg));
                self.emit("D=-1");
                self.emit("@R13");
                self.emit("M=D");
                self.emit(&format!("@{}", l_end));
                self.emit("0;JMP");
                // Normal path: n < 15, compute 2^n in R14
                self.emit(&format!("({})", l_normal));
                self.emit("@R14");
                self.emit("D=M");           // D = n
                self.emit("@R15");
                self.emit("M=D");           // R15 = n (loop counter)
                self.emit("@R14");
                self.emit("M=1");           // R14 = 1 (will become 2^n)
                self.emit(&format!("({})", l_pow_loop));
                self.emit("@R15");
                self.emit("D=M");
                self.emit(&format!("@{}", l_pow_end));
                self.emit("D;JEQ");
                self.emit("@R14");
                self.emit("D=M");
                self.emit("M=D+M");         // R14 *= 2
                self.emit("@R15");
                self.emit("M=M-1");
                self.emit(&format!("@{}", l_pow_loop));
                self.emit("0;JMP");
                self.emit(&format!("({})", l_pow_end));
                // Call __div: quotient → R13, remainder → R15
                self.emit(&format!("@{}", ret_lbl));
                self.emit("D=A");
                self.emit("@R3");
                self.emit("M=D");
                self.emit("@__div");
                self.emit("0;JMP");
                self.emit(&format!("({})", ret_lbl));
                // Floor adjustment: if remainder (R15) < 0, decrement quotient.
                // Avoids the pre-adjustment overflow at lhs = INT16_MIN.
                self.emit("@R15");
                self.emit("D=M");
                self.emit(&format!("@{}", l_end));
                self.emit("D;JGE");         // remainder >= 0: already exact or positive
                self.emit("@R13");
                self.emit("M=M-1");         // floor correction
                self.emit(&format!("({})", l_end));
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

    fn gen_binop_long(&mut self, op: &BinOp, lhs: &Expr, rhs: &Expr, vars: &HashMap<String, VarInfo>) -> Result<(), CodegenError> {
        self.gen_expr_as_long(lhs, vars)?;
        self.gen_expr_as_long(rhs, vars)?;
        // Stack: [hi_a, lo_a, hi_b, lo_b] with lo_b on top

        match op {
            BinOp::Add => {
                self.pop_long_pair_to_r5678();
                self.call_r3_helper("__ladd");
                self.push_long_from_r56();
            }
            BinOp::Sub => {
                self.pop_long_pair_to_r5678();
                self.call_r3_helper("__lsub");
                self.push_long_from_r56();
            }
            BinOp::Mul => {
                self.pop_long_pair_to_r5678();
                self.call_r3_helper("__lmul");
                self.push_long_from_r56();
            }
            BinOp::Div => {
                self.pop_long_pair_to_r5678();
                self.call_r3_helper("__ldiv");
                self.push_long_from_r56();
            }
            BinOp::Mod => {
                self.pop_long_pair_to_r5678();
                self.call_r3_helper("__ldiv");
                // remainder in R9:R10
                self.emit("@R9"); self.emit("D=M"); self.push_d();
                self.emit("@R10"); self.emit("D=M"); self.push_d();
            }
            BinOp::BitAnd => {
                self.pop_long_pair_to_r5678();
                self.emit("@R5"); self.emit("D=M"); self.emit("@R7"); self.emit("D=D&M"); self.emit("@R5"); self.emit("M=D");
                self.emit("@R6"); self.emit("D=M"); self.emit("@R8"); self.emit("D=D&M"); self.emit("@R6"); self.emit("M=D");
                self.push_long_from_r56();
            }
            BinOp::BitOr => {
                self.pop_long_pair_to_r5678();
                self.emit("@R5"); self.emit("D=M"); self.emit("@R7"); self.emit("D=D|M"); self.emit("@R5"); self.emit("M=D");
                self.emit("@R6"); self.emit("D=M"); self.emit("@R8"); self.emit("D=D|M"); self.emit("@R6"); self.emit("M=D");
                self.push_long_from_r56();
            }
            BinOp::BitXor => {
                self.pop_long_pair_to_r5678();
                // XOR = (a|b) & ~(a&b) per word
                // hi:
                self.emit("@R5"); self.emit("D=M"); self.emit("@R7"); self.emit("D=D&M"); self.emit("D=!D"); self.emit("@R9"); self.emit("M=D");
                self.emit("@R5"); self.emit("D=M"); self.emit("@R7"); self.emit("D=D|M"); self.emit("@R9"); self.emit("D=D&M"); self.emit("@R5"); self.emit("M=D");
                // lo:
                self.emit("@R6"); self.emit("D=M"); self.emit("@R8"); self.emit("D=D&M"); self.emit("D=!D"); self.emit("@R9"); self.emit("M=D");
                self.emit("@R6"); self.emit("D=M"); self.emit("@R8"); self.emit("D=D|M"); self.emit("@R9"); self.emit("D=D&M"); self.emit("@R6"); self.emit("M=D");
                self.push_long_from_r56();
            }
            BinOp::Eq | BinOp::Ne => {
                self.pop_long_pair_to_r5678();
                let id = self.label();
                let l_nomatch = format!("__lcmp_nm_{}", id);
                let l_end = format!("__lcmp_e_{}", id);
                // check hi
                self.emit("@R5"); self.emit("D=M"); self.emit("@R7"); self.emit("D=D-M");
                self.emit(&format!("@{}", l_nomatch)); self.emit("D;JNE");
                // check lo
                self.emit("@R6"); self.emit("D=M"); self.emit("@R8"); self.emit("D=D-M");
                self.emit(&format!("@{}", l_nomatch)); self.emit("D;JNE");
                // match
                let match_val: i32 = if matches!(op, BinOp::Eq) { 1 } else { 0 };
                let nomatch_val: i32 = if matches!(op, BinOp::Eq) { 0 } else { 1 };
                self.emit(&format!("D={}", match_val));
                self.emit(&format!("@{}", l_end)); self.emit("0;JMP");
                self.emit(&format!("({})", l_nomatch));
                self.emit(&format!("D={}", nomatch_val));
                self.emit(&format!("({})", l_end));
                self.push_d();
            }
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                self.pop_long_pair_to_r5678();
                // For GT/GE: swap a (R5:R6) and b (R7:R8), then use LT/LE logic
                if matches!(op, BinOp::Gt | BinOp::Ge) {
                    self.emit("@R5"); self.emit("D=M"); self.emit("@R9"); self.emit("M=D");
                    self.emit("@R7"); self.emit("D=M"); self.emit("@R5"); self.emit("M=D");
                    self.emit("@R9"); self.emit("D=M"); self.emit("@R7"); self.emit("M=D");
                    self.emit("@R6"); self.emit("D=M"); self.emit("@R9"); self.emit("M=D");
                    self.emit("@R8"); self.emit("D=M"); self.emit("@R6"); self.emit("M=D");
                    self.emit("@R9"); self.emit("D=M"); self.emit("@R8"); self.emit("M=D");
                }
                let use_le = matches!(op, BinOp::Le | BinOp::Ge);
                let id = self.label();
                let l_true = format!("__llt_t_{}", id);
                let l_false = format!("__llt_f_{}", id);
                let l_end = format!("__llt_e_{}", id);
                let l_la_neg = format!("__llt_lan_{}", id);
                let l_lb_neg = format!("__llt_lbn_{}", id);
                let l_lb_neg2 = format!("__llt_lbn2_{}", id);
                let l_ha_neg = format!("__llt_han_{}", id);
                let l_ha_pos_b_neg = format!("__llt_hapbn_{}", id);
                let l_ha_safe_sub = format!("__llt_hss_{}", id);
                // Compare hi signed (overflow-safe: check signs before subtracting)
                self.emit("@R5"); self.emit("D=M");
                self.emit(&format!("@{}", l_ha_neg)); self.emit("D;JLT");
                // hi_a >= 0: check hi_b sign
                self.emit("@R7"); self.emit("D=M");
                self.emit(&format!("@{}", l_ha_pos_b_neg)); self.emit("D;JLT");
                // both hi >= 0: safe subtraction
                self.emit("@R5"); self.emit("D=M"); self.emit("@R7"); self.emit("D=D-M");
                self.emit(&format!("@{}", l_true)); self.emit("D;JLT");
                self.emit(&format!("@{}", l_false)); self.emit("D;JGT");
                self.emit(&format!("@{}", l_ha_safe_sub)); self.emit("0;JMP");
                // hi_a >= 0, hi_b < 0: a > b → not Lt
                self.emit(&format!("({})", l_ha_pos_b_neg));
                self.emit(&format!("@{}", l_false)); self.emit("0;JMP");
                // hi_a < 0: check hi_b sign
                self.emit(&format!("({})", l_ha_neg));
                self.emit("@R7"); self.emit("D=M");
                self.emit(&format!("@{}", l_true)); self.emit("D;JGE"); // hi_a<0,hi_b>=0: a<b
                // both hi < 0: safe subtraction
                self.emit("@R5"); self.emit("D=M"); self.emit("@R7"); self.emit("D=D-M");
                self.emit(&format!("@{}", l_true)); self.emit("D;JLT");
                self.emit(&format!("@{}", l_false)); self.emit("D;JGT");
                // hi equal (both same sign, D==0): compare lo unsigned
                self.emit(&format!("({})", l_ha_safe_sub));
                // hi equal: compare lo unsigned
                self.emit("@R6"); self.emit("D=M");
                self.emit(&format!("@{}", l_la_neg)); self.emit("D;JLT");
                // lo_a >= 0
                self.emit("@R8"); self.emit("D=M");
                self.emit(&format!("@{}", l_lb_neg)); self.emit("D;JLT");
                // both >= 0: signed comparison works
                self.emit("@R6"); self.emit("D=M"); self.emit("@R8"); self.emit("D=D-M");
                if use_le {
                    self.emit(&format!("@{}", l_true)); self.emit("D;JLE");
                } else {
                    self.emit(&format!("@{}", l_true)); self.emit("D;JLT");
                }
                self.emit(&format!("@{}", l_false)); self.emit("0;JMP");
                self.emit(&format!("({})", l_lb_neg));
                // lo_a >= 0, lo_b < 0: unsigned lo_b >= 32768 > lo_a -> a < b
                self.emit(&format!("@{}", l_true)); self.emit("0;JMP");
                self.emit(&format!("({})", l_la_neg));
                // lo_a < 0
                self.emit("@R8"); self.emit("D=M");
                self.emit(&format!("@{}", l_lb_neg2)); self.emit("D;JLT");
                // lo_a < 0, lo_b >= 0: unsigned lo_a >= 32768 > lo_b -> a > b
                self.emit(&format!("@{}", l_false)); self.emit("0;JMP");
                self.emit(&format!("({})", l_lb_neg2));
                // both < 0: signed comparison works
                self.emit("@R6"); self.emit("D=M"); self.emit("@R8"); self.emit("D=D-M");
                if use_le {
                    self.emit(&format!("@{}", l_true)); self.emit("D;JLE");
                } else {
                    self.emit(&format!("@{}", l_true)); self.emit("D;JLT");
                }
                self.emit(&format!("({})", l_false));
                self.emit("D=0");
                self.emit(&format!("@{}", l_end)); self.emit("0;JMP");
                self.emit(&format!("({})", l_true));
                self.emit("D=1");
                self.emit(&format!("({})", l_end));
                self.push_d();
            }
            BinOp::Shl => {
                self.pop_long_pair_to_r5678();
                // shift count is lo word of rhs (R8); hi word (R7) ignored
                self.call_r3_helper("__lshl");
                self.push_long_from_r56();
            }
            BinOp::Shr => {
                self.pop_long_pair_to_r5678();
                // shift count is lo word of rhs (R8); hi word (R7) ignored
                self.call_r3_helper("__lshr");
                self.push_long_from_r56();
            }
            _ => return Err(CodegenError::new(format!("long binop {:?} not supported", op))),
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
        // Evaluate lhs, collapse Long to boolean
        let lhs_ty = self.expr_type(lhs, vars).unwrap_or(Type::Int);
        self.gen_expr(lhs, vars)?;
        if matches!(lhs_ty, Type::Long) {
            self.pop_d();
            self.emit("@R13"); self.emit("M=D");
            self.pop_d();
            self.emit("@R13"); self.emit("D=D|M");
        } else {
            self.pop_d();
        }
        self.emit(&format!("@{}", l_false));
        self.emit("D;JEQ");
        // Evaluate rhs, collapse Long to boolean
        let rhs_ty = self.expr_type(rhs, vars).unwrap_or(Type::Int);
        self.gen_expr(rhs, vars)?;
        if matches!(rhs_ty, Type::Long) {
            self.pop_d();
            self.emit("@R13"); self.emit("M=D");
            self.pop_d();
            self.emit("@R13"); self.emit("D=D|M");
        } else {
            self.pop_d();
        }
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
        let lhs_ty = self.expr_type(lhs, vars).unwrap_or(Type::Int);
        self.gen_expr(lhs, vars)?;
        if matches!(lhs_ty, Type::Long) {
            self.pop_d();
            self.emit("@R13"); self.emit("M=D");
            self.pop_d();
            self.emit("@R13"); self.emit("D=D|M");
        } else {
            self.pop_d();
        }
        self.emit(&format!("@{}", l_true));
        self.emit("D;JNE");
        let rhs_ty = self.expr_type(rhs, vars).unwrap_or(Type::Int);
        self.gen_expr(rhs, vars)?;
        if matches!(rhs_ty, Type::Long) {
            self.pop_d();
            self.emit("@R13"); self.emit("M=D");
            self.pop_d();
            self.emit("@R13"); self.emit("D=D|M");
        } else {
            self.pop_d();
        }
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
            VarStorage::Global(sym) => {
                // R13 has the value; store directly to named global
                self.emit("@R13");
                self.emit("D=M");
                self.emit(&format!("@{}", sym));
                self.emit("M=D");
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
        let lhs_ty = self.lvalue_type(lhs, vars);
        let rhs_ty = self.expr_type(rhs, vars).unwrap_or(Type::Int);

        if matches!(lhs_ty, Some(Type::Long)) {
            // Long lhs: need 2 words
            self.gen_expr(rhs, vars)?;
            if !matches!(rhs_ty, Type::Long) {
                self.sign_extend_to_long();
            }
            // Stack: [hi, lo] with lo on top
            match lhs {
                Expr::Ident(name) => {
                    let info = vars.get(name).ok_or_else(|| {
                        CodegenError::new(format!("undefined variable '{}'", name))
                    })?.clone();
                    self.pop_d();
                    self.emit("@R14");
                    self.emit("M=D"); // R14 = lo
                    self.pop_d();
                    self.emit("@R13");
                    self.emit("M=D"); // R13 = hi
                    self.store_var_long_r13r14(&info);
                    // Result of assignment expression: push back hi then lo
                    self.emit("@R13");
                    self.emit("D=M");
                    self.push_d();
                    self.emit("@R14");
                    self.emit("D=M");
                    self.push_d();
                }
                _ => {
                    // Deref/Index/Member: stash hi/lo in R5/R6, compute address, store
                    self.pop_d();
                    self.emit("@R6");
                    self.emit("M=D"); // R6 = lo
                    self.pop_d();
                    self.emit("@R5");
                    self.emit("M=D"); // R5 = hi
                    match lhs {
                        Expr::UnOp(UnOp::Deref, ptr_expr) => {
                            self.gen_expr(ptr_expr, vars)?;
                            self.pop_d();
                            self.store_long_r5r6_at_addr_d();
                        }
                        Expr::Index(base, idx) => {
                            let base_ty = self.expr_type(base, vars);
                            let stride = match &base_ty {
                                Some(Type::Array(e, _)) | Some(Type::Ptr(e)) => self.type_size(e).max(1),
                                _ => 2,
                            };
                            self.gen_expr(base, vars)?;
                            self.gen_expr(idx, vars)?;
                            self.pop_d();
                            if stride == 1 {
                                self.emit("@R14");
                                self.emit("M=D");
                                self.pop_d();
                                self.emit("@R14");
                                self.emit("D=D+M");
                            } else {
                                self.emit_stride_mul(stride); // D = idx*stride; safe, uses R13/R14/R15
                                self.emit("@R14");
                                self.emit("M=D");
                                self.pop_d();
                                self.emit("@R14");
                                self.emit("D=D+M");
                            }
                            self.store_long_r5r6_at_addr_d();
                        }
                        Expr::Member(base, field) => {
                            let base_ty = self.expr_type(base, vars)
                                .ok_or_else(|| CodegenError::new("cannot determine type for member assignment"))?;
                            let struct_name = match &base_ty {
                                Type::Struct(name) => name.clone(),
                                _ => return Err(CodegenError::new(
                                    format!("member access on non-struct type {:?}", base_ty)
                                )),
                            };
                            let offset = self.field_offset(&struct_name, field)?;
                            self.gen_addr(base, vars)?;
                            self.pop_d();
                            if offset > 0 {
                                self.emit(&format!("@{}", offset));
                                self.emit("D=D+A");
                            }
                            self.store_long_r5r6_at_addr_d();
                        }
                        _ => return Err(CodegenError::new("long assignment to this lvalue not supported")),
                    }
                    self.emit("@R5");
                    self.emit("D=M");
                    self.push_d();
                    self.emit("@R6");
                    self.emit("D=M");
                    self.push_d();
                }
            }
            return Ok(());
        }

        // Non-Long lhs: original code follows
        // 1. Evaluate rhs first
        self.gen_expr(rhs, vars)?;
        // If rhs is Long but lhs is not, take lo word
        if matches!(rhs_ty, Type::Long) {
            self.pop_d();     // lo (on top)
            self.emit("@R13");
            self.emit("M=D");
            self.pop_d();     // hi (discard)
            self.emit("@R13");
            self.emit("D=M");
            self.push_d();    // push lo as single word
        }
        // 2. Pop value into R13
        self.pop_d();
        self.emit("@R13");
        self.emit("M=D");
        // 3. Store R13 to lhs
        match lhs {
            Expr::Ident(name) => {
                let info = vars.get(name).ok_or_else(|| {
                    CodegenError::new(format!("undefined variable '{}'", name))
                })?.clone();
                self.store_var_from_r13(&info);
            }
            Expr::UnOp(UnOp::Deref, ptr_expr) => {
                self.gen_expr(ptr_expr, vars)?;
                self.pop_d();
                self.emit("@R14");
                self.emit("M=D");
                self.emit("@R13");
                self.emit("D=M");
                self.emit("@R14");
                self.emit("A=M");
                self.emit("M=D");
            }
            Expr::Index(base, idx) => {
                self.gen_expr(base, vars)?;
                self.gen_expr(idx, vars)?;
                self.pop_d();
                self.emit("@R14");
                self.emit("M=D");
                self.pop_d();
                self.emit("@R14");
                self.emit("D=D+M");
                self.emit("@R14");
                self.emit("M=D");
                self.emit("@R13");
                self.emit("D=M");
                self.emit("@R14");
                self.emit("A=M");
                self.emit("M=D");
            }
            Expr::Member(base, field) => {
                let base_ty = self.expr_type(base, vars)
                    .ok_or_else(|| CodegenError::new("cannot determine type for member assignment"))?;
                let struct_name = match &base_ty {
                    Type::Struct(name) => name.clone(),
                    _ => return Err(CodegenError::new(
                        format!("member access on non-struct type {:?}", base_ty)
                    )),
                };
                let offset = self.field_offset(&struct_name, field)?;
                self.gen_addr(base, vars)?;
                self.pop_d();
                if offset > 0 {
                    self.emit(&format!("@{}", offset));
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
        let id = self.label();
        let ret_lbl = format!("{}$ret_{}", name, id);
        // Compute n_args as sum of word sizes of arguments
        let n_args: usize = args.iter()
            .map(|a| self.type_size(&self.expr_type(a, vars).unwrap_or(Type::Int)).max(1))
            .sum();

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

        // 2. Push call overhead / jump via trampoline
        if self.use_trampolines {
            // Compact trampoline call: R13=nArgs, R14=callee, D=retAddr → __vm_call
            self.emit(&format!("@{}", n_args));
            self.emit("D=A");
            self.emit("@R13");
            self.emit("M=D");
            self.emit(&format!("@{}", name));
            self.emit("D=A");
            self.emit("@R14");
            self.emit("M=D");
            self.emit(&format!("@{}", ret_lbl));
            self.emit("D=A");
            self.emit("@__vm_call");
            self.emit("0;JMP");
            self.need_call_trampoline = true;
        } else {
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
        }

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
                let ty = self.expr_type(e, vars).unwrap_or(Type::Int);
                let words = self.type_size(&ty).max(1);
                for _ in 0..words {
                    self.pop_d();
                }
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
                                VarStorage::Global(sym) => {
                                    let elem_sym = if i == 0 {
                                        sym.clone()
                                    } else {
                                        format!("{}_{}", sym, i)
                                    };
                                    self.emit(&format!("@{}", elem_sym));
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
                        let decl_ty = vars.get(name).map(|v| v.ty.clone()).unwrap_or(Type::Int);
                        self.gen_expr(init_expr, vars)?;
                        if matches!(decl_ty, Type::Long) {
                            let expr_ty = self.expr_type(init_expr, vars).unwrap_or(Type::Int);
                            if !matches!(expr_ty, Type::Long) {
                                self.sign_extend_to_long();
                            }
                            // Stack: [hi, lo]
                            self.pop_d(); self.emit("@R14"); self.emit("M=D"); // R14 = lo
                            self.pop_d(); self.emit("@R13"); self.emit("M=D"); // R13 = hi
                            self.store_var_long_r13r14(&info);
                        } else {
                            // If init expr is Long but var is not, take lo
                            let expr_ty = self.expr_type(init_expr, vars).unwrap_or(Type::Int);
                            if matches!(expr_ty, Type::Long) {
                                self.pop_d();       // lo
                                self.emit("@R13"); self.emit("M=D");
                                self.pop_d();       // hi (discard)
                                self.emit("@R13"); self.emit("D=M");
                                self.push_d();
                            }
                            self.pop_d();
                            self.emit("@R13");
                            self.emit("M=D");
                            self.store_var_from_r13(&info);
                        }
                    }
                }
            }
            Stmt::Return(expr) => {
                if let Some(e) = expr {
                    self.gen_expr(e, vars)?;
                    if matches!(self.current_ret_ty, Type::Long) {
                        let e_ty = self.expr_type(e, vars).unwrap_or(Type::Int);
                        if !matches!(e_ty, Type::Long) {
                            self.sign_extend_to_long();
                        }
                    }
                } else {
                    if matches!(self.current_ret_ty, Type::Long) {
                        self.emit("D=0"); self.push_d(); // hi
                        self.emit("D=0"); self.push_d(); // lo
                    } else {
                        self.emit("D=0");
                        self.push_d();
                    }
                }
                self.emit(&format!("@{}$return", func_name));
                self.emit("0;JMP");
            }
            Stmt::If(cond, then, els) => {
                let id = self.label();
                let l_else = format!("__if_else_{}", id);
                let l_end  = format!("__if_end_{}", id);
                self.gen_cond_d(cond, vars)?;
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
                self.gen_cond_d(cond, vars)?;
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
                    self.gen_cond_d(c, vars)?;
                    self.emit(&format!("@{}", l_end));
                    self.emit("D;JEQ");
                }
                self.gen_stmt(body, vars, func_name)?;
                self.emit(&format!("({})", l_incr));
                if let Some(inc) = incr {
                    self.gen_expr(inc, vars)?;
                    let inc_ty = self.expr_type(inc, vars).unwrap_or(Type::Int);
                    let words = self.type_size(&inc_ty).max(1);
                    for _ in 0..words {
                        self.pop_d();
                    }
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
                self.gen_cond_d(cond, vars)?;
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

                // `continue` inside a switch should propagate to the enclosing loop's
                // continue target, not stay within the switch.
                let outer_continue = self.loop_ctx.last()
                    .map(|(_, c)| c.clone())
                    .unwrap_or_default();
                self.loop_ctx.push((l_end.clone(), outer_continue));

                for (i, arm) in arms.iter().enumerate() {
                    self.emit(&format!("({})", arm_labels[i]));
                    for s in &arm.stmts {
                        self.gen_stmt(s, vars, func_name)?;
                    }
                }

                self.loop_ctx.pop();
                self.emit(&format!("({})", l_end));
            }
            Stmt::Goto(lbl) => {
                self.emit(&format!("@__lbl_{}_{}", func_name, lbl));
                self.emit("0;JMP");
            }
            Stmt::Label(lbl, stmt) => {
                self.emit(&format!("(__lbl_{}_{})", func_name, lbl));
                self.gen_stmt(stmt, vars, func_name)?;
            }
        }
        Ok(())
    }

    // ── Function codegen ─────────────────────────────────────────────────

    fn gen_func(&mut self, f: &AnnotatedFunc) -> Result<(), CodegenError> {
        let n_locals = f.n_locals;
        self.current_ret_ty = f.ret_ty.clone();
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
        if matches!(f.ret_ty, Type::Long) {
            self.emit("D=0"); self.push_d(); // hi
            self.emit("D=0"); self.push_d(); // lo
        } else {
            self.emit("D=0");
            self.emit("@SP");
            self.emit("A=M");
            self.emit("M=D");
            self.emit("@SP");
            self.emit("M=M+1");
        }

        // Return label for early exits, followed by return sequence / trampoline
        self.emit(&format!("({}$return)", f.name));

        if self.use_trampolines {
            if matches!(f.ret_ty, Type::Long) {
                self.emit("@__vm_return_long");
                self.need_return_long_trampoline = true;
            } else {
                self.emit("@__vm_return");
                self.need_return_trampoline = true;
            }
            self.emit("0;JMP");
        } else {
            if matches!(f.ret_ty, Type::Long) {
                // Long return inline sequence
                self.emit("@LCL"); self.emit("D=M"); self.emit("@R13"); self.emit("M=D");
                self.emit("@5"); self.emit("A=D-A"); self.emit("D=M"); self.emit("@R14"); self.emit("M=D");
                // pop lo -> R15
                self.pop_d(); self.emit("@R15"); self.emit("M=D");
                // pop hi -> ARG[0]
                self.pop_d(); self.emit("@ARG"); self.emit("A=M"); self.emit("M=D");
                // ARG[1] = lo
                self.emit("@ARG"); self.emit("D=M+1"); self.emit("@R9"); self.emit("M=D");
                self.emit("@R15"); self.emit("D=M"); self.emit("@R9"); self.emit("A=M"); self.emit("M=D");
                // SP = ARG + 2
                self.emit("@ARG"); self.emit("D=M+1"); self.emit("D=D+1"); self.emit("@SP"); self.emit("M=D");
                // Restore THAT, THIS, ARG, LCL
                self.emit("@R13"); self.emit("AM=M-1"); self.emit("D=M"); self.emit("@THAT"); self.emit("M=D");
                self.emit("@R13"); self.emit("AM=M-1"); self.emit("D=M"); self.emit("@THIS"); self.emit("M=D");
                self.emit("@R13"); self.emit("AM=M-1"); self.emit("D=M"); self.emit("@ARG"); self.emit("M=D");
                self.emit("@R13"); self.emit("AM=M-1"); self.emit("D=M"); self.emit("@LCL"); self.emit("M=D");
                self.emit("@R14"); self.emit("A=M"); self.emit("0;JMP");
            } else {
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
            }
        }

        Ok(())
    }
}

// ── Pre-scan helpers (call graph analysis) ───────────────────────────────────

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
        Stmt::Goto(_) => {}
        Stmt::Label(_, stmt) => collect_calls_stmt(stmt, calls),
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
// ── Entry point ──────────────────────────────────────────────────────────────

/// Emit assembly to initialize a named RAM symbol to a given value.
/// If `val == 0`, just emits `@sym` to force the assembler to allocate the slot.
fn emit_init_value(g: &mut Gen, val: i16, sym: &str) {
    if val == 0 {
        g.emit(&format!("@{}", sym));
    } else if val == 1 {
        g.emit("D=1");
        g.emit(&format!("@{}", sym));
        g.emit("M=D");
    } else if val == -1 {
        g.emit("D=-1");
        g.emit(&format!("@{}", sym));
        g.emit("M=D");
    } else if val > 0 {
        g.emit(&format!("@{}", val));
        g.emit("D=A");
        g.emit(&format!("@{}", sym));
        g.emit("M=D");
    } else {
        // negative: load absolute value, negate
        g.emit(&format!("@{}", -(val as i32)));
        g.emit("D=-A");
        g.emit(&format!("@{}", sym));
        g.emit("M=D");
    }
}

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
/// Generate Hack assembly instructions that initialize the 8×11 font table in RAM.
/// Returns only the instructions (no labels, no SP init, no call to main).
/// Suitable for inclusion in a bootstrap before calling main.
pub fn gen_font_init_asm() -> String {
    let mut out = String::from("// Pre-load font table\n");
    for ch_idx in 0..96usize {
        for row in 0..11usize {
            let byte = FONT_8X11[ch_idx][row];
            if byte == 0 { continue; }
            let addr = FONT_BASE + ch_idx * 11 + row;
            out.push_str(&format!("@{}\nD=A\n@{}\nM=D\n", byte, addr));
        }
    }
    out
}

/// Return the font table as static `DataInit` entries (for `RAM@` sections in
/// hackem / tst output, avoiding runtime init instructions in the bootstrap).
pub fn gen_font_data_inits() -> Vec<DataInit> {
    let mut data = Vec::new();
    for ch_idx in 0..96usize {
        for row in 0..11usize {
            let byte = FONT_8X11[ch_idx][row];
            if byte == 0 { continue; }
            let addr = (FONT_BASE + ch_idx * 11 + row) as u16;
            data.push(DataInit { address: addr, value: byte as i16 });
        }
    }
    data
}

/// calls `main`, and halts.  `init_code` is inserted between the SP
/// initialization and the call to `main` — use it to initialize global
/// variables and string literals.
pub fn gen_bootstrap(init_code: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push("// Bootstrap".to_string());
    lines.push("@256".to_string());
    lines.push("D=A".to_string());
    lines.push("@SP".to_string());
    lines.push("M=D".to_string());
    if !init_code.is_empty() {
        lines.push(init_code.trim_end().to_string());
    }
    let tail: &[&str] = &[
        // Call main via __vm_call trampoline: R13=0 (nArgs), R14=main addr, D=retAddr
        "@0", "D=A", "@R13", "M=D",
        "@main", "D=A", "@R14", "M=D",
        "@__ld_main_ret", "D=A",
        "@__vm_call", "0;JMP",
        "(__ld_main_ret)",
        "(__end)", "@__end", "0;JMP",
        "",
    ];
    for line in tail {
        lines.push(line.to_string());
    }
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

    // ── Phase 2: Generate code ───────────────────────────────────────────────
    let mut g = Gen::new(sema.string_map.clone(), sema.struct_defs.clone(), true, sema.func_return_types.clone());

    if !body_only {
        g.emit("// Bootstrap");
        g.emit("@256");
        g.emit("D=A");
        g.emit("@SP");
        g.emit("M=D");

        // Emit initialization for string literals (ensures consecutive RAM allocation)
        for (sym_prefix, chars) in &sema.string_literals {
            let n = chars.len();
            for (i, &ch) in chars.iter().enumerate() {
                let sym = if i == 0 { sym_prefix.clone() } else { format!("{}_{}", sym_prefix, i) };
                emit_init_value(&mut g, ch, &sym);
            }
            // Null terminator (always zero — just allocate the slot)
            g.emit(&format!("@{}_{}", sym_prefix, n));
        }

        // Emit allocation for multi-word globals (arrays, structs) — ensures consecutive RAM
        for (name, ty, _init_val) in &sema.globals {
            let sym = format!("__g_{}", name);
            let size = type_size(ty, &sema.struct_defs).max(1);
            if size > 1 {
                for i in 0..size {
                    let elem_sym = if i == 0 { sym.clone() } else { format!("{}_{}", sym, i) };
                    g.emit(&format!("@{}", elem_sym));
                }
            }
        }

        // Emit initialization for non-zero scalar globals
        for (name, ty, init_val) in &sema.globals {
            let sym = format!("__g_{}", name);
            let size = type_size(ty, &sema.struct_defs).max(1);
            if size == 1 {
                if let Some(val) = init_val {
                    if *val != 0 {
                        emit_init_value(&mut g, *val as i16, &sym);
                    }
                }
            }
        }

        // Initialize non-zero Long globals (both hi and lo words)
        for (name, ty, init_val) in &sema.globals {
            if matches!(ty, Type::Long) {
                if let Some(val) = init_val {
                    let val = *val as i32;
                    let hi = ((val as u32 >> 16) & 0xFFFF) as i16;
                    let lo = (val as u16) as i16;
                    let sym = format!("__g_{}", name);
                    if hi != 0 {
                        emit_init_value(&mut g, hi, &sym);
                    }
                    if lo != 0 {
                        emit_init_value(&mut g, lo, &format!("{}_1", sym));
                    }
                }
            }
        }

        // Font table init (before calling main, after globals)
        // Detect reachable functions that use the font table.
        const FONT_USERS: &[&str] = &[
            "draw_char", "draw_string", "print_at",
            "putchar_screen", "puts_screen",
        ];
        let needs_font = sema.funcs.iter()
            .filter(|f| reachable.contains(&f.name))
            .any(|f| {
                let calls = collect_calls_from_stmts(&f.body);
                FONT_USERS.iter().any(|name| calls.contains(*name))
            });
        if needs_font {
            g.emit("// Pre-load font table");
            for ch_idx in 0..96usize {
                for row in 0..11usize {
                    let byte = FONT_8X11[ch_idx][row];
                    if byte == 0 { continue; }
                    let addr = FONT_BASE + ch_idx * 11 + row;
                    g.emit(&format!("@{}", byte));
                    g.emit("D=A");
                    g.emit(&format!("@{}", addr));
                    g.emit("M=D");
                }
            }
        }

        // Call main via trampoline: R13=0 (nArgs), R14=main addr, D=retAddr
        g.emit("@0");
        g.emit("D=A");
        g.emit("@R13");
        g.emit("M=D");
        g.emit("@main");
        g.emit("D=A");
        g.emit("@R14");
        g.emit("M=D");
        g.emit("@__ld_main_ret");
        g.emit("D=A");
        g.emit("@__vm_call");
        g.emit("0;JMP");
        g.emit("(__ld_main_ret)");
        g.emit("(__end)");
        g.emit("@__end");
        g.emit("0;JMP");
        g.emit("");
        g.need_call_trampoline = true;
    }

    // Emit only reachable user-defined functions
    for f in &sema.funcs {
        if reachable.contains(&f.name) {
            g.emit("");
            g.gen_func(f)?;
        }
    }

    // Emit shared VM trampolines inline (whole-program mode only).
    // In body_only mode the trampolines come from lib/sys/__vm_call.s and __vm_return.s.
    if !body_only {
        if g.need_call_trampoline {
            g.emit("");
            g.emit("// VM call trampoline: R13=nArgs, R14=callee_addr, D=retAddr");
            g.emit("(__vm_call)");
            // push retAddr (D)
            g.emit("@SP"); g.emit("A=M"); g.emit("M=D"); g.emit("@SP"); g.emit("M=M+1");
            // push LCL
            g.emit("@LCL"); g.emit("D=M");
            g.emit("@SP"); g.emit("A=M"); g.emit("M=D"); g.emit("@SP"); g.emit("M=M+1");
            // push ARG
            g.emit("@ARG"); g.emit("D=M");
            g.emit("@SP"); g.emit("A=M"); g.emit("M=D"); g.emit("@SP"); g.emit("M=M+1");
            // push THIS
            g.emit("@THIS"); g.emit("D=M");
            g.emit("@SP"); g.emit("A=M"); g.emit("M=D"); g.emit("@SP"); g.emit("M=M+1");
            // push THAT
            g.emit("@THAT"); g.emit("D=M");
            g.emit("@SP"); g.emit("A=M"); g.emit("M=D"); g.emit("@SP"); g.emit("M=M+1");
            // ARG = SP - R13 - 5
            g.emit("@SP"); g.emit("D=M");
            g.emit("@5"); g.emit("D=D-A");
            g.emit("@R13"); g.emit("D=D-M");
            g.emit("@ARG"); g.emit("M=D");
            // LCL = SP
            g.emit("@SP"); g.emit("D=M");
            g.emit("@LCL"); g.emit("M=D");
            // goto callee (address in R14)
            g.emit("@R14"); g.emit("A=M"); g.emit("0;JMP");
        }

        if g.need_return_trampoline {
            g.emit("");
            g.emit("// VM return trampoline");
            g.emit("(__vm_return)");
            // FRAME(R13) = LCL
            g.emit("@LCL"); g.emit("D=M");
            g.emit("@R13"); g.emit("M=D");
            // RET(R14) = *(FRAME-5)
            g.emit("@5"); g.emit("A=D-A"); g.emit("D=M");
            g.emit("@R14"); g.emit("M=D");
            // *ARG = retval (top of stack)
            g.emit("@SP"); g.emit("M=M-1"); g.emit("A=M"); g.emit("D=M");
            g.emit("@ARG"); g.emit("A=M"); g.emit("M=D");
            // SP = ARG + 1
            g.emit("@ARG"); g.emit("D=M+1");
            g.emit("@SP"); g.emit("M=D");
            // THAT = *(FRAME-1)
            g.emit("@R13"); g.emit("AM=M-1"); g.emit("D=M");
            g.emit("@THAT"); g.emit("M=D");
            // THIS = *(FRAME-2)
            g.emit("@R13"); g.emit("AM=M-1"); g.emit("D=M");
            g.emit("@THIS"); g.emit("M=D");
            // ARG = *(FRAME-3)
            g.emit("@R13"); g.emit("AM=M-1"); g.emit("D=M");
            g.emit("@ARG"); g.emit("M=D");
            // LCL = *(FRAME-4)
            g.emit("@R13"); g.emit("AM=M-1"); g.emit("D=M");
            g.emit("@LCL"); g.emit("M=D");
            // goto retAddr
            g.emit("@R14"); g.emit("A=M"); g.emit("0;JMP");
        }

        if g.need_return_long_trampoline {
            g.emit("");
            g.emit("// VM return-long trampoline");
            g.emit("(__vm_return_long)");
            g.emit("@LCL"); g.emit("D=M"); g.emit("@R13"); g.emit("M=D");
            g.emit("@5"); g.emit("A=D-A"); g.emit("D=M"); g.emit("@R14"); g.emit("M=D");
            g.emit("@SP"); g.emit("M=M-1"); g.emit("A=M"); g.emit("D=M"); g.emit("@R15"); g.emit("M=D");
            g.emit("@SP"); g.emit("M=M-1"); g.emit("A=M"); g.emit("D=M");
            g.emit("@ARG"); g.emit("A=M"); g.emit("M=D");
            g.emit("@ARG"); g.emit("D=M+1"); g.emit("@R9"); g.emit("M=D");
            g.emit("@R15"); g.emit("D=M"); g.emit("@R9"); g.emit("A=M"); g.emit("M=D");
            g.emit("@ARG"); g.emit("D=M+1"); g.emit("D=D+1"); g.emit("@SP"); g.emit("M=D");
            g.emit("@R13"); g.emit("AM=M-1"); g.emit("D=M"); g.emit("@THAT"); g.emit("M=D");
            g.emit("@R13"); g.emit("AM=M-1"); g.emit("D=M"); g.emit("@THIS"); g.emit("M=D");
            g.emit("@R13"); g.emit("AM=M-1"); g.emit("D=M"); g.emit("@ARG"); g.emit("M=D");
            g.emit("@R13"); g.emit("AM=M-1"); g.emit("D=M"); g.emit("@LCL"); g.emit("M=D");
            g.emit("@R14"); g.emit("A=M"); g.emit("0;JMP");
        }
    }

    let asm = g.out.join("\n") + "\n";

    Ok(CompiledProgram { asm, data: Vec::new() })
}
