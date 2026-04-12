use thiserror::Error;
use std::collections::HashMap;
use crate::lexer::{Token, TokenKind};

#[derive(Debug, Error, Clone)]
#[error("parse error at {line}:{col}: {msg}")]
pub struct ParseError {
    pub line: u32,
    pub col: u32,
    pub msg: String,
}

impl ParseError {
    fn new(line: u32, col: u32, msg: impl Into<String>) -> Self {
        Self { line, col, msg: msg.into() }
    }
}

// ── AST types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<(Type, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Void,
    Int,
    Char,
    Long,  // treated as int (1 word)
    Ptr(Box<Type>),
    Array(Box<Type>, usize),
    Struct(String),
}

impl Type {
    /// Size in Hack words for non-struct types.
    /// For Struct, returns 0 — use sema::type_size with struct_defs for proper size.
    pub fn size(&self) -> usize {
        match self {
            Type::Void => 0,
            Type::Int | Type::Char | Type::Long => 1,
            Type::Ptr(_) => 1,
            Type::Array(base, n) => base.size() * n,
            Type::Struct(_) => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Le, Gt, Ge,
    And, Or, BitAnd, BitOr, BitXor,
    Shl, Shr,
    Assign, AddAssign, SubAssign,
    MulAssign, DivAssign, ModAssign,
    AndAssign, OrAssign, XorAssign,
    ShlAssign, ShrAssign,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnOp { Neg, Not, BitNot, Addr, Deref }

#[derive(Debug, Clone)]
pub enum Expr {
    Num(i32),
    StringLit(String),
    Ident(String),
    BinOp(BinOp, Box<Expr>, Box<Expr>),
    UnOp(UnOp, Box<Expr>),
    Call(String, Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Sizeof(Type),
    Member(Box<Expr>, String), // expr.field (also used for expr->field after desugaring)
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>), // cond ? then : else
    Cast(Type, Box<Expr>),
    PostInc(Box<Expr>),
    PostDec(Box<Expr>),
    InitList(Vec<Expr>),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Block(Vec<Stmt>),
    If(Expr, Box<Stmt>, Option<Box<Stmt>>),
    While(Expr, Box<Stmt>),
    For {
        init: Option<Box<Stmt>>,
        cond: Option<Expr>,
        incr: Option<Expr>,
        body: Box<Stmt>,
    },
    Return(Option<Expr>),
    Decl(Type, String, Option<Expr>),
    Expr(Expr),
    DoWhile(Box<Stmt>, Expr),
    Break,
    Continue,
    Switch { expr: Expr, arms: Vec<SwitchArm> },
}

#[derive(Debug, Clone)]
pub struct SwitchArm {
    pub labels: Vec<SwitchLabel>,
    pub stmts: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub enum SwitchLabel {
    Case(i32),
    Default,
}

#[derive(Debug, Clone)]
pub struct FuncDef {
    pub ret_ty: Type,
    pub name: String,
    pub params: Vec<(Type, String)>,
    pub body: Vec<Stmt>,
    /// True when this is a forward declaration (no body), false for a definition.
    pub is_decl: bool,
    pub is_variadic: bool,
}

#[derive(Debug, Clone)]
pub struct Program {
    pub struct_defs: Vec<StructDef>,
    pub globals: Vec<(Type, String, Option<Expr>)>,
    pub funcs: Vec<FuncDef>,
}

// ── Parser ───────────────────────────────────────────────────────────────────

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    struct_defs: Vec<StructDef>,
    typedef_map: HashMap<String, Type>,
    enum_map: HashMap<String, i32>,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0, struct_defs: Vec::new(), typedef_map: HashMap::new(), enum_map: HashMap::new() }
    }

    fn cur(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn cur_lc(&self) -> (u32, u32) {
        let t = self.cur();
        (t.line, t.col)
    }

    fn peek(&self) -> &TokenKind {
        &self.cur().kind
    }

    fn peek_at(&self, offset: usize) -> &TokenKind {
        let idx = (self.pos + offset).min(self.tokens.len() - 1);
        &self.tokens[idx].kind
    }

    fn advance(&mut self) -> &Token {
        let t = &self.tokens[self.pos];
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        t
    }

    fn expect(&mut self, kind: &TokenKind) -> Result<(), ParseError> {
        if self.peek() == kind {
            self.advance();
            Ok(())
        } else {
            let (l, c) = self.cur_lc();
            Err(ParseError::new(
                l, c,
                format!("expected {:?}, got {:?}", kind, self.peek()),
            ))
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        let (l, c) = self.cur_lc();
        match self.peek().clone() {
            TokenKind::Ident(s) => { self.advance(); Ok(s) }
            got => Err(ParseError::new(l, c, format!("expected identifier, got {:?}", got))),
        }
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.peek() == kind {
            self.advance();
            true
        } else {
            false
        }
    }

    // ── Type parsing ──────────────────────────────────────────────────────

    fn parse_base_type(&mut self) -> Result<Type, ParseError> {
        let (l, c) = self.cur_lc();
        match self.peek().clone() {
            TokenKind::KwConst | TokenKind::KwExtern | TokenKind::KwStatic => {
                self.advance();
                return self.parse_base_type();
            }
            TokenKind::KwUnsigned | TokenKind::KwSigned => {
                self.advance();
                match self.peek().clone() {
                    TokenKind::KwInt  => { self.advance(); return Ok(Type::Int); }
                    TokenKind::KwChar => { self.advance(); return Ok(Type::Char); }
                    TokenKind::KwLong => {
                        self.advance();
                        if *self.peek() == TokenKind::KwLong { self.advance(); }
                        if *self.peek() == TokenKind::KwInt  { self.advance(); }
                        return Ok(Type::Long);
                    }
                    _ => return Ok(Type::Int),
                }
            }
            TokenKind::KwLong => {
                self.advance();
                if *self.peek() == TokenKind::KwLong { self.advance(); }
                if *self.peek() == TokenKind::KwInt  { self.advance(); }
                return Ok(Type::Long);
            }
            TokenKind::KwShort => {
                self.advance();
                if *self.peek() == TokenKind::KwInt { self.advance(); }
                return Ok(Type::Int);
            }
            TokenKind::KwInt  => { self.advance(); return Ok(Type::Int); }
            TokenKind::KwChar => { self.advance(); return Ok(Type::Char); }
            TokenKind::KwVoid => { self.advance(); return Ok(Type::Void); }
            TokenKind::KwStruct => {
                self.advance();
                let name = self.expect_ident()?;
                return Ok(Type::Struct(name));
            }
            TokenKind::KwEnum => {
                self.advance();
                let _name = self.expect_ident()?;
                return Ok(Type::Int);
            }
            TokenKind::Ident(s) if self.typedef_map.contains_key(&s) => {
                let ty = self.typedef_map[&s].clone();
                self.advance();
                return Ok(ty);
            }
            got => return Err(ParseError::new(l, c, format!("expected type, got {:?}", got))),
        }
    }

    /// Parse type + pointer stars: e.g. `int**`
    fn parse_type(&mut self) -> Result<Type, ParseError> {
        let mut ty = self.parse_base_type()?;
        while self.eat(&TokenKind::Star) {
            ty = Type::Ptr(Box::new(ty));
        }
        Ok(ty)
    }

    /// Parse type including trailing array brackets: `int arr[N]`
    /// Returns (type, name)
    fn parse_typed_decl(&mut self) -> Result<(Type, String), ParseError> {
        let mut ty = self.parse_type()?;
        let name = self.expect_ident()?;
        let mut dims = Vec::new();
        while self.eat(&TokenKind::LBracket) {
            let n = self.parse_const_int()?;
            self.expect(&TokenKind::RBracket)?;
            dims.push(n as usize);
        }
        for &dim in dims.iter().rev() {
            ty = Type::Array(Box::new(ty), dim);
        }
        Ok((ty, name))
    }

    fn parse_const_int(&mut self) -> Result<i32, ParseError> {
        let (l, c) = self.cur_lc();
        match self.peek().clone() {
            TokenKind::Number(n) => { self.advance(); Ok(n) }
            got => Err(ParseError::new(l, c, format!("expected integer literal, got {:?}", got))),
        }
    }

    fn parse_const_int_signed(&mut self) -> Result<i32, ParseError> {
        let neg = self.eat(&TokenKind::Minus);
        let n = self.parse_const_int()?;
        Ok(if neg { -n } else { n })
    }

    // ── Program ───────────────────────────────────────────────────────────

    fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut globals = Vec::new();
        let mut funcs = Vec::new();

        while *self.peek() != TokenKind::Eof {
            if *self.peek() == TokenKind::KwTypedef {
                self.advance();
                let ty = self.parse_type()?;
                let name = self.expect_ident()?;
                self.expect(&TokenKind::Semicolon)?;
                self.typedef_map.insert(name, ty);
                continue;
            }

            if *self.peek() == TokenKind::KwEnum
                && matches!(self.peek_at(1), TokenKind::Ident(_))
                && *self.peek_at(2) == TokenKind::LBrace
            {
                self.parse_enum_def()?;
                continue;
            }

            // Standalone struct definition: struct Name { ... };
            if *self.peek() == TokenKind::KwStruct
                && matches!(self.peek_at(1), TokenKind::Ident(_))
                && *self.peek_at(2) == TokenKind::LBrace
            {
                self.parse_struct_def()?;
                continue;
            }

            let (ty, name) = self.parse_typed_decl()?;

            if *self.peek() == TokenKind::LParen {
                funcs.push(self.parse_func_rest(ty, name)?);
            } else {
                let init = if self.eat(&TokenKind::Assign) {
                    Some(self.parse_assign_expr()?)
                } else {
                    None
                };
                self.expect(&TokenKind::Semicolon)?;
                globals.push((ty, name, init));
            }
        }
        let struct_defs = std::mem::take(&mut self.struct_defs);
        Ok(Program { struct_defs, globals, funcs })
    }

    /// Parse `struct Name { field; field; }` — stores into self.struct_defs.
    fn parse_struct_def(&mut self) -> Result<(), ParseError> {
        self.expect(&TokenKind::KwStruct)?;
        let name = self.expect_ident()?;
        self.expect(&TokenKind::LBrace)?;
        let mut fields = Vec::new();
        while *self.peek() != TokenKind::RBrace && *self.peek() != TokenKind::Eof {
            let (ty, fname) = self.parse_typed_decl()?;
            self.expect(&TokenKind::Semicolon)?;
            fields.push((ty, fname));
        }
        self.expect(&TokenKind::RBrace)?;
        self.expect(&TokenKind::Semicolon)?;
        self.struct_defs.push(StructDef { name, fields });
        Ok(())
    }

    fn parse_enum_def(&mut self) -> Result<(), ParseError> {
        self.expect(&TokenKind::KwEnum)?;
        let _name = self.expect_ident()?;
        self.expect(&TokenKind::LBrace)?;
        let mut next_val = 0i32;
        while *self.peek() != TokenKind::RBrace && *self.peek() != TokenKind::Eof {
            let member = self.expect_ident()?;
            if self.eat(&TokenKind::Assign) {
                next_val = self.parse_const_int_signed()?;
            }
            self.enum_map.insert(member, next_val);
            next_val += 1;
            if !self.eat(&TokenKind::Comma) { break; }
        }
        self.expect(&TokenKind::RBrace)?;
        self.expect(&TokenKind::Semicolon)?;
        Ok(())
    }

    fn parse_func_rest(&mut self, ret_ty: Type, name: String) -> Result<FuncDef, ParseError> {
        self.expect(&TokenKind::LParen)?;
        let mut params = Vec::new();
        let mut is_variadic = false;
        if *self.peek() != TokenKind::RParen {
            // Handle `(void)` — C syntax for explicitly-no-parameters
            if *self.peek() == TokenKind::KwVoid && *self.peek_at(1) == TokenKind::RParen {
                self.advance(); // consume `void`
            } else {
                loop {
                    if *self.peek() == TokenKind::DotDotDot {
                        self.advance();
                        is_variadic = true;
                        break;
                    }
                    let (ty, pname) = self.parse_typed_decl()?;
                    params.push((ty, pname));
                    if !self.eat(&TokenKind::Comma) { break; }
                }
            }
        }
        self.expect(&TokenKind::RParen)?;
        // Semicolon here = forward declaration (no body)
        if self.eat(&TokenKind::Semicolon) {
            return Ok(FuncDef { ret_ty, name, params, body: vec![], is_decl: true, is_variadic });
        }
        self.expect(&TokenKind::LBrace)?;
        let body = self.parse_stmts_until_rbrace()?;
        Ok(FuncDef { ret_ty, name, params, body, is_decl: false, is_variadic })
    }

    fn parse_stmts_until_rbrace(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut stmts = Vec::new();
        while *self.peek() != TokenKind::RBrace && *self.peek() != TokenKind::Eof {
            stmts.push(self.parse_stmt()?);
        }
        self.expect(&TokenKind::RBrace)?;
        Ok(stmts)
    }

    // ── Statements ────────────────────────────────────────────────────────

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        match self.peek().clone() {
            TokenKind::LBrace => {
                self.advance();
                let stmts = self.parse_stmts_until_rbrace()?;
                Ok(Stmt::Block(stmts))
            }
            TokenKind::KwIf => {
                self.advance();
                self.expect(&TokenKind::LParen)?;
                let cond = self.parse_expr()?;
                self.expect(&TokenKind::RParen)?;
                let then = Box::new(self.parse_stmt()?);
                let els = if self.eat(&TokenKind::KwElse) {
                    Some(Box::new(self.parse_stmt()?))
                } else {
                    None
                };
                Ok(Stmt::If(cond, then, els))
            }
            TokenKind::KwWhile => {
                self.advance();
                self.expect(&TokenKind::LParen)?;
                let cond = self.parse_expr()?;
                self.expect(&TokenKind::RParen)?;
                let body = Box::new(self.parse_stmt()?);
                Ok(Stmt::While(cond, body))
            }
            TokenKind::KwFor => {
                self.advance();
                self.expect(&TokenKind::LParen)?;
                // init: decl or expr or empty
                let init = if *self.peek() == TokenKind::Semicolon {
                    self.advance();
                    None
                } else if self.is_type_start() {
                    let s = self.parse_decl_stmt()?;
                    // parse_decl_stmt already consumed the semicolon
                    Some(Box::new(s))
                } else {
                    let e = self.parse_expr()?;
                    self.expect(&TokenKind::Semicolon)?;
                    Some(Box::new(Stmt::Expr(e)))
                };
                let cond = if *self.peek() == TokenKind::Semicolon {
                    None
                } else {
                    Some(self.parse_expr()?)
                };
                self.expect(&TokenKind::Semicolon)?;
                let incr = if *self.peek() == TokenKind::RParen {
                    None
                } else {
                    Some(self.parse_expr()?)
                };
                self.expect(&TokenKind::RParen)?;
                let body = Box::new(self.parse_stmt()?);
                Ok(Stmt::For { init, cond, incr, body })
            }
            TokenKind::KwReturn => {
                self.advance();
                let expr = if *self.peek() == TokenKind::Semicolon {
                    None
                } else {
                    Some(self.parse_expr()?)
                };
                self.expect(&TokenKind::Semicolon)?;
                Ok(Stmt::Return(expr))
            }
            TokenKind::KwDo => {
                self.advance();
                let body = Box::new(self.parse_stmt()?);
                self.expect(&TokenKind::KwWhile)?;
                self.expect(&TokenKind::LParen)?;
                let cond = self.parse_expr()?;
                self.expect(&TokenKind::RParen)?;
                self.expect(&TokenKind::Semicolon)?;
                Ok(Stmt::DoWhile(body, cond))
            }
            TokenKind::KwBreak => {
                self.advance();
                self.expect(&TokenKind::Semicolon)?;
                Ok(Stmt::Break)
            }
            TokenKind::KwContinue => {
                self.advance();
                self.expect(&TokenKind::Semicolon)?;
                Ok(Stmt::Continue)
            }
            TokenKind::KwSwitch => {
                self.advance();
                self.expect(&TokenKind::LParen)?;
                let expr = self.parse_expr()?;
                self.expect(&TokenKind::RParen)?;
                self.expect(&TokenKind::LBrace)?;
                let arms = self.parse_switch_body()?;
                Ok(Stmt::Switch { expr, arms })
            }
            TokenKind::KwTypedef => {
                self.advance();
                let ty = self.parse_type()?;
                let name = self.expect_ident()?;
                self.expect(&TokenKind::Semicolon)?;
                self.typedef_map.insert(name, ty);
                Ok(Stmt::Block(vec![]))
            }
            _ if self.is_type_start() => self.parse_decl_stmt(),
            _ => {
                let e = self.parse_expr()?;
                self.expect(&TokenKind::Semicolon)?;
                Ok(Stmt::Expr(e))
            }
        }
    }

    fn is_type_start(&self) -> bool {
        match self.peek() {
            TokenKind::KwInt | TokenKind::KwVoid | TokenKind::KwChar | TokenKind::KwStruct
            | TokenKind::KwLong | TokenKind::KwShort | TokenKind::KwConst | TokenKind::KwExtern
            | TokenKind::KwStatic | TokenKind::KwUnsigned | TokenKind::KwSigned | TokenKind::KwEnum => true,
            TokenKind::Ident(s) => self.typedef_map.contains_key(s),
            _ => false,
        }
    }

    fn is_cast_expr(&self) -> bool {
        match self.peek_at(1) {
            TokenKind::KwInt | TokenKind::KwChar | TokenKind::KwVoid
            | TokenKind::KwLong | TokenKind::KwShort | TokenKind::KwConst
            | TokenKind::KwUnsigned | TokenKind::KwSigned | TokenKind::KwStruct
            | TokenKind::KwEnum => true,
            TokenKind::Ident(s) => self.typedef_map.contains_key(s),
            _ => false,
        }
    }

    fn parse_decl_stmt(&mut self) -> Result<Stmt, ParseError> {
        if *self.peek() == TokenKind::KwTypedef {
            self.advance();
            let ty = self.parse_type()?;
            let name = self.expect_ident()?;
            self.expect(&TokenKind::Semicolon)?;
            self.typedef_map.insert(name, ty);
            return Ok(Stmt::Block(vec![]));
        }
        let (ty, name) = self.parse_typed_decl()?;
        let init = if self.eat(&TokenKind::Assign) {
            if *self.peek() == TokenKind::LBrace {
                Some(self.parse_init_list()?)
            } else {
                Some(self.parse_assign_expr()?)
            }
        } else {
            None
        };
        self.expect(&TokenKind::Semicolon)?;
        Ok(Stmt::Decl(ty, name, init))
    }

    fn parse_init_list(&mut self) -> Result<Expr, ParseError> {
        self.expect(&TokenKind::LBrace)?;
        let mut items = Vec::new();
        if *self.peek() != TokenKind::RBrace {
            loop {
                items.push(self.parse_assign_expr()?);
                if !self.eat(&TokenKind::Comma) { break; }
                if *self.peek() == TokenKind::RBrace { break; }
            }
        }
        self.expect(&TokenKind::RBrace)?;
        Ok(Expr::InitList(items))
    }

    fn parse_switch_body(&mut self) -> Result<Vec<SwitchArm>, ParseError> {
        let mut arms: Vec<SwitchArm> = Vec::new();
        while *self.peek() != TokenKind::RBrace && *self.peek() != TokenKind::Eof {
            let mut labels = Vec::new();
            while matches!(self.peek(), TokenKind::KwCase | TokenKind::KwDefault) {
                if *self.peek() == TokenKind::KwCase {
                    self.advance();
                    let val = self.parse_const_int_signed()?;
                    self.expect(&TokenKind::Colon)?;
                    labels.push(SwitchLabel::Case(val));
                } else {
                    self.advance();
                    self.expect(&TokenKind::Colon)?;
                    labels.push(SwitchLabel::Default);
                }
            }
            if labels.is_empty() { break; }
            let mut stmts = Vec::new();
            while !matches!(self.peek(), TokenKind::RBrace | TokenKind::KwCase | TokenKind::KwDefault | TokenKind::Eof) {
                stmts.push(self.parse_stmt()?);
            }
            arms.push(SwitchArm { labels, stmts });
        }
        self.expect(&TokenKind::RBrace)?;
        Ok(arms)
    }

    // ── Expressions ───────────────────────────────────────────────────────

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_assign_expr()
    }

