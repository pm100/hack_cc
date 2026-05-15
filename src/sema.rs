use std::collections::{HashMap, HashSet};
use thiserror::Error;
use crate::parser::{Program, FuncDef, Stmt, Expr, BinOp, UnOp, Type, StorageClass, SwitchLabel};

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
    /// Assembler symbol name (e.g. `__g_count`); address resolved by assembler
    Global(String),
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

/// Initial value for a global variable.
#[derive(Debug, Clone)]
pub enum GlobalInit {
    /// Single-word (scalar) initial value.
    Scalar(i32),
    /// Per-element values for an array global (in declaration order).
    Array(Vec<i32>),
}

#[derive(Debug, Clone)]
pub struct SemaResult {
    pub globals: Vec<(String, Type, Option<GlobalInit>)>, // symbol, ty, init
    pub funcs: Vec<AnnotatedFunc>,
    pub func_sigs: HashMap<String, (Type, usize)>, // name -> (ret_ty, n_params)
    /// Return type for each function name (for codegen to know call return type).
    pub func_return_types: HashMap<String, Type>,
    /// Symbol prefix and char values (without null terminator) for each string literal.
    pub string_literals: Vec<(String, Vec<i16>)>,
    /// Map from string content to its assembler symbol prefix (for codegen lookup).
    pub string_map: HashMap<String, String>,
    /// Struct definitions: name -> ordered list of (field_name, field_type).
    pub struct_defs: HashMap<String, Vec<(String, Type)>>,
}

/// Compute the size in Hack words of a type, resolving struct sizes via struct_defs.
pub fn type_size(ty: &Type, defs: &HashMap<String, Vec<(String, Type)>>) -> usize {
    match ty {
        Type::Void => 0,
        Type::Int | Type::Char | Type::UnsignedChar | Type::Ptr(_) => 1,
        Type::Long => 2,
        Type::Array(base, n) => type_size(base, defs) * n,
        Type::Struct(name) => {
            defs.get(name)
                .map(|fields| fields.iter().map(|(_, t)| type_size(t, defs)).sum())
                .unwrap_or(1) // unknown struct â€” treat as 1 to avoid 0-size locals
        }
    }
}

/// Analyze a complete program (all functions must be defined or in KNOWN_EXTERNALS).
pub fn analyze(prog: Program) -> Result<SemaResult, SemaError> {
    analyze_impl(prog, &[])
}

/// Analyze a single translation unit for separate compilation.
/// `user_externals` lists forward-declared functions (from headers) that will be
/// provided by other object files at link time.
pub fn analyze_for_object_file(prog: Program, user_externals: &[&str]) -> Result<SemaResult, SemaError> {
    analyze_impl(prog, user_externals)
}

fn analyze_impl(prog: Program, user_externals: &[&str]) -> Result<SemaResult, SemaError> {
    // Build struct definition map: name -> [(field_name, field_type)]
    let mut struct_defs: HashMap<String, Vec<(String, Type)>> = HashMap::new();
    for sd in &prog.struct_defs {
        let fields: Vec<(String, Type)> = sd.fields.iter()
            .map(|(ty, name)| (name.clone(), ty.clone()))
            .collect();
        struct_defs.insert(sd.name.clone(), fields);
    }

    // Assign global variable symbols.
    // Multiple declarations of the same name (tentative definitions) are merged into one.
    // Extern-only declarations (no definition anywhere) allocate no storage.
    let mut global_map: HashMap<String, VarInfo> = HashMap::new();
    let mut globals_out: Vec<(String, Type, Option<GlobalInit>)> = Vec::new();
    let mut seen_globals: HashMap<String, (Type, Option<GlobalInit>, bool, bool)> = HashMap::new();
    // name -> (ty, init, has_definition, is_static)
    for (ty, name, init_expr, sc) in &prog.globals {
        let is_extern_only = *sc == StorageClass::Extern;
        if is_extern_only && init_expr.is_some() {
            return Err(SemaError::new(format!("extern variable '{}' cannot have an initializer", name)));
        }
        let init_val = match init_expr {
            Some(e) => Some(eval_global_init(e, ty)?),
            None => None,
        };
        if let Some(existing) = seen_globals.get(name) {
            if &existing.0 != ty {
                return Err(SemaError::new(format!("conflicting types for variable '{}'", name)));
            }
            let is_static = *sc == StorageClass::Static;
            if *sc != StorageClass::Extern && existing.3 != is_static {
                return Err(SemaError::new(format!("conflicting linkage for variable '{}'", name)));
            }
            if !is_extern_only && existing.1.is_some() && init_val.is_some() {
                return Err(SemaError::new(format!("multiple definitions of '{}'", name)));
            }
            let entry = seen_globals.get_mut(name).unwrap();
            if !is_extern_only {
                entry.2 = true;
                if entry.1.is_none() && init_val.is_some() {
                    entry.1 = init_val;
                }
            }
        } else {
            let is_static = *sc == StorageClass::Static;
            seen_globals.insert(name.clone(), (ty.clone(), init_val, !is_extern_only, is_static));
        }
    }

    for (name, (ty, init_val, has_def, _)) in &seen_globals {
        let sym = format!("__g_{}", name);
        global_map.insert(name.clone(), VarInfo { ty: ty.clone(), storage: VarStorage::Global(sym.clone()) });
        if *has_def {
            globals_out.push((sym, ty.clone(), init_val.clone()));
        }
    }

    // Collect string literals from all function bodies (and global inits)
    let mut string_map: HashMap<String, String> = HashMap::new();
    let mut string_literals: Vec<(String, Vec<i16>)> = Vec::new();
    let mut str_counter = 0usize;
    for (_, _, init_expr, _) in &prog.globals {
        if let Some(e) = init_expr {
            collect_strings_expr(e, &mut string_map, &mut string_literals, &mut str_counter);
        }
    }
    for f in &prog.funcs {
        if f.is_decl { continue; }
        for stmt in &f.body {
            collect_strings_stmt(stmt, &mut string_map, &mut string_literals, &mut str_counter);
        }
    }

    // Build function signature table (include declarations so calls/type checks see them).
    let mut func_sigs: HashMap<String, (Type, usize)> = HashMap::new();
    let mut func_params: HashMap<String, Vec<Type>> = HashMap::new();
    let mut func_linkage: HashMap<String, bool> = HashMap::new();
    let mut func_def_count: HashMap<String, usize> = HashMap::new();
    let mut func_variadic: HashSet<String> = HashSet::new();
    let mut func_variadic_info: HashMap<String, bool> = HashMap::new();
    for f in &prog.funcs {
        if seen_globals.contains_key(&f.name) {
            return Err(SemaError::new(format!("'{}' redeclared as different kind of symbol", f.name)));
        }
        if let Some(existing) = func_sigs.get(&f.name) {
            if existing.0 != f.ret_ty {
                return Err(SemaError::new(format!("conflicting return type for function '{}'", f.name)));
            }
            let was_variadic = *func_variadic_info.get(&f.name).unwrap();
            if was_variadic != f.is_variadic {
                return Err(SemaError::new(format!("conflicting parameter count for function '{}'", f.name)));
            }
            if existing.1 != f.params.len() && !was_variadic {
                return Err(SemaError::new(format!("conflicting parameter count for function '{}'", f.name)));
            }
            if let Some(&was_static) = func_linkage.get(&f.name) {
                if was_static != f.is_static {
                    return Err(SemaError::new(format!("conflicting linkage for function '{}'", f.name)));
                }
            }
            if !f.is_decl {
                let cnt = func_def_count.entry(f.name.clone()).or_insert(0);
                *cnt += 1;
                if *cnt > 1 {
                    return Err(SemaError::new(format!("multiple definitions of function '{}'", f.name)));
                }
            }
        } else {
            func_sigs.insert(f.name.clone(), (f.ret_ty.clone(), f.params.len()));
            func_params.insert(f.name.clone(), f.params.iter().map(|(ty, _)| ty.clone()).collect());
            func_linkage.insert(f.name.clone(), f.is_static);
            func_variadic_info.insert(f.name.clone(), f.is_variadic);
            if !f.is_decl {
                func_def_count.insert(f.name.clone(), 1);
            }
        }
        if f.is_variadic {
            func_variadic.insert(f.name.clone());
        }
    }
    for name in seen_globals.keys() {
        if func_sigs.contains_key(name) {
            return Err(SemaError::new(format!("'{}' redeclared as different kind of symbol", name)));
        }
    }

    let defined_funcs: HashSet<String> = func_sigs.keys().cloned().collect();
    let mut funcs_out = Vec::new();
    for f in prog.funcs {
        if f.is_decl { continue; }
        let (af, static_locals) = analyze_func(f, &global_map, &struct_defs, &func_sigs, &func_params)?;
        check_calls_defined_ext(&af.body, &defined_funcs, &[], user_externals)?;
        check_call_arity(&af.body, &func_sigs, &func_variadic)?;
        globals_out.extend(static_locals);
        funcs_out.push(af);
    }

    let func_return_types: HashMap<String, Type> = func_sigs.iter()
        .map(|(name, (ret_ty, _))| (name.clone(), ret_ty.clone()))
        .collect();

    Ok(SemaResult { globals: globals_out, funcs: funcs_out, func_sigs, func_return_types, string_literals, string_map, struct_defs })
}

