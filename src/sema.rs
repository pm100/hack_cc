use std::collections::HashMap;
use thiserror::Error;
use crate::parser::{Program, FuncDef, Stmt, Expr, BinOp, Type};

#[derive(Debug, Error, Clone)]
#[error("semantic error: {0}")]
pub struct SemaError(pub String);

impl SemaError {
    fn new(msg: impl Into<String>) -> Self { Self(msg.into()) }
}

#[derive(Debug, Clone)]
pub struct VarInfo {
    pub ty: Type,
    pub storage: VarStorage,
}

#[derive(Debug, Clone)]
pub enum VarStorage {
    /// Offset from LCL base (index, 0-based)
    Local(usize),
    /// Offset from ARG base (index, 0-based)
    Param(usize),
    /// Absolute RAM address
    Global(usize),
}

#[derive(Debug, Clone)]
pub struct AnnotatedFunc {
    pub name: String,
    pub ret_ty: Type,
    pub params: Vec<(Type, String)>,
    pub n_locals: usize,
    pub body: Vec<Stmt>,
    pub vars: HashMap<String, VarInfo>,
}

#[derive(Debug, Clone)]
pub struct SemaResult {
    pub globals: Vec<(String, usize, Type, Option<i32>)>, // name, addr, ty, init
    pub funcs: Vec<AnnotatedFunc>,
    pub func_sigs: HashMap<String, (Type, usize)>, // name -> (ret_ty, n_params)
}

pub fn analyze(prog: Program) -> Result<SemaResult, SemaError> {
    // Assign global variable addresses starting at RAM[16]
    let mut next_global_addr = 16usize;
    let mut global_map: HashMap<String, VarInfo> = HashMap::new();
    let mut globals_out = Vec::new();

    for (ty, name, init_expr) in &prog.globals {
        let addr = next_global_addr;
        let size = ty.size().max(1);
        next_global_addr += size;
        let init_val = match init_expr {
            Some(e) => Some(eval_const(e)?),
            None => None,
        };
        global_map.insert(name.clone(), VarInfo {
            ty: ty.clone(),
            storage: VarStorage::Global(addr),
        });
        globals_out.push((name.clone(), addr, ty.clone(), init_val));
    }

    // Build function signature table
    let mut func_sigs: HashMap<String, (Type, usize)> = HashMap::new();
    for f in &prog.funcs {
        func_sigs.insert(f.name.clone(), (f.ret_ty.clone(), f.params.len()));
    }

    // Analyze each function
    let mut funcs_out = Vec::new();
    for f in prog.funcs {
        let af = analyze_func(f, &global_map)?;
        funcs_out.push(af);
    }

    Ok(SemaResult { globals: globals_out, funcs: funcs_out, func_sigs })
}

fn eval_const(expr: &Expr) -> Result<i32, SemaError> {
    match expr {
        Expr::Num(n) => Ok(*n),
        Expr::UnOp(crate::parser::UnOp::Neg, e) => Ok(-eval_const(e)?),
        _ => Err(SemaError::new("global initializer must be a constant integer")),
    }
}

fn analyze_func(
    f: FuncDef,
    globals: &HashMap<String, VarInfo>,
) -> Result<AnnotatedFunc, SemaError> {
    let mut vars: HashMap<String, VarInfo> = HashMap::new();

    // Insert params
    for (i, (ty, name)) in f.params.iter().enumerate() {
        vars.insert(name.clone(), VarInfo {
            ty: ty.clone(),
            storage: VarStorage::Param(i),
        });
    }

    // Collect locals from body
    let mut local_idx = 0usize;
    collect_locals(&f.body, &mut vars, &mut local_idx)?;

    // Merge globals (lower priority)
    for (name, info) in globals {
        vars.entry(name.clone()).or_insert_with(|| info.clone());
    }

    Ok(AnnotatedFunc {
        name: f.name,
        ret_ty: f.ret_ty,
        params: f.params,
        n_locals: local_idx,
        body: f.body,
        vars,
    })
}

fn collect_locals(
    stmts: &[Stmt],
    vars: &mut HashMap<String, VarInfo>,
    next_idx: &mut usize,
) -> Result<(), SemaError> {
    for stmt in stmts {
        collect_locals_stmt(stmt, vars, next_idx)?;
    }
    Ok(())
}

fn collect_locals_stmt(
    stmt: &Stmt,
    vars: &mut HashMap<String, VarInfo>,
    next_idx: &mut usize,
) -> Result<(), SemaError> {
    match stmt {
        Stmt::Decl(ty, name, _) => {
            let size = ty.size().max(1);
            let idx = *next_idx;
            *next_idx += size;
            vars.insert(name.clone(), VarInfo {
                ty: ty.clone(),
                storage: VarStorage::Local(idx),
            });
        }
        Stmt::Block(stmts) => collect_locals(stmts, vars, next_idx)?,
        Stmt::If(_, then, els) => {
            collect_locals_stmt(then, vars, next_idx)?;
            if let Some(e) = els { collect_locals_stmt(e, vars, next_idx)?; }
        }
        Stmt::While(_, body) => collect_locals_stmt(body, vars, next_idx)?,
        Stmt::For { init, body, .. } => {
            if let Some(s) = init { collect_locals_stmt(s, vars, next_idx)?; }
            collect_locals_stmt(body, vars, next_idx)?;
        }
        Stmt::Return(_) | Stmt::Expr(_) => {}
    }
    Ok(())
}

/// Given an expression that appears as an lvalue, return an Expr that is
/// guaranteed to be usable as an assignment target.
/// We don't need to do deep validation here; codegen will catch it.
pub fn lvalue_ok(expr: &Expr) -> bool {
    match expr {
        Expr::Ident(_) => true,
        Expr::UnOp(crate::parser::UnOp::Deref, _) => true,
        Expr::Index(_, _) => true,
        _ => false,
    }
}

/// Resolve compound assignment `x op= rhs` into `x = x op rhs`
pub fn desugar_compound(op: &BinOp, lhs: Expr, rhs: Expr) -> Expr {
    let arith_op = match op {
        BinOp::AddAssign => BinOp::Add,
        BinOp::SubAssign => BinOp::Sub,
        _ => unreachable!(),
    };
    Expr::BinOp(
        BinOp::Assign,
        Box::new(lhs.clone()),
        Box::new(Expr::BinOp(arith_op, Box::new(lhs), Box::new(rhs))),
    )
}