    fn parse_assign_expr(&mut self) -> Result<Expr, ParseError> {
        let lhs = self.parse_conditional_expr()?;
        if self.eat(&TokenKind::Assign) {
            let rhs = self.parse_assign_expr()?;
            return Ok(Expr::BinOp(BinOp::Assign, Box::new(lhs), Box::new(rhs)));
        }
        if self.eat(&TokenKind::PlusAssign) {
            let rhs = self.parse_assign_expr()?;
            return Ok(Expr::BinOp(BinOp::AddAssign, Box::new(lhs), Box::new(rhs)));
        }
        if self.eat(&TokenKind::MinusAssign) {
            let rhs = self.parse_assign_expr()?;
            return Ok(Expr::BinOp(BinOp::SubAssign, Box::new(lhs), Box::new(rhs)));
        }
        if self.eat(&TokenKind::StarAssign) {
            let rhs = self.parse_assign_expr()?;
            return Ok(Expr::BinOp(BinOp::MulAssign, Box::new(lhs), Box::new(rhs)));
        }
        if self.eat(&TokenKind::SlashAssign) {
            let rhs = self.parse_assign_expr()?;
            return Ok(Expr::BinOp(BinOp::DivAssign, Box::new(lhs), Box::new(rhs)));
        }
        if self.eat(&TokenKind::PercentAssign) {
            let rhs = self.parse_assign_expr()?;
            return Ok(Expr::BinOp(BinOp::ModAssign, Box::new(lhs), Box::new(rhs)));
        }
        if self.eat(&TokenKind::AmpAssign) {
            let rhs = self.parse_assign_expr()?;
            return Ok(Expr::BinOp(BinOp::AndAssign, Box::new(lhs), Box::new(rhs)));
        }
        if self.eat(&TokenKind::PipeAssign) {
            let rhs = self.parse_assign_expr()?;
            return Ok(Expr::BinOp(BinOp::OrAssign, Box::new(lhs), Box::new(rhs)));
        }
        if self.eat(&TokenKind::CaretAssign) {
            let rhs = self.parse_assign_expr()?;
            return Ok(Expr::BinOp(BinOp::XorAssign, Box::new(lhs), Box::new(rhs)));
        }
        if self.eat(&TokenKind::LtLtAssign) {
            let rhs = self.parse_assign_expr()?;
            return Ok(Expr::BinOp(BinOp::ShlAssign, Box::new(lhs), Box::new(rhs)));
        }
        if self.eat(&TokenKind::GtGtAssign) {
            let rhs = self.parse_assign_expr()?;
            return Ok(Expr::BinOp(BinOp::ShrAssign, Box::new(lhs), Box::new(rhs)));
        }
        Ok(lhs)
    }