fn eval_const(expr: &Expr) -> Result<i32, SemaError> {
    match expr {
        Expr::Num(n) => Ok(*n),
        Expr::UnOp(crate::parser::UnOp::Neg, e) => Ok(-eval_const(e)?),
        Expr::Cast(_, e) => eval_const(e),
        Expr::StringLit(_) => Err(SemaError::new("string literal cannot be used as integer constant initializer")),
        _ => Err(SemaError::new("global initializer must be a constant integer")),
    }
}

/// Evaluate a global-variable initializer expression, returning a `GlobalInit`.
/// Scalars â†’ `GlobalInit::Scalar(n)`.
/// `{...}` lists â†’ `GlobalInit::Array(vec![...])`.
fn eval_global_init(expr: &Expr, ty: &Type) -> Result<GlobalInit, SemaError> {
    match expr {
        Expr::InitList(items) => {
            // Scalar initialized with multi-item compound initializer is an error.
            if !matches!(ty, Type::Array(_, _)) && items.len() > 1 {
                return Err(SemaError::new("too many values in initializer for scalar type"));
            }
            // Array initialized with too many items is an error.
            if let Type::Array(_, n) = ty {
                if *n > 0 && items.len() > *n {
                    return Err(SemaError::new(format!(
                        "too many initializers for array (expected at most {}, got {})", n, items.len()
                    )));
                }
            }
            let mut vals = Vec::new();
            for item in items {
                match item {
                    // String literal element: copy char values + implicit 0 padding handled at codegen
                    Expr::StringLit(s) => {
                        for b in s.bytes() { vals.push(b as i32); }
                        vals.push(0); // null terminator
                    }
                    other => vals.push(eval_const(other)?),
                }
            }
            Ok(GlobalInit::Array(vals))
        }
        other => {
            // Array type cannot be initialized with a scalar expression.
            if matches!(ty, Type::Array(_, _)) {
                return Err(SemaError::new("array must be initialized with a compound initializer"));
            }
            let val = eval_const(other)?;
            if matches!(ty, Type::Ptr(_)) && val != 0 {
                return Err(SemaError::new("cannot initialize pointer with non-zero integer constant"));
            }
            Ok(GlobalInit::Scalar(val))
        }
    }
}

fn check_calls_defined_ext(
    stmts: &[Stmt],
    defined: &HashSet<String>,
    builtins: &[&str],
    externals: &[&str],
) -> Result<(), SemaError> {
    for stmt in stmts {
        check_calls_stmt_ext(stmt, defined, builtins, externals)?;
    }
    Ok(())
}

fn check_calls_stmt_ext(stmt: &Stmt, defined: &HashSet<String>, builtins: &[&str], externals: &[&str]) -> Result<(), SemaError> {
    match stmt {
        Stmt::Expr(e) => check_calls_expr_ext(e, defined, builtins, externals)?,
        Stmt::Return(Some(e)) => check_calls_expr_ext(e, defined, builtins, externals)?,
        Stmt::Decl(_, _, Some(e), _) => check_calls_expr_ext(e, defined, builtins, externals)?,
        Stmt::If(c, t, el) => {
            check_calls_expr_ext(c, defined, builtins, externals)?;
            check_calls_stmt_ext(t, defined, builtins, externals)?;
            if let Some(e) = el { check_calls_stmt_ext(e, defined, builtins, externals)?; }
        }
        Stmt::While(c, b) => {
            check_calls_expr_ext(c, defined, builtins, externals)?;
            check_calls_stmt_ext(b, defined, builtins, externals)?;
        }
        Stmt::DoWhile(b, c) => {
            check_calls_stmt_ext(b, defined, builtins, externals)?;
            check_calls_expr_ext(c, defined, builtins, externals)?;
        }
        Stmt::For { init, cond, incr, body } => {
            if let Some(s) = init { check_calls_stmt_ext(s, defined, builtins, externals)?; }
            if let Some(e) = cond { check_calls_expr_ext(e, defined, builtins, externals)?; }
            if let Some(e) = incr { check_calls_expr_ext(e, defined, builtins, externals)?; }
            check_calls_stmt_ext(body, defined, builtins, externals)?;
        }
        Stmt::Block(stmts) => check_calls_defined_ext(stmts, defined, builtins, externals)?,
        Stmt::Switch { expr, arms } => {
            check_calls_expr_ext(expr, defined, builtins, externals)?;
            for arm in arms {
                for s in &arm.stmts { check_calls_stmt_ext(s, defined, builtins, externals)?; }
            }
        }
        Stmt::Break | Stmt::Continue => {}
        _ => {}
    }
    Ok(())
}

fn check_calls_expr_ext(expr: &Expr, defined: &HashSet<String>, builtins: &[&str], externals: &[&str]) -> Result<(), SemaError> {
    match expr {
        Expr::Call(name, args) => {
            if !defined.contains(name) && !builtins.contains(&name.as_str()) && !externals.contains(&name.as_str()) {
                return Err(SemaError::new(format!(
                    "call to undeclared function '{}'",
                    name
                )));
            }
            for a in args { check_calls_expr_ext(a, defined, builtins, externals)?; }
        }
        Expr::BinOp(_, l, r) => {
            check_calls_expr_ext(l, defined, builtins, externals)?;
            check_calls_expr_ext(r, defined, builtins, externals)?;
        }
        Expr::UnOp(_, e) => check_calls_expr_ext(e, defined, builtins, externals)?,
        Expr::Index(a, b) => {
            check_calls_expr_ext(a, defined, builtins, externals)?;
            check_calls_expr_ext(b, defined, builtins, externals)?;
        }
        Expr::Member(base, _) => check_calls_expr_ext(base, defined, builtins, externals)?,
        Expr::Ternary(c, t, e) => {
            check_calls_expr_ext(c, defined, builtins, externals)?;
            check_calls_expr_ext(t, defined, builtins, externals)?;
            check_calls_expr_ext(e, defined, builtins, externals)?;
        }
        Expr::Cast(_, e) => check_calls_expr_ext(e, defined, builtins, externals)?,
        Expr::PostInc(e) | Expr::PostDec(e) => check_calls_expr_ext(e, defined, builtins, externals)?,
        Expr::InitList(items) => {
            for item in items { check_calls_expr_ext(item, defined, builtins, externals)?; }
        }
        Expr::Num(_) | Expr::Ident(_) | Expr::StringLit(_) | Expr::Sizeof(_) | Expr::SizeofExpr(_) => {}
    }
    Ok(())
}


fn check_call_arity(
    stmts: &[Stmt],
    func_sigs: &HashMap<String, (Type, usize)>,
    variadic: &HashSet<String>,
) -> Result<(), SemaError> {
    for stmt in stmts {
        check_call_arity_stmt(stmt, func_sigs, variadic)?;
    }
    Ok(())
}

