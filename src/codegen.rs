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

use std::collections::HashMap;
use thiserror::Error;
use crate::sema::{SemaResult, AnnotatedFunc, VarInfo, VarStorage};
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
}

impl Gen {
    fn new(_func_sigs: HashMap<String, (Type, usize)>) -> Self {
        Self { out: Vec::new(), label_id: 0 }
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
                let sz = ty.size().max(1) as i32;
                self.emit(&format!("@{}", sz));
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
                self.emit("A=D");
                self.emit("@R13");
                self.emit("D=M");
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
                self.emit("A=D");
                self.emit("@R13");
                self.emit("D=M");
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
    }
}

pub fn generate(sema: SemaResult) -> Result<String, CodegenError> {
    let mut g = Gen::new(sema.func_sigs.clone());

    // Bootstrap
    g.emit("// Bootstrap");
    g.emit("@256");
    g.emit("D=A");
    g.emit("@SP");
    g.emit("M=D");

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

    // Initialize global variables
    if !sema.globals.is_empty() {
        g.emit("// Global variable initialization");
        g.emit("// (globals are pre-zeroed; non-zero initializers set here)");
        for (name, addr, _ty, init_val) in &sema.globals {
            if let Some(val) = init_val {
                g.emit(&format!("// init global {} at {}", name, addr));
                if *val == 0 {
                    g.emit("D=0");
                } else if *val == 1 {
                    g.emit("D=1");
                } else if *val == -1 {
                    g.emit("D=-1");
                } else if *val > 0 {
                    g.emit(&format!("@{}", val));
                    g.emit("D=A");
                } else {
                    g.emit(&format!("@{}", -val));
                    g.emit("D=-A");
                }
                g.emit(&format!("@{}", addr));
                g.emit("M=D");
            }
        }
    }

    // Generate functions
    for f in &sema.funcs {
        g.emit("");
        g.gen_func(f)?;
    }

    // Runtime subroutines
    g.emit_runtime();

    Ok(g.out.join("\n") + "\n")
}

