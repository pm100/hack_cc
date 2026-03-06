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
    /// RAM address and char values (without null terminator) for each string literal.
    pub string_literals: Vec<(usize, Vec<i16>)>,
    /// Map from string content to its RAM address (for codegen lookup).
    pub string_map: HashMap<String, usize>,
    /// Struct definitions: name -> ordered list of (field_name, field_type).
    pub struct_defs: HashMap<String, Vec<(String, Type)>>,
}

/// Compute the size in Hack words of a type, resolving struct sizes via struct_defs.
pub fn type_size(ty: &Type, defs: &HashMap<String, Vec<(String, Type)>>) -> usize {
    match ty {
        Type::Void => 0,
        Type::Int | Type::Char | Type::Ptr(_) => 1,
        Type::Array(base, n) => type_size(base, defs) * n,
        Type::Struct(name) => {
            defs.get(name)
                .map(|fields| fields.iter().map(|(_, t)| type_size(t, defs)).sum())
                .unwrap_or(1) // unknown struct — treat as 1 to avoid 0-size locals
        }
    }
}

pub fn analyze(prog: Program) -> Result<SemaResult, SemaError> {
    // Build struct definition map: name -> [(field_name, field_type)]
    let mut struct_defs: HashMap<String, Vec<(String, Type)>> = HashMap::new();
    for sd in &prog.struct_defs {
        let fields: Vec<(String, Type)> = sd.fields.iter()
            .map(|(ty, name)| (name.clone(), ty.clone()))
            .collect();
        struct_defs.insert(sd.name.clone(), fields);
    }

    // Assign global variable addresses starting at RAM[16]
    let mut next_global_addr = 16usize;
    let mut global_map: HashMap<String, VarInfo> = HashMap::new();
    let mut globals_out = Vec::new();

    for (ty, name, init_expr) in &prog.globals {
        let addr = next_global_addr;
        let size = type_size(ty, &struct_defs).max(1);
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

    // Collect string literals from all function bodies (and global inits)
    let mut string_map: HashMap<String, usize> = HashMap::new();
    let mut string_literals: Vec<(usize, Vec<i16>)> = Vec::new();
    for (_, _, init_expr) in &prog.globals {
        if let Some(e) = init_expr {
            collect_strings_expr(e, &mut string_map, &mut string_literals, &mut next_global_addr);
        }
    }
    for f in &prog.funcs {
        for stmt in &f.body {
            collect_strings_stmt(stmt, &mut string_map, &mut string_literals, &mut next_global_addr);
        }
    }

    // Build function signature table
    let mut func_sigs: HashMap<String, (Type, usize)> = HashMap::new();
    for f in &prog.funcs {
        func_sigs.insert(f.name.clone(), (f.ret_ty.clone(), f.params.len()));
    }

    // Analyze each function
    let mut funcs_out = Vec::new();
    for f in prog.funcs {
        let af = analyze_func(f, &global_map, &struct_defs)?;
        funcs_out.push(af);
    }

    Ok(SemaResult { globals: globals_out, funcs: funcs_out, func_sigs, string_literals, string_map, struct_defs })
}

fn eval_const(expr: &Expr) -> Result<i32, SemaError> {
    match expr {
        Expr::Num(n) => Ok(*n),
        Expr::UnOp(crate::parser::UnOp::Neg, e) => Ok(-eval_const(e)?),
        Expr::StringLit(_) => Err(SemaError::new("string literal cannot be used as integer constant initializer")),
        _ => Err(SemaError::new("global initializer must be a constant integer")),
    }
}

fn analyze_func(
    f: FuncDef,
    globals: &HashMap<String, VarInfo>,
    struct_defs: &HashMap<String, Vec<(String, Type)>>,
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
    collect_locals(&f.body, &mut vars, &mut local_idx, struct_defs)?;

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
    struct_defs: &HashMap<String, Vec<(String, Type)>>,
) -> Result<(), SemaError> {
    for stmt in stmts {
        collect_locals_stmt(stmt, vars, next_idx, struct_defs)?;
    }
    Ok(())
}

fn collect_locals_stmt(
    stmt: &Stmt,
    vars: &mut HashMap<String, VarInfo>,
    next_idx: &mut usize,
    struct_defs: &HashMap<String, Vec<(String, Type)>>,
) -> Result<(), SemaError> {
    match stmt {
        Stmt::Decl(ty, name, _) => {
            let size = type_size(ty, struct_defs).max(1);
            let idx = *next_idx;
            *next_idx += size;
            vars.insert(name.clone(), VarInfo {
                ty: ty.clone(),
                storage: VarStorage::Local(idx),
            });
        }
        Stmt::Block(stmts) => collect_locals(stmts, vars, next_idx, struct_defs)?,
        Stmt::If(_, then, els) => {
            collect_locals_stmt(then, vars, next_idx, struct_defs)?;
            if let Some(e) = els { collect_locals_stmt(e, vars, next_idx, struct_defs)?; }
        }
        Stmt::While(_, body) => collect_locals_stmt(body, vars, next_idx, struct_defs)?,
        Stmt::For { init, body, .. } => {
            if let Some(s) = init { collect_locals_stmt(s, vars, next_idx, struct_defs)?; }
            collect_locals_stmt(body, vars, next_idx, struct_defs)?;
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
        Expr::Member(_, _) => true,
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

// ── String literal collection ────────────────────────────────────────────────

fn intern_string(
    s: &str,
    map: &mut HashMap<String, usize>,
    lits: &mut Vec<(usize, Vec<i16>)>,
    next_addr: &mut usize,
) {
    if !map.contains_key(s) {
        let addr = *next_addr;
        let chars: Vec<i16> = s.bytes().map(|b| b as i16).collect();
        *next_addr += chars.len() + 1; // +1 for null terminator slot
        map.insert(s.to_string(), addr);
        lits.push((addr, chars));
    }
}

fn collect_strings_expr(
    expr: &Expr,
    map: &mut HashMap<String, usize>,
    lits: &mut Vec<(usize, Vec<i16>)>,
    next_addr: &mut usize,
) {
    match expr {
        Expr::StringLit(s) => intern_string(s, map, lits, next_addr),
        Expr::BinOp(_, l, r) => {
            collect_strings_expr(l, map, lits, next_addr);
            collect_strings_expr(r, map, lits, next_addr);
        }
        Expr::UnOp(_, e) => collect_strings_expr(e, map, lits, next_addr),
        Expr::Call(_, args) => {
            for a in args { collect_strings_expr(a, map, lits, next_addr); }
        }
        Expr::Index(a, b) => {
            collect_strings_expr(a, map, lits, next_addr);
            collect_strings_expr(b, map, lits, next_addr);
        }
        Expr::Member(base, _) => collect_strings_expr(base, map, lits, next_addr),
        Expr::Num(_) | Expr::Ident(_) | Expr::Sizeof(_) => {}
    }
}

fn collect_strings_stmt(
    stmt: &Stmt,
    map: &mut HashMap<String, usize>,
    lits: &mut Vec<(usize, Vec<i16>)>,
    next_addr: &mut usize,
) {
    match stmt {
        Stmt::Expr(e) => collect_strings_expr(e, map, lits, next_addr),
        Stmt::Return(Some(e)) => collect_strings_expr(e, map, lits, next_addr),
        Stmt::Decl(_, _, Some(e)) => collect_strings_expr(e, map, lits, next_addr),
        Stmt::Block(stmts) => {
            for s in stmts { collect_strings_stmt(s, map, lits, next_addr); }
        }
        Stmt::If(cond, then, els) => {
            collect_strings_expr(cond, map, lits, next_addr);
            collect_strings_stmt(then, map, lits, next_addr);
            if let Some(e) = els { collect_strings_stmt(e, map, lits, next_addr); }
        }
        Stmt::While(cond, body) => {
            collect_strings_expr(cond, map, lits, next_addr);
            collect_strings_stmt(body, map, lits, next_addr);
        }
        Stmt::For { init, cond, incr, body } => {
            if let Some(s) = init { collect_strings_stmt(s, map, lits, next_addr); }
            if let Some(e) = cond { collect_strings_expr(e, map, lits, next_addr); }
            if let Some(e) = incr { collect_strings_expr(e, map, lits, next_addr); }
            collect_strings_stmt(body, map, lits, next_addr);
        }
        _ => {}
    }
}