fn check_call_arity_stmt(
    stmt: &Stmt,
    func_sigs: &HashMap<String, (Type, usize)>,
    variadic: &HashSet<String>,
) -> Result<(), SemaError> {
    match stmt {
        Stmt::Expr(e) => check_call_arity_expr(e, func_sigs, variadic)?,
        Stmt::Return(Some(e)) => check_call_arity_expr(e, func_sigs, variadic)?,
        Stmt::Decl(_, _, Some(e), _) => check_call_arity_expr(e, func_sigs, variadic)?,
        Stmt::Decl(_, _, None, _) => {}
        Stmt::If(c, t, el) => {
            check_call_arity_expr(c, func_sigs, variadic)?;
            check_call_arity_stmt(t, func_sigs, variadic)?;
            if let Some(e) = el { check_call_arity_stmt(e, func_sigs, variadic)?; }
        }
        Stmt::While(c, b) => {
            check_call_arity_expr(c, func_sigs, variadic)?;
            check_call_arity_stmt(b, func_sigs, variadic)?;
        }
        Stmt::DoWhile(b, c) => {
            check_call_arity_stmt(b, func_sigs, variadic)?;
            check_call_arity_expr(c, func_sigs, variadic)?;
        }
        Stmt::For { init, cond, incr, body } => {
            if let Some(s) = init { check_call_arity_stmt(s, func_sigs, variadic)?; }
            if let Some(e) = cond { check_call_arity_expr(e, func_sigs, variadic)?; }
            if let Some(e) = incr { check_call_arity_expr(e, func_sigs, variadic)?; }
            check_call_arity_stmt(body, func_sigs, variadic)?;
        }
        Stmt::Block(stmts) => check_call_arity(stmts, func_sigs, variadic)?,
        Stmt::Switch { expr, arms } => {
            check_call_arity_expr(expr, func_sigs, variadic)?;
            for arm in arms {
                for s in &arm.stmts {
                    check_call_arity_stmt(s, func_sigs, variadic)?;
                }
            }
        }
        Stmt::Label(_, inner) => check_call_arity_stmt(inner, func_sigs, variadic)?,
        Stmt::Break | Stmt::Continue | Stmt::Return(None) | Stmt::Goto(_) => {}
    }
    Ok(())
}

fn check_call_arity_expr(
    expr: &Expr,
    func_sigs: &HashMap<String, (Type, usize)>,
    variadic: &HashSet<String>,
) -> Result<(), SemaError> {
    match expr {
        Expr::Call(name, args) => {
            if let Some((_, n_params)) = func_sigs.get(name) {
                if variadic.contains(name) {
                    if args.len() < *n_params {
                        return Err(SemaError::new(format!(
                            "too few arguments to function '{}'",
                            name
                        )));
                    }
                } else if args.len() != *n_params {
                    return Err(SemaError::new(format!(
                        "wrong number of arguments to function '{}'",
                        name
                    )));
                }
            }
            for a in args {
                check_call_arity_expr(a, func_sigs, variadic)?;
            }
        }
        Expr::BinOp(_, l, r) => {
            check_call_arity_expr(l, func_sigs, variadic)?;
            check_call_arity_expr(r, func_sigs, variadic)?;
        }
        Expr::UnOp(_, e) => check_call_arity_expr(e, func_sigs, variadic)?,
        Expr::Index(a, b) => {
            check_call_arity_expr(a, func_sigs, variadic)?;
            check_call_arity_expr(b, func_sigs, variadic)?;
        }
        Expr::Member(base, _) => check_call_arity_expr(base, func_sigs, variadic)?,
        Expr::Ternary(c, t, e) => {
            check_call_arity_expr(c, func_sigs, variadic)?;
            check_call_arity_expr(t, func_sigs, variadic)?;
            check_call_arity_expr(e, func_sigs, variadic)?;
        }
        Expr::Cast(_, e) => check_call_arity_expr(e, func_sigs, variadic)?,
        Expr::PostInc(e) | Expr::PostDec(e) | Expr::SizeofExpr(e) => {
            check_call_arity_expr(e, func_sigs, variadic)?;
        }
        Expr::InitList(items) => {
            for item in items {
                check_call_arity_expr(item, func_sigs, variadic)?;
            }
        }
        Expr::Num(_) | Expr::Ident(_) | Expr::StringLit(_) | Expr::Sizeof(_) => {}
    }
    Ok(())
}

fn check_scope_flow(
    params: &[(Type, String)],
    body: &[Stmt],
    known_globals: &HashSet<String>,
    func_sigs: &HashMap<String, (Type, usize)>,
) -> Result<(), SemaError> {
    let mut scope_stack: Vec<HashMap<String, StorageClass>> = Vec::new();
    let mut param_scope: HashMap<String, StorageClass> = HashMap::new();
    for (_, name) in params {
        if param_scope.insert(name.clone(), StorageClass::None).is_some() {
            return Err(SemaError::new(format!("variable '{}' redeclared in the same scope", name)));
        }
    }
    scope_stack.push(param_scope);
    check_sf_stmts(body, &mut scope_stack, known_globals, func_sigs, false, false)?;
    scope_stack.pop();
    Ok(())
}

fn check_sf_stmts(
    stmts: &[Stmt],
    ss: &mut Vec<HashMap<String, StorageClass>>,
    globals: &HashSet<String>,
    func_sigs: &HashMap<String, (Type, usize)>,
    can_break: bool,
    can_continue: bool,
) -> Result<(), SemaError> {
    for stmt in stmts {
        check_sf_stmt(stmt, ss, globals, func_sigs, can_break, can_continue)?;
    }
    Ok(())
}

fn check_sf_stmt(
    stmt: &Stmt,
    ss: &mut Vec<HashMap<String, StorageClass>>,
    globals: &HashSet<String>,
    func_sigs: &HashMap<String, (Type, usize)>,
    can_break: bool,
    can_continue: bool,
) -> Result<(), SemaError> {
    match stmt {
        Stmt::Decl(_, name, init, sc) => {
            if *sc == StorageClass::Extern && init.is_some() {
                return Err(SemaError::new(format!("extern variable '{}' cannot have an initializer", name)));
            }
            if let Some(prev_sc) = ss.last().unwrap().get(name) {
                if !(*sc == StorageClass::Extern && *prev_sc == StorageClass::Extern) {
                    return Err(SemaError::new(format!("variable '{}' redeclared in the same scope", name)));
                }
            } else {
                ss.last_mut().unwrap().insert(name.clone(), sc.clone());
            }
            if let Some(e) = init {
                check_sf_expr(e, ss, globals, func_sigs)?;
            }
        }
        Stmt::Block(stmts) => {
            ss.push(HashMap::new());
            check_sf_stmts(stmts, ss, globals, func_sigs, can_break, can_continue)?;
            ss.pop();
        }
        Stmt::If(cond, then, els) => {
            check_sf_expr(cond, ss, globals, func_sigs)?;
            check_sf_stmt(then, ss, globals, func_sigs, can_break, can_continue)?;
            if let Some(e) = els {
                check_sf_stmt(e, ss, globals, func_sigs, can_break, can_continue)?;
            }
        }
        Stmt::While(cond, body) => {
            check_sf_expr(cond, ss, globals, func_sigs)?;
            check_sf_stmt(body, ss, globals, func_sigs, true, true)?;
        }
        Stmt::DoWhile(body, cond) => {
            check_sf_stmt(body, ss, globals, func_sigs, true, true)?;
            check_sf_expr(cond, ss, globals, func_sigs)?;
        }
        Stmt::For { init, cond, incr, body } => {
            ss.push(HashMap::new());
            if let Some(s) = init { check_sf_stmt(s, ss, globals, func_sigs, false, false)?; }
            if let Some(e) = cond { check_sf_expr(e, ss, globals, func_sigs)?; }
            if let Some(e) = incr { check_sf_expr(e, ss, globals, func_sigs)?; }
            check_sf_stmt(body, ss, globals, func_sigs, true, true)?;
            ss.pop();
        }
        Stmt::Return(e) => {
            if let Some(e) = e { check_sf_expr(e, ss, globals, func_sigs)?; }
        }
        Stmt::Expr(e) => check_sf_expr(e, ss, globals, func_sigs)?,
        Stmt::Break => {
            if !can_break {
                return Err(SemaError::new("break statement not in a loop or switch"));
            }
        }
        Stmt::Continue => {
            if !can_continue {
                return Err(SemaError::new("continue statement not in a loop"));
            }
        }
        Stmt::Goto(_) => {}
        Stmt::Label(_, inner) => {
            check_sf_stmt(inner, ss, globals, func_sigs, can_break, can_continue)?;
        }
        Stmt::Switch { expr, arms } => {
            check_sf_expr(expr, ss, globals, func_sigs)?;
            ss.push(HashMap::new());
            for arm in arms {
                check_sf_stmts(&arm.stmts, ss, globals, func_sigs, true, can_continue)?;
            }
            ss.pop();
        }
    }
    Ok(())
}

fn sf_ident_in_scope(name: &str, ss: &[HashMap<String, StorageClass>]) -> bool {
    ss.iter().rev().any(|s| s.contains_key(name))
}