    /// Ternary: cond ? then : else  (right-associative)
    fn parse_conditional_expr(&mut self) -> Result<Expr, ParseError> {
        let cond = self.parse_or_expr()?;
        if self.eat(&TokenKind::Question) {
            let then = self.parse_assign_expr()?;
            self.expect(&TokenKind::Colon)?;
            let els = self.parse_conditional_expr()?;
            return Ok(Expr::Ternary(Box::new(cond), Box::new(then), Box::new(els)));
        }
        Ok(cond)
    }

    fn parse_or_expr(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_and_expr()?;
        while self.eat(&TokenKind::PipePipe) {
            let rhs = self.parse_and_expr()?;
            lhs = Expr::BinOp(BinOp::Or, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_and_expr(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_bitor_expr()?;
        while self.eat(&TokenKind::AmpAmp) {
            let rhs = self.parse_bitor_expr()?;
            lhs = Expr::BinOp(BinOp::And, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_bitor_expr(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_bitxor_expr()?;
        while self.eat(&TokenKind::Pipe) {
            let rhs = self.parse_bitxor_expr()?;
            lhs = Expr::BinOp(BinOp::BitOr, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_bitxor_expr(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_bitand_expr()?;
        while self.eat(&TokenKind::Caret) {
            let rhs = self.parse_bitand_expr()?;
            lhs = Expr::BinOp(BinOp::BitXor, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_bitand_expr(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_eq_expr()?;
        while self.eat(&TokenKind::Amp) {
            let rhs = self.parse_eq_expr()?;
            lhs = Expr::BinOp(BinOp::BitAnd, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_eq_expr(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_rel_expr()?;
        loop {
            if self.eat(&TokenKind::Eq) {
                lhs = Expr::BinOp(BinOp::Eq, Box::new(lhs), Box::new(self.parse_rel_expr()?));
            } else if self.eat(&TokenKind::Ne) {
                lhs = Expr::BinOp(BinOp::Ne, Box::new(lhs), Box::new(self.parse_rel_expr()?));
            } else { break; }
        }
        Ok(lhs)
    }

    fn parse_rel_expr(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_shift_expr()?;
        loop {
            if self.eat(&TokenKind::Lt) {
                lhs = Expr::BinOp(BinOp::Lt, Box::new(lhs), Box::new(self.parse_shift_expr()?));
            } else if self.eat(&TokenKind::Le) {
                lhs = Expr::BinOp(BinOp::Le, Box::new(lhs), Box::new(self.parse_shift_expr()?));
            } else if self.eat(&TokenKind::Gt) {
                lhs = Expr::BinOp(BinOp::Gt, Box::new(lhs), Box::new(self.parse_shift_expr()?));
            } else if self.eat(&TokenKind::Ge) {
                lhs = Expr::BinOp(BinOp::Ge, Box::new(lhs), Box::new(self.parse_shift_expr()?));
            } else { break; }
        }
        Ok(lhs)
    }

    fn parse_shift_expr(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_add_expr()?;
        loop {
            if self.eat(&TokenKind::LtLt) {
                lhs = Expr::BinOp(BinOp::Shl, Box::new(lhs), Box::new(self.parse_add_expr()?));
            } else if self.eat(&TokenKind::GtGt) {
                lhs = Expr::BinOp(BinOp::Shr, Box::new(lhs), Box::new(self.parse_add_expr()?));
            } else { break; }
        }
        Ok(lhs)
    }

    fn parse_add_expr(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_mul_expr()?;
        loop {
            if self.eat(&TokenKind::Plus) {
                lhs = Expr::BinOp(BinOp::Add, Box::new(lhs), Box::new(self.parse_mul_expr()?));
            } else if self.eat(&TokenKind::Minus) {
                lhs = Expr::BinOp(BinOp::Sub, Box::new(lhs), Box::new(self.parse_mul_expr()?));
            } else { break; }
        }
        Ok(lhs)
    }

    fn parse_mul_expr(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_unary_expr()?;
        loop {
            if self.eat(&TokenKind::Star) {
                lhs = Expr::BinOp(BinOp::Mul, Box::new(lhs), Box::new(self.parse_unary_expr()?));
            } else if self.eat(&TokenKind::Slash) {
                lhs = Expr::BinOp(BinOp::Div, Box::new(lhs), Box::new(self.parse_unary_expr()?));
            } else if self.eat(&TokenKind::Percent) {
                lhs = Expr::BinOp(BinOp::Mod, Box::new(lhs), Box::new(self.parse_unary_expr()?));
            } else { break; }
        }
        Ok(lhs)
    }

    fn parse_unary_expr(&mut self) -> Result<Expr, ParseError> {
        if self.eat(&TokenKind::Minus) {
            return Ok(Expr::UnOp(UnOp::Neg, Box::new(self.parse_unary_expr()?)));
        }
        if self.eat(&TokenKind::Bang) {
            return Ok(Expr::UnOp(UnOp::Not, Box::new(self.parse_unary_expr()?)));
        }
        if self.eat(&TokenKind::Tilde) {
            return Ok(Expr::UnOp(UnOp::BitNot, Box::new(self.parse_unary_expr()?)));
        }
        if self.eat(&TokenKind::Amp) {
            return Ok(Expr::UnOp(UnOp::Addr, Box::new(self.parse_unary_expr()?)));
        }
        if self.eat(&TokenKind::Star) {
            return Ok(Expr::UnOp(UnOp::Deref, Box::new(self.parse_unary_expr()?)));
        }
        if self.eat(&TokenKind::KwSizeof) {
            self.expect(&TokenKind::LParen)?;
            let ty = self.parse_type()?;
            self.expect(&TokenKind::RParen)?;
            return Ok(Expr::Sizeof(ty));
        }
        // prefix ++/-- (lower priority, treat as +=1)
        if self.eat(&TokenKind::PlusPlus) {
            let e = self.parse_postfix_expr()?;
            return Ok(Expr::BinOp(BinOp::AddAssign, Box::new(e), Box::new(Expr::Num(1))));
        }
        if self.eat(&TokenKind::MinusMinus) {
            let e = self.parse_postfix_expr()?;
            return Ok(Expr::BinOp(BinOp::SubAssign, Box::new(e), Box::new(Expr::Num(1))));
        }
        self.parse_postfix_expr()
    }

    fn parse_postfix_expr(&mut self) -> Result<Expr, ParseError> {
        let mut e = self.parse_primary()?;
        loop {
            if self.eat(&TokenKind::LBracket) {
                let idx = self.parse_expr()?;
                self.expect(&TokenKind::RBracket)?;
                e = Expr::Index(Box::new(e), Box::new(idx));
            } else if self.eat(&TokenKind::Dot) {
                let field = self.expect_ident()?;
                e = Expr::Member(Box::new(e), field);
            } else if self.eat(&TokenKind::Arrow) {
                // p->f  ≡  (*p).f
                let field = self.expect_ident()?;
                e = Expr::Member(Box::new(Expr::UnOp(UnOp::Deref, Box::new(e))), field);
            } else if self.eat(&TokenKind::PlusPlus) {
                e = Expr::PostInc(Box::new(e));
            } else if self.eat(&TokenKind::MinusMinus) {
                e = Expr::PostDec(Box::new(e));
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let (l, c) = self.cur_lc();
        match self.peek().clone() {
            TokenKind::Number(n) => { self.advance(); Ok(Expr::Num(n)) }
            TokenKind::CharLit(c) => { self.advance(); Ok(Expr::Num(c as i32)) }
            TokenKind::StringLit(s) => { self.advance(); Ok(Expr::StringLit(s)) }
            TokenKind::Ident(name) => {
                if let Some(&val) = self.enum_map.get(&name) {
                    self.advance();
                    return Ok(Expr::Num(val));
                }
                self.advance();
                if self.eat(&TokenKind::LParen) {
                    let mut args = Vec::new();
                    if *self.peek() != TokenKind::RParen {
                        loop {
                            args.push(self.parse_assign_expr()?);
                            if !self.eat(&TokenKind::Comma) { break; }
                        }
                    }
                    self.expect(&TokenKind::RParen)?;
                    Ok(Expr::Call(name, args))
                } else {
                    Ok(Expr::Ident(name))
                }
            }
            TokenKind::LParen => {
                if self.is_cast_expr() {
                    self.advance();
                    let ty = self.parse_type()?;
                    self.expect(&TokenKind::RParen)?;
                    let inner = self.parse_unary_expr()?;
                    return Ok(Expr::Cast(ty, Box::new(inner)));
                }
                self.advance();
                let e = self.parse_expr()?;
                self.expect(&TokenKind::RParen)?;
                Ok(e)
            }
            got => Err(ParseError::new(l, c, format!("unexpected token in expression: {:?}", got))),
        }
    }
}

pub fn parse(tokens: Vec<Token>) -> Result<Program, ParseError> {
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}