fn sf_ident_in_current_scope(name: &str, ss: &[HashMap<String, StorageClass>]) -> bool {
    ss.last().map(|s| s.contains_key(name)).unwrap_or(false)
}

fn check_sf_expr(
    expr: &Expr,
    ss: &[HashMap<String, StorageClass>],
    globals: &HashSet<String>,
    func_sigs: &HashMap<String, (Type, usize)>,
) -> Result<(), SemaError> {
    match expr {
        Expr::Ident(name) => {
            if !sf_ident_in_scope(name, ss) && !globals.contains(name) {
                return Err(SemaError::new(format!("undeclared identifier '{}'", name)));
            }
        }
        Expr::Call(name, args) => {
            if sf_ident_in_current_scope(name, ss)
                || (!func_sigs.contains_key(name) && (sf_ident_in_scope(name, ss) || globals.contains(name)))
            {
                return Err(SemaError::new(format!("'{}' is not a function", name)));
            }
            for a in args { check_sf_expr(a, ss, globals, func_sigs)?; }
        }
        Expr::BinOp(_, l, r) => {
            check_sf_expr(l, ss, globals, func_sigs)?;
            check_sf_expr(r, ss, globals, func_sigs)?;
        }
        Expr::UnOp(_, e) => check_sf_expr(e, ss, globals, func_sigs)?,
        Expr::Index(a, b) => {
            check_sf_expr(a, ss, globals, func_sigs)?;
            check_sf_expr(b, ss, globals, func_sigs)?;
        }
        Expr::Member(base, _) => check_sf_expr(base, ss, globals, func_sigs)?,
        Expr::Ternary(c, t, e) => {
            check_sf_expr(c, ss, globals, func_sigs)?;
            check_sf_expr(t, ss, globals, func_sigs)?;
            check_sf_expr(e, ss, globals, func_sigs)?;
        }
        Expr::Cast(_, e) => check_sf_expr(e, ss, globals, func_sigs)?,
        Expr::PostInc(e) | Expr::PostDec(e) | Expr::SizeofExpr(e) => check_sf_expr(e, ss, globals, func_sigs)?,
        Expr::InitList(items) => {
            for item in items { check_sf_expr(item, ss, globals, func_sigs)?; }
        }
        Expr::Num(_) | Expr::StringLit(_) | Expr::Sizeof(_) => {}
    }
    Ok(())
}

fn check_labels_gotos(body: &[Stmt]) -> Result<(), SemaError> {
    let mut labels: HashSet<String> = HashSet::new();
    let mut gotos: Vec<String> = Vec::new();
    collect_labels_gotos_stmts(body, &mut labels, &mut gotos)?;
    for target in &gotos {
        if !labels.contains(target) {
            return Err(SemaError::new(format!("goto target '{}' not declared", target)));
        }
    }
    Ok(())
}

fn collect_labels_gotos_stmts(
    stmts: &[Stmt],
    labels: &mut HashSet<String>,
    gotos: &mut Vec<String>,
) -> Result<(), SemaError> {
    for stmt in stmts {
        collect_labels_gotos_stmt(stmt, labels, gotos)?;
    }
    Ok(())
}

fn collect_labels_gotos_stmt(
    stmt: &Stmt,
    labels: &mut HashSet<String>,
    gotos: &mut Vec<String>,
) -> Result<(), SemaError> {
    match stmt {
        Stmt::Label(name, inner) => {
            if !labels.insert(name.clone()) {
                return Err(SemaError::new(format!("duplicate label '{}'", name)));
            }
            collect_labels_gotos_stmt(inner, labels, gotos)?;
        }
        Stmt::Goto(target) => gotos.push(target.clone()),
        Stmt::Block(stmts) => collect_labels_gotos_stmts(stmts, labels, gotos)?,
        Stmt::If(_, then, els) => {
            collect_labels_gotos_stmt(then, labels, gotos)?;
            if let Some(e) = els { collect_labels_gotos_stmt(e, labels, gotos)?; }
        }
        Stmt::While(_, body) | Stmt::DoWhile(body, _) => {
            collect_labels_gotos_stmt(body, labels, gotos)?;
        }
        Stmt::For { init, body, .. } => {
            if let Some(s) = init { collect_labels_gotos_stmt(s, labels, gotos)?; }
            collect_labels_gotos_stmt(body, labels, gotos)?;
        }
        Stmt::Switch { arms, .. } => {
            for arm in arms {
                collect_labels_gotos_stmts(&arm.stmts, labels, gotos)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn check_duplicate_cases(body: &[Stmt]) -> Result<(), SemaError> {
    for stmt in body {
        check_dup_cases_stmt(stmt)?;
    }
    Ok(())
}

fn check_dup_cases_stmt(stmt: &Stmt) -> Result<(), SemaError> {
    match stmt {
        Stmt::Switch { arms, .. } => {
            let mut seen_cases: HashSet<i32> = HashSet::new();
            let mut seen_default = false;
            for arm in arms {
                for label in &arm.labels {
                    match label {
                        SwitchLabel::Case(n) => {
                            if !seen_cases.insert(*n) {
                                return Err(SemaError::new(format!("duplicate case value {}", n)));
                            }
                        }
                        SwitchLabel::Default => {
                            if seen_default {
                                return Err(SemaError::new("multiple default labels in switch"));
                            }
                            seen_default = true;
                        }
                    }
                }
                for s in &arm.stmts {
                    check_dup_cases_stmt(s)?;
                }
            }
        }
        Stmt::Block(stmts) => {
            for s in stmts { check_dup_cases_stmt(s)?; }
        }
        Stmt::If(_, then, els) => {
            check_dup_cases_stmt(then)?;
            if let Some(e) = els { check_dup_cases_stmt(e)?; }
        }
        Stmt::While(_, body) | Stmt::DoWhile(body, _) => check_dup_cases_stmt(body)?,
        Stmt::For { init, body, .. } => {
            if let Some(s) = init { check_dup_cases_stmt(s)?; }
            check_dup_cases_stmt(body)?;
        }
        Stmt::Label(_, inner) => check_dup_cases_stmt(inner)?,
        _ => {}
    }
    Ok(())
}

fn type_of_expr(
    expr: &Expr,
    vars: &HashMap<String, VarInfo>,
    func_sigs: &HashMap<String, (Type, usize)>,
    struct_defs: &HashMap<String, Vec<(String, Type)>>,
) -> Type {
    match expr {
        Expr::Num(_) => Type::Int,
        Expr::StringLit(_) => Type::Ptr(Box::new(Type::Char)),
        Expr::Ident(name) => vars.get(name).map(|v| v.ty.clone()).unwrap_or(Type::Int),
        Expr::Cast(ty, _) => ty.clone(),
        Expr::UnOp(UnOp::Addr, e) => Type::Ptr(Box::new(type_of_expr(e, vars, func_sigs, struct_defs))),
        Expr::UnOp(UnOp::Deref, e) => match type_of_expr(e, vars, func_sigs, struct_defs) {
            Type::Ptr(inner) => *inner,
            Type::Array(inner, _) => *inner,
            _ => Type::Int,
        },
        Expr::UnOp(_, e) => type_of_expr(e, vars, func_sigs, struct_defs),
        Expr::BinOp(BinOp::Add, l, r) => {
            let lt = type_of_expr(l, vars, func_sigs, struct_defs);
            let rt = type_of_expr(r, vars, func_sigs, struct_defs);
            if matches!(lt, Type::Ptr(_)) {
                lt
            } else if matches!(rt, Type::Ptr(_)) {
                rt
            } else {
                lt
            }
        }
        Expr::BinOp(BinOp::Sub, l, _) => type_of_expr(l, vars, func_sigs, struct_defs),
        Expr::BinOp(BinOp::Assign, lhs, _)
        | Expr::BinOp(BinOp::AddAssign, lhs, _)
        | Expr::BinOp(BinOp::SubAssign, lhs, _)
        | Expr::BinOp(BinOp::MulAssign, lhs, _)
        | Expr::BinOp(BinOp::DivAssign, lhs, _)
        | Expr::BinOp(BinOp::ModAssign, lhs, _)
        | Expr::BinOp(BinOp::AndAssign, lhs, _)
        | Expr::BinOp(BinOp::OrAssign, lhs, _)
        | Expr::BinOp(BinOp::XorAssign, lhs, _)
        | Expr::BinOp(BinOp::ShlAssign, lhs, _)
        | Expr::BinOp(BinOp::ShrAssign, lhs, _) => type_of_expr(lhs, vars, func_sigs, struct_defs),
        Expr::BinOp(_, l, _) => type_of_expr(l, vars, func_sigs, struct_defs),
        Expr::Call(name, _) => func_sigs.get(name).map(|(ret, _)| ret.clone()).unwrap_or(Type::Int),
        Expr::Index(arr, _) => match type_of_expr(arr, vars, func_sigs, struct_defs) {
            Type::Ptr(inner) | Type::Array(inner, _) => *inner,
            _ => Type::Int,
        },
        Expr::Member(base, field) => {
            let bt = type_of_expr(base, vars, func_sigs, struct_defs);
            let struct_name = match &bt {
                Type::Struct(s) => s.clone(),
                Type::Ptr(inner) => match inner.as_ref() {
                    Type::Struct(s) => s.clone(),
                    _ => return Type::Int,
                },
                _ => return Type::Int,
            };
            struct_defs.get(&struct_name)
                .and_then(|fields| fields.iter().find(|(n, _)| n == field))
                .map(|(_, t)| t.clone())
                .unwrap_or(Type::Int)
        }
        Expr::Ternary(_, t, _) => type_of_expr(t, vars, func_sigs, struct_defs),
        Expr::PostInc(e) | Expr::PostDec(e) => type_of_expr(e, vars, func_sigs, struct_defs),
        Expr::InitList(_) => Type::Int,
        Expr::Sizeof(_) | Expr::SizeofExpr(_) => Type::Int,
    }
}

fn is_pointer_or_array(ty: &Type) -> bool {
    matches!(ty, Type::Ptr(_) | Type::Array(_, _))
}

fn is_struct(ty: &Type) -> bool {
    matches!(ty, Type::Struct(_))
}

fn check_types_expr(
    expr: &Expr,
    vars: &HashMap<String, VarInfo>,
    func_sigs: &HashMap<String, (Type, usize)>,
    func_params: &HashMap<String, Vec<Type>>,
    struct_defs: &HashMap<String, Vec<(String, Type)>>,
) -> Result<(), SemaError> {
    match expr {
        Expr::UnOp(op, e) => {
            let et = type_of_expr(e, vars, func_sigs, struct_defs);
            match op {
                UnOp::Neg | UnOp::BitNot => {
                    if is_pointer_or_array(&et) {
                        return Err(SemaError::new("invalid operand: pointer type in arithmetic/bitwise operation"));
                    }
                    if is_struct(&et) {
                        return Err(SemaError::new("invalid operand: struct type in unary operation"));
                    }
                }
                UnOp::Not => {
                    if is_struct(&et) {
                        return Err(SemaError::new("invalid operand: struct in logical not"));
                    }
                }
                UnOp::Deref => {
                    if !is_pointer_or_array(&et) {
                        return Err(SemaError::new("cannot dereference non-pointer type"));
                    }
                }
                _ => {}
            }
            check_types_expr(e, vars, func_sigs, func_params, struct_defs)?;
        }
        Expr::BinOp(op, l, r) => {
            let lt = type_of_expr(l, vars, func_sigs, struct_defs);
            let rt = type_of_expr(r, vars, func_sigs, struct_defs);
            match op {
                BinOp::Mul | BinOp::Div | BinOp::Mod |
                BinOp::MulAssign | BinOp::DivAssign | BinOp::ModAssign => {
                    if is_pointer_or_array(&lt) || is_pointer_or_array(&rt) {
                        return Err(SemaError::new("invalid operand: pointer type in multiply/divide/mod"));
                    }
                }
                BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr |
                BinOp::AndAssign | BinOp::OrAssign | BinOp::XorAssign |
                BinOp::ShlAssign | BinOp::ShrAssign => {
                    if is_pointer_or_array(&lt) || is_pointer_or_array(&rt) {
                        return Err(SemaError::new("invalid operand: pointer type in bitwise operation"));
                    }
                }
                BinOp::Add | BinOp::AddAssign => {
                    if is_pointer_or_array(&lt) && is_pointer_or_array(&rt) {
                        return Err(SemaError::new("invalid operands: cannot add two pointers"));
                    }
                    // Compound assign to array is illegal (array is not assignable)
                    if *op == BinOp::AddAssign && matches!(lt, Type::Array(_, _)) {
                        return Err(SemaError::new("cannot apply += to array type (arrays are not assignable)"));
                    }
                }
                BinOp::Sub | BinOp::SubAssign => {
                    if !is_pointer_or_array(&lt) && is_pointer_or_array(&rt) {
                        return Err(SemaError::new("invalid operands: cannot subtract pointer from integer"));
                    }
                    // Compound assign to array is illegal (array is not assignable)
                    if *op == BinOp::SubAssign && matches!(lt, Type::Array(_, _)) {
                        return Err(SemaError::new("cannot apply -= to array type (arrays are not assignable)"));
                    }
                }
                BinOp::Assign => {
                    // Cannot assign to an array
                    if matches!(lt, Type::Array(_, _)) {
                        return Err(SemaError::new("cannot assign to array type"));
                    }
                    if is_pointer_or_array(&lt) && !is_pointer_or_array(&rt) {
                        if !matches!(r.as_ref(), Expr::Num(0) | Expr::Cast(Type::Ptr(_), _)) {
                            return Err(SemaError::new("incompatible types: assigning integer to pointer"));
                        }
                    }
                    if !is_pointer_or_array(&lt) && is_pointer_or_array(&rt) && !is_struct(&lt) {
                        return Err(SemaError::new("incompatible types: assigning pointer to integer"));
                    }
                    if let (Type::Ptr(lt_inner), Type::Ptr(rt_inner)) = (&lt, &rt) {
                        if lt_inner != rt_inner
                            && !matches!(rt_inner.as_ref(), Type::Void)
                            && !matches!(lt_inner.as_ref(), Type::Void)
                        {
                            return Err(SemaError::new("incompatible pointer types in assignment"));
                        }
                    }
                }
                BinOp::Eq | BinOp::Ne => {
                    // Helper: get the "effective pointer inner type" treating Array(T, n) as Ptr(T)
                    fn effective_inner(ty: &Type) -> Option<&Type> {
                        match ty {
                            Type::Ptr(inner) => Some(inner.as_ref()),
                            Type::Array(inner, _) => Some(inner.as_ref()),
                            _ => None,
                        }
                    }
                    if let (Some(l_inner), Some(r_inner)) = (effective_inner(&lt), effective_inner(&rt)) {
                        if l_inner != r_inner
                            && !matches!(l_inner, Type::Void)
                            && !matches!(r_inner, Type::Void)
                        {
                            return Err(SemaError::new("comparison of pointers to different types"));
                        }
                    }
                    // pointer == integer: only null pointer constant (literal 0) is allowed
                    if (is_pointer_or_array(&lt) && !is_pointer_or_array(&rt))
                        || (!is_pointer_or_array(&lt) && is_pointer_or_array(&rt))
                    {
                        let non_ptr_is_zero = match (&lt, &rt) {
                            (_, t) if !is_pointer_or_array(t) => matches!(r.as_ref(), Expr::Num(0)),
                            (t, _) if !is_pointer_or_array(t) => matches!(l.as_ref(), Expr::Num(0)),
                            _ => false,
                        };
                        if !non_ptr_is_zero {
                            return Err(SemaError::new("comparison between pointer and integer"));
                        }
                    }
                }
                BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                    // Ordering comparisons: pointer vs pointer must match types; pointer vs integer always illegal
                    if let (Type::Ptr(l_inner), Type::Ptr(r_inner)) = (&lt, &rt) {
                        if l_inner != r_inner
                            && !matches!(l_inner.as_ref(), Type::Void)
                            && !matches!(r_inner.as_ref(), Type::Void)
                        {
                            return Err(SemaError::new("comparison of pointers to different types"));
                        }
                    }
                    if (is_pointer_or_array(&lt) && !is_pointer_or_array(&rt))
                        || (!is_pointer_or_array(&lt) && is_pointer_or_array(&rt))
                    {
                        return Err(SemaError::new("comparison between pointer and integer"));
                    }
                }
                _ => {}
            }
            check_types_expr(l, vars, func_sigs, func_params, struct_defs)?;
            check_types_expr(r, vars, func_sigs, func_params, struct_defs)?;
        }
        Expr::Call(name, args) => {
            // Check argument types against parameter types
            if let Some(param_types) = func_params.get(name) {
                for (i, (arg, param_ty)) in args.iter().zip(param_types.iter()).enumerate() {
                    let arg_ty = type_of_expr(arg, vars, func_sigs, struct_defs);
                    // pointer passed where non-pointer expected
                    if is_pointer_or_array(&arg_ty) && !is_pointer_or_array(param_ty) && !is_struct(param_ty) {
                        return Err(SemaError::new(format!(
                            "incompatible argument type for parameter {}: cannot pass pointer as integer", i + 1
                        )));
                    }
                    // non-pointer passed where pointer expected
                    if !is_pointer_or_array(&arg_ty) && is_pointer_or_array(param_ty) {
                        if !matches!(arg, Expr::Num(0)) {
                            return Err(SemaError::new(format!(
                                "incompatible argument type for parameter {}: cannot pass integer as pointer", i + 1
                            )));
                        }
                    }
                    // incompatible pointer types: Ptr(X) vs Ptr(Y) where X != Y and neither is void
                    // Also catch Array(X,n) vs Ptr(Y) where X != Y (e.g., &arr passed where int** expected)
                    let arg_inner = match &arg_ty {
                        Type::Ptr(inner) => Some(inner.as_ref()),
                        Type::Array(inner, _) => Some(inner.as_ref()),
                        _ => None,
                    };
                    if let (Some(arg_inner), Type::Ptr(param_inner)) = (arg_inner, param_ty) {
                        if arg_inner != param_inner.as_ref()
                            && !matches!(arg_inner, Type::Void)
                            && !matches!(param_inner.as_ref(), Type::Void)
                        {
                            return Err(SemaError::new(format!(
                                "incompatible pointer types for parameter {}", i + 1
                            )));
                        }
                    }
                }
            }
            for a in args { check_types_expr(a, vars, func_sigs, func_params, struct_defs)?; }
        }
        Expr::PostInc(e) | Expr::PostDec(e) | Expr::Cast(_, e) | Expr::Member(e, _) => {
            // PostInc/PostDec on array is illegal
            if matches!(expr, Expr::PostInc(_) | Expr::PostDec(_)) {
                let et = type_of_expr(e, vars, func_sigs, struct_defs);
                if matches!(et, Type::Array(_, _)) {
                    return Err(SemaError::new("cannot apply ++ or -- to array type"));
                }
            }
            check_types_expr(e, vars, func_sigs, func_params, struct_defs)?;
        }
        Expr::Index(a, b) => {
            let at = type_of_expr(a, vars, func_sigs, struct_defs);
            let bt = type_of_expr(b, vars, func_sigs, struct_defs);
            if !is_pointer_or_array(&at) && !is_pointer_or_array(&bt) {
                return Err(SemaError::new("subscript operator requires a pointer/array"));
            }
            if is_pointer_or_array(&at) && is_pointer_or_array(&bt) {
                return Err(SemaError::new("subscript with two pointers"));
            }
            check_types_expr(a, vars, func_sigs, func_params, struct_defs)?;
            check_types_expr(b, vars, func_sigs, func_params, struct_defs)?;
        }
        Expr::Ternary(c, t, e) => {
            check_types_expr(c, vars, func_sigs, func_params, struct_defs)?;
            check_types_expr(t, vars, func_sigs, func_params, struct_defs)?;
            check_types_expr(e, vars, func_sigs, func_params, struct_defs)?;
        }
        Expr::InitList(items) => {
            for item in items { check_types_expr(item, vars, func_sigs, func_params, struct_defs)?; }
        }
        Expr::SizeofExpr(e) => check_types_expr(e, vars, func_sigs, func_params, struct_defs)?,
        Expr::Num(_) | Expr::StringLit(_) | Expr::Ident(_) | Expr::Sizeof(_) => {}
    }
    Ok(())
}

fn check_types_stmt(
    stmt: &Stmt,
    vars: &HashMap<String, VarInfo>,
    func_sigs: &HashMap<String, (Type, usize)>,
    func_params: &HashMap<String, Vec<Type>>,
    struct_defs: &HashMap<String, Vec<(String, Type)>>,
    ret_ty: &Type,
) -> Result<(), SemaError> {
    match stmt {
        Stmt::Decl(ty, _, init, _) => {
            if *ty == Type::Void {
                return Err(SemaError::new("variable cannot have void type"));
            }
            if let Some(e) = init {
                // Array initializer checks
                if let Type::Array(_, n) = ty {
                    match e {
                        Expr::InitList(items) => {
                            if *n > 0 && items.len() > *n {
                                return Err(SemaError::new(format!(
                                    "too many initializers for array (expected at most {}, got {})", n, items.len()
                                )));
                            }
                        }
                        Expr::StringLit(_) => {} // string literal initializes char array — OK
                        _ => return Err(SemaError::new("array must be initialized with a compound initializer")),
                    }
                } else if let Expr::InitList(items) = e {
                    // Scalar initialized with multi-item compound initializer
                    if items.len() > 1 {
                        return Err(SemaError::new("too many values in initializer for scalar type"));
                    }
                } else {
                    // Check that pointer variables aren't initialized with non-pointer non-zero values
                    if matches!(ty, Type::Ptr(_)) {
                        let init_ty = type_of_expr(e, vars, func_sigs, struct_defs);
                        if !is_pointer_or_array(&init_ty) {
                            if !matches!(e, Expr::Num(0) | Expr::Cast(Type::Ptr(_), _)) {
                                return Err(SemaError::new("incompatible types: initializing pointer with integer"));
                            }
                        }
                    }
                }
                check_types_expr(e, vars, func_sigs, func_params, struct_defs)?;
            }
        }
        Stmt::Return(Some(e)) => {
            if *ret_ty == Type::Void {
                return Err(SemaError::new("returning a value from a void function"));
            }
            check_types_expr(e, vars, func_sigs, func_params, struct_defs)?;
        }
        Stmt::Return(None) => {}
        Stmt::Expr(e) => check_types_expr(e, vars, func_sigs, func_params, struct_defs)?,
        Stmt::If(cond, then, els) => {
            let ct = type_of_expr(cond, vars, func_sigs, struct_defs);
            if ct == Type::Void { return Err(SemaError::new("void type in condition")); }
            if is_struct(&ct) { return Err(SemaError::new("struct type in condition")); }
            check_types_expr(cond, vars, func_sigs, func_params, struct_defs)?;
            check_types_stmt(then, vars, func_sigs, func_params, struct_defs, ret_ty)?;
            if let Some(e) = els { check_types_stmt(e, vars, func_sigs, func_params, struct_defs, ret_ty)?; }
        }
        Stmt::While(cond, body) => {
            let ct = type_of_expr(cond, vars, func_sigs, struct_defs);
            if ct == Type::Void { return Err(SemaError::new("void type in condition")); }
            if is_struct(&ct) { return Err(SemaError::new("struct type in condition")); }
            check_types_expr(cond, vars, func_sigs, func_params, struct_defs)?;
            check_types_stmt(body, vars, func_sigs, func_params, struct_defs, ret_ty)?;
        }
        Stmt::DoWhile(body, cond) => {
            check_types_stmt(body, vars, func_sigs, func_params, struct_defs, ret_ty)?;
            let ct = type_of_expr(cond, vars, func_sigs, struct_defs);
            if ct == Type::Void { return Err(SemaError::new("void type in condition")); }
            if is_struct(&ct) { return Err(SemaError::new("struct type in condition")); }
            check_types_expr(cond, vars, func_sigs, func_params, struct_defs)?;
        }
        Stmt::For { init, cond, incr, body } => {
            if let Some(s) = init { check_types_stmt(s, vars, func_sigs, func_params, struct_defs, ret_ty)?; }
            if let Some(e) = cond {
                let ct = type_of_expr(e, vars, func_sigs, struct_defs);
                if ct == Type::Void { return Err(SemaError::new("void type in for condition")); }
                if is_struct(&ct) { return Err(SemaError::new("struct type in condition")); }
                check_types_expr(e, vars, func_sigs, func_params, struct_defs)?;
            }
            if let Some(e) = incr { check_types_expr(e, vars, func_sigs, func_params, struct_defs)?; }
            check_types_stmt(body, vars, func_sigs, func_params, struct_defs, ret_ty)?;
        }
        Stmt::Switch { expr, arms } => {
            let et = type_of_expr(expr, vars, func_sigs, struct_defs);
            if is_pointer_or_array(&et) || is_struct(&et) || et == Type::Void {
                return Err(SemaError::new("invalid controlling expression type in switch (must be integer)"));
            }
            check_types_expr(expr, vars, func_sigs, func_params, struct_defs)?;
            for arm in arms {
                for s in &arm.stmts {
                    check_types_stmt(s, vars, func_sigs, func_params, struct_defs, ret_ty)?;
                }
            }
        }
        Stmt::Block(stmts) => {
            for s in stmts { check_types_stmt(s, vars, func_sigs, func_params, struct_defs, ret_ty)?; }
        }
        Stmt::Label(_, inner) => check_types_stmt(inner, vars, func_sigs, func_params, struct_defs, ret_ty)?,
        Stmt::Break | Stmt::Continue | Stmt::Goto(_) => {}
    }
    Ok(())
}

fn check_types_in_func(
    body: &[Stmt],
    vars: &HashMap<String, VarInfo>,
    func_sigs: &HashMap<String, (Type, usize)>,
    func_params: &HashMap<String, Vec<Type>>,
    struct_defs: &HashMap<String, Vec<(String, Type)>>,
    ret_ty: &Type,
) -> Result<(), SemaError> {
    for stmt in body {
        check_types_stmt(stmt, vars, func_sigs, func_params, struct_defs, ret_ty)?;
    }
    Ok(())
}

fn analyze_func(
    mut f: FuncDef,
    globals: &HashMap<String, VarInfo>,
    struct_defs: &HashMap<String, Vec<(String, Type)>>,
    func_sigs: &HashMap<String, (Type, usize)>,
    func_params: &HashMap<String, Vec<Type>>,
) -> Result<(AnnotatedFunc, Vec<(String, Type, Option<GlobalInit>)>), SemaError> {
    let known_globals: HashSet<String> = globals.keys()
        .chain(func_sigs.keys())
        .cloned()
        .collect();
    check_scope_flow(&f.params, &f.body, &known_globals, func_sigs)?;
    check_labels_gotos(&f.body)?;
    check_duplicate_cases(&f.body)?;

    // Alpha-rename shadowed local variables so the flat HashMap can handle them.
    alpha_rename_func(&f.params, &mut f.body);

    let mut vars: HashMap<String, VarInfo> = HashMap::new();

    // Insert params — offsets are word-based (Long params take 2 slots)
    let mut param_offset = 0usize;
    for (ty, name) in f.params.iter() {
        vars.insert(name.clone(), VarInfo {
            ty: ty.clone(),
            storage: VarStorage::Param(param_offset),
        });
        param_offset += type_size(ty, struct_defs).max(1);
    }

    // Collect locals from body
    let mut local_idx = 0usize;
    let mut static_locals: Vec<(String, Type, Option<GlobalInit>)> = Vec::new();
    collect_locals(&f.body, &mut vars, &mut local_idx, struct_defs, &f.name, &mut static_locals)?;

    // Merge globals (lower priority)
    for (name, info) in globals {
        vars.entry(name.clone()).or_insert_with(|| info.clone());
    }

    check_types_in_func(&f.body, &vars, func_sigs, func_params, struct_defs, &f.ret_ty)?;

    Ok((AnnotatedFunc {
        name: f.name,
        ret_ty: f.ret_ty,
        params: f.params,
        n_locals: local_idx,
        body: f.body,
        vars,
    }, static_locals))
}


fn collect_locals(
    stmts: &[Stmt],
    vars: &mut HashMap<String, VarInfo>,
    next_idx: &mut usize,
    struct_defs: &HashMap<String, Vec<(String, Type)>>,
    func_name: &str,
    static_locals: &mut Vec<(String, Type, Option<GlobalInit>)>,
) -> Result<(), SemaError> {
    for stmt in stmts {
        collect_locals_stmt(stmt, vars, next_idx, struct_defs, func_name, static_locals)?;
    }
    Ok(())
}

fn collect_locals_stmt(
    stmt: &Stmt,
    vars: &mut HashMap<String, VarInfo>,
    next_idx: &mut usize,
    struct_defs: &HashMap<String, Vec<(String, Type)>>,
    func_name: &str,
    static_locals: &mut Vec<(String, Type, Option<GlobalInit>)>,
) -> Result<(), SemaError> {
    match stmt {
        Stmt::Decl(ty, name, init_expr, sc) => {
            match sc {
                StorageClass::Static => {
                    // Static local â€” lives in global storage, not on the stack frame.
                    let sym = format!("__sl_{}_{}", func_name, name);
                    let init_val = match init_expr {
                        Some(e) => Some(eval_global_init(e, ty)?),
                        None => None,
                    };
                    vars.insert(name.clone(), VarInfo { ty: ty.clone(), storage: VarStorage::Global(sym.clone()) });
                    static_locals.push((sym, ty.clone(), init_val));
                }
                StorageClass::Extern => {
                    // Local extern declaration â€” refers to the file-scope global with the
                    // base name (strip any $N alpha-rename suffix to recover original C name).
                    let base = if let Some(pos) = name.rfind('$') {
                        name[..pos].to_string()
                    } else {
                        name.clone()
                    };
                    let sym = format!("__g_{}", base);
                    vars.insert(name.clone(), VarInfo { ty: ty.clone(), storage: VarStorage::Global(sym) });
                    // No stack slot allocated.
                }
                StorageClass::None => {
                    // For unsized arrays (e.g. `char arr[] = "hello"`), infer size from the
                    // string literal initializer: s.len() + 1 (to include null terminator).
                    let resolved_ty = if let (Type::Array(base, 0), Some(Expr::StringLit(s))) = (ty, init_expr) {
                        Type::Array(base.clone(), s.len() + 1)
                    } else {
                        ty.clone()
                    };
                    let size = type_size(&resolved_ty, struct_defs).max(1);
                    let idx = *next_idx;
                    *next_idx += size;
                    vars.insert(name.clone(), VarInfo { ty: resolved_ty, storage: VarStorage::Local(idx) });
                }
            }
        }
        Stmt::Block(stmts) => collect_locals(stmts, vars, next_idx, struct_defs, func_name, static_locals)?,
        Stmt::If(_, then, els) => {
            collect_locals_stmt(then, vars, next_idx, struct_defs, func_name, static_locals)?;
            if let Some(e) = els { collect_locals_stmt(e, vars, next_idx, struct_defs, func_name, static_locals)?; }
        }
        Stmt::While(_, body) => collect_locals_stmt(body, vars, next_idx, struct_defs, func_name, static_locals)?,
        Stmt::DoWhile(body, _) => collect_locals_stmt(body, vars, next_idx, struct_defs, func_name, static_locals)?,
        Stmt::For { init, body, .. } => {
            if let Some(s) = init { collect_locals_stmt(s, vars, next_idx, struct_defs, func_name, static_locals)?; }
            collect_locals_stmt(body, vars, next_idx, struct_defs, func_name, static_locals)?;
        }
        Stmt::Switch { arms, .. } => {
            for arm in arms {
                collect_locals(arm.stmts.as_slice(), vars, next_idx, struct_defs, func_name, static_locals)?;
            }
        }
        Stmt::Return(_) | Stmt::Expr(_) | Stmt::Break | Stmt::Continue | Stmt::Goto(_) => {}
        Stmt::Label(_, stmt) => collect_locals_stmt(stmt, vars, next_idx, struct_defs, func_name, static_locals)?,
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
    Expr::BinOp(
        BinOp::Assign,
        Box::new(lhs.clone()),
        Box::new(Expr::BinOp(arith_op, Box::new(lhs), Box::new(rhs))),
    )
}

// â”€â”€ String literal collection â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn intern_string(
    s: &str,
    map: &mut HashMap<String, String>,
    lits: &mut Vec<(String, Vec<i16>)>,
    counter: &mut usize,
) {
    if !map.contains_key(s) {
        let sym_prefix = format!("__str_{}", *counter);
        *counter += 1;
        let chars: Vec<i16> = s.bytes().map(|b| b as i16).collect();
        map.insert(s.to_string(), sym_prefix.clone());
        lits.push((sym_prefix, chars));
    }
}

fn collect_strings_expr(
    expr: &Expr,
    map: &mut HashMap<String, String>,
    lits: &mut Vec<(String, Vec<i16>)>,
    counter: &mut usize,
) {
    match expr {
        Expr::StringLit(s) => intern_string(s, map, lits, counter),
        Expr::BinOp(_, l, r) => {
            collect_strings_expr(l, map, lits, counter);
            collect_strings_expr(r, map, lits, counter);
        }
        Expr::UnOp(_, e) => collect_strings_expr(e, map, lits, counter),
        Expr::Call(_, args) => {
            for a in args { collect_strings_expr(a, map, lits, counter); }
        }
        Expr::Index(a, b) => {
            collect_strings_expr(a, map, lits, counter);
            collect_strings_expr(b, map, lits, counter);
        }
        Expr::Member(base, _) => collect_strings_expr(base, map, lits, counter),
        Expr::Ternary(c, t, e) => {
            collect_strings_expr(c, map, lits, counter);
            collect_strings_expr(t, map, lits, counter);
            collect_strings_expr(e, map, lits, counter);
        }
        Expr::Cast(_, e) => collect_strings_expr(e, map, lits, counter),
        Expr::PostInc(e) | Expr::PostDec(e) => collect_strings_expr(e, map, lits, counter),
        Expr::InitList(items) => {
            for item in items { collect_strings_expr(item, map, lits, counter); }
        }
        Expr::Num(_) | Expr::Ident(_) | Expr::Sizeof(_) | Expr::SizeofExpr(_) => {}
    }
}

fn collect_strings_stmt(
    stmt: &Stmt,
    map: &mut HashMap<String, String>,
    lits: &mut Vec<(String, Vec<i16>)>,
    counter: &mut usize,
) {
    match stmt {
        Stmt::Expr(e) => collect_strings_expr(e, map, lits, counter),
        Stmt::Return(Some(e)) => collect_strings_expr(e, map, lits, counter),
        Stmt::Decl(_, _, Some(e), _) => collect_strings_expr(e, map, lits, counter),
        Stmt::Block(stmts) => {
            for s in stmts { collect_strings_stmt(s, map, lits, counter); }
        }
        Stmt::If(cond, then, els) => {
            collect_strings_expr(cond, map, lits, counter);
            collect_strings_stmt(then, map, lits, counter);
            if let Some(e) = els { collect_strings_stmt(e, map, lits, counter); }
        }
        Stmt::While(cond, body) => {
            collect_strings_expr(cond, map, lits, counter);
            collect_strings_stmt(body, map, lits, counter);
        }
        Stmt::DoWhile(body, cond) => {
            collect_strings_stmt(body, map, lits, counter);
            collect_strings_expr(cond, map, lits, counter);
        }
        Stmt::For { init, cond, incr, body } => {
            if let Some(s) = init { collect_strings_stmt(s, map, lits, counter); }
            if let Some(e) = cond { collect_strings_expr(e, map, lits, counter); }
            if let Some(e) = incr { collect_strings_expr(e, map, lits, counter); }
            collect_strings_stmt(body, map, lits, counter);
        }
        Stmt::Switch { expr, arms } => {
            collect_strings_expr(expr, map, lits, counter);
            for arm in arms {
                for s in &arm.stmts { collect_strings_stmt(s, map, lits, counter); }
            }
        }
        Stmt::Break | Stmt::Continue => {}
        _ => {}
    }
}

// â”€â”€ Alpha-renaming (variable shadowing support) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Rename shadowed local variables to unique names so the flat variable HashMap
/// used by the rest of the compiler can handle nested scopes with the same name.
/// E.g. `int a = 1; { int a = 2; }` becomes `int a = 1; { int a$1 = 2; }`.
fn alpha_rename_func(params: &[(Type, String)], body: &mut Vec<Stmt>) {
    // Count every base name â€” start at 1 for each param so shadowing locals
    // get the $1 suffix.
    let mut counters: HashMap<String, u32> = HashMap::new();
    let mut initial_scope: HashMap<String, String> = HashMap::new();
    for (_, name) in params {
        counters.insert(name.clone(), 1);
        initial_scope.insert(name.clone(), name.clone());
    }
    let mut scopes: Vec<HashMap<String, String>> = vec![initial_scope];
    alpha_rename_stmts(body, &mut counters, &mut scopes);
}

fn alpha_rename_stmts(
    stmts: &mut Vec<Stmt>,
    counters: &mut HashMap<String, u32>,
    scopes: &mut Vec<HashMap<String, String>>,
) {
    for stmt in stmts {
        alpha_rename_stmt(stmt, counters, scopes);
    }
}

fn alpha_rename_stmt(
    stmt: &mut Stmt,
    counters: &mut HashMap<String, u32>,
    scopes: &mut Vec<HashMap<String, String>>,
) {
    match stmt {
        Stmt::Decl(_, name, init, _sc) => {
            let count = counters.entry(name.clone()).or_insert(0);
            let new_name = if *count == 0 {
                name.clone()
            } else {
                format!("{}${}", name, count)
            };
            *count += 1;
            scopes.last_mut().unwrap().insert(name.clone(), new_name.clone());
            *name = new_name;
            if let Some(e) = init {
                alpha_rename_expr(e, scopes);
            }
        }
        Stmt::Block(stmts) => {
            scopes.push(HashMap::new());
            alpha_rename_stmts(stmts, counters, scopes);
            scopes.pop();
        }
        Stmt::For { init, cond, incr, body } => {
            scopes.push(HashMap::new());
            if let Some(s) = init { alpha_rename_stmt(s, counters, scopes); }
            if let Some(e) = cond { alpha_rename_expr(e, scopes); }
            if let Some(e) = incr { alpha_rename_expr(e, scopes); }
            alpha_rename_stmt(body, counters, scopes);
            scopes.pop();
        }
        Stmt::If(cond, then, els) => {
            alpha_rename_expr(cond, scopes);
            alpha_rename_stmt(then, counters, scopes);
            if let Some(e) = els { alpha_rename_stmt(e, counters, scopes); }
        }
        Stmt::While(cond, body) => {
            alpha_rename_expr(cond, scopes);
            alpha_rename_stmt(body, counters, scopes);
        }
        Stmt::DoWhile(body, cond) => {
            alpha_rename_stmt(body, counters, scopes);
            alpha_rename_expr(cond, scopes);
        }
        Stmt::Return(e) => {
            if let Some(e) = e { alpha_rename_expr(e, scopes); }
        }
        Stmt::Expr(e) => alpha_rename_expr(e, scopes),
        Stmt::Break | Stmt::Continue => {}
        Stmt::Goto(_) => {}
        Stmt::Label(_, stmt) => {
            alpha_rename_stmt(stmt, counters, scopes);
        }
        Stmt::Switch { expr, arms } => {
            alpha_rename_expr(expr, scopes);
            for arm in arms {
                scopes.push(HashMap::new());
                for stmt in &mut arm.stmts {
                    alpha_rename_stmt(stmt, counters, scopes);
                }
                scopes.pop();
            }
        }
    }
}

fn alpha_rename_expr(expr: &mut Expr, scopes: &[HashMap<String, String>]) {
    match expr {
        Expr::Ident(name) => {
            for scope in scopes.iter().rev() {
                if let Some(renamed) = scope.get(name.as_str()) {
                    *name = renamed.clone();
                    return;
                }
            }
        }
        Expr::BinOp(_, l, r) => {
            alpha_rename_expr(l, scopes);
            alpha_rename_expr(r, scopes);
        }
        Expr::UnOp(_, e) => alpha_rename_expr(e, scopes),
        Expr::Call(_, args) => {
            for a in args { alpha_rename_expr(a, scopes); }
        }
        Expr::Index(a, b) => {
            alpha_rename_expr(a, scopes);
            alpha_rename_expr(b, scopes);
        }
        Expr::Member(base, _) => alpha_rename_expr(base, scopes),
        Expr::Ternary(c, t, e) => {
            alpha_rename_expr(c, scopes);
            alpha_rename_expr(t, scopes);
            alpha_rename_expr(e, scopes);
        }
        Expr::Cast(_, e) => alpha_rename_expr(e, scopes),
        Expr::PostInc(e) | Expr::PostDec(e) => alpha_rename_expr(e, scopes),
        Expr::InitList(items) => {
            for item in items { alpha_rename_expr(item, scopes); }
        }
        Expr::Num(_) | Expr::StringLit(_) | Expr::Sizeof(_) | Expr::SizeofExpr(_) => {}
    }
}

