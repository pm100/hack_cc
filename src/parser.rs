use thiserror::Error;
use crate::lexer::{Token, TokenKind};

#[derive(Debug, Error, Clone)]
#[error("parse error at position {pos}: {msg}")]
pub struct ParseError {
    pub pos: usize,
    pub msg: String,
}

impl ParseError {
    fn new(pos: usize, msg: impl Into<String>) -> Self {
        Self { pos, msg: msg.into() }
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
            Type::Int | Type::Char => 1,
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
    And, Or, BitAnd, BitOr,
    Assign, AddAssign, SubAssign,
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
}

#[derive(Debug, Clone)]
pub struct FuncDef {
    pub ret_ty: Type,
    pub name: String,
    pub params: Vec<(Type, String)>,
    pub body: Vec<Stmt>,
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
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0, struct_defs: Vec::new() }
    }

    fn cur(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn cur_pos(&self) -> usize {
        self.cur().pos
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
            Err(ParseError::new(
                self.cur_pos(),
                format!("expected {:?}, got {:?}", kind, self.peek()),
            ))
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        let pos = self.cur_pos();
        match self.peek().clone() {
            TokenKind::Ident(s) => { self.advance(); Ok(s) }
            got => Err(ParseError::new(pos, format!("expected identifier, got {:?}", got))),
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
        let pos = self.cur_pos();
        match self.peek().clone() {
            TokenKind::KwInt  => { self.advance(); Ok(Type::Int) }
            TokenKind::KwChar => { self.advance(); Ok(Type::Char) }
            TokenKind::KwVoid => { self.advance(); Ok(Type::Void) }
            TokenKind::KwStruct => {
                self.advance();
                let name = self.expect_ident()?;
                Ok(Type::Struct(name))
            }
            got => Err(ParseError::new(pos, format!("expected type, got {:?}", got))),
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
        // array suffix
        if self.eat(&TokenKind::LBracket) {
            let n = self.parse_const_int()?;
            self.expect(&TokenKind::RBracket)?;
            ty = Type::Array(Box::new(ty), n as usize);
        }
        Ok((ty, name))
    }

    fn parse_const_int(&mut self) -> Result<i32, ParseError> {
        let pos = self.cur_pos();
        match self.peek().clone() {
            TokenKind::Number(n) => { self.advance(); Ok(n) }
            got => Err(ParseError::new(pos, format!("expected integer literal, got {:?}", got))),
        }
    }

    // ── Program ───────────────────────────────────────────────────────────

    fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut globals = Vec::new();
        let mut funcs = Vec::new();

        while *self.peek() != TokenKind::Eof {
            // Standalone struct definition: struct Name { ... };
            if *self.peek() == TokenKind::KwStruct
                && matches!(self.peek_at(1), TokenKind::Ident(_))
                && *self.peek_at(2) == TokenKind::LBrace
            {
                self.parse_struct_def()?;
                continue;
            }

            // peek ahead to determine if function or global
            // Pattern: type name '(' => function
            let (ty, name) = self.parse_typed_decl()?;

            if *self.peek() == TokenKind::LParen {
                // function definition
                funcs.push(self.parse_func_rest(ty, name)?);
            } else {
                // global variable
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

    fn parse_func_rest(&mut self, ret_ty: Type, name: String) -> Result<FuncDef, ParseError> {
        self.expect(&TokenKind::LParen)?;
        let mut params = Vec::new();
        if *self.peek() != TokenKind::RParen {
            // Handle `(void)` — C syntax for explicitly-no-parameters
            if *self.peek() == TokenKind::KwVoid && *self.peek_at(1) == TokenKind::RParen {
                self.advance(); // consume `void`
            } else {
                loop {
                    let (ty, pname) = self.parse_typed_decl()?;
                    params.push((ty, pname));
                    if !self.eat(&TokenKind::Comma) { break; }
                }
            }
        }
        self.expect(&TokenKind::RParen)?;
        // Allow a semicolon here for forward declarations (skip the body)
        if self.eat(&TokenKind::Semicolon) {
            return Ok(FuncDef { ret_ty, name, params, body: vec![] });
        }
        self.expect(&TokenKind::LBrace)?;
        let body = self.parse_stmts_until_rbrace()?;
        Ok(FuncDef { ret_ty, name, params, body })
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
            _ if self.is_type_start() => self.parse_decl_stmt(),
            _ => {
                let e = self.parse_expr()?;
                self.expect(&TokenKind::Semicolon)?;
                Ok(Stmt::Expr(e))
            }
        }
    }

    fn is_type_start(&self) -> bool {
        matches!(self.peek(), TokenKind::KwInt | TokenKind::KwVoid | TokenKind::KwChar | TokenKind::KwStruct)
    }

    fn parse_decl_stmt(&mut self) -> Result<Stmt, ParseError> {
        let (ty, name) = self.parse_typed_decl()?;
        let init = if self.eat(&TokenKind::Assign) {
            Some(self.parse_assign_expr()?)
        } else {
            None
        };
        self.expect(&TokenKind::Semicolon)?;
        Ok(Stmt::Decl(ty, name, init))
    }

    // ── Expressions ───────────────────────────────────────────────────────

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_assign_expr()
    }

    fn parse_assign_expr(&mut self) -> Result<Expr, ParseError> {
        let lhs = self.parse_or_expr()?;
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
        Ok(lhs)
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
        let mut lhs = self.parse_bitand_expr()?;
        while self.eat(&TokenKind::Pipe) {
            let rhs = self.parse_bitand_expr()?;
            lhs = Expr::BinOp(BinOp::BitOr, Box::new(lhs), Box::new(rhs));
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
        let mut lhs = self.parse_add_expr()?;
        loop {
            if self.eat(&TokenKind::Lt) {
                lhs = Expr::BinOp(BinOp::Lt, Box::new(lhs), Box::new(self.parse_add_expr()?));
            } else if self.eat(&TokenKind::Le) {
                lhs = Expr::BinOp(BinOp::Le, Box::new(lhs), Box::new(self.parse_add_expr()?));
            } else if self.eat(&TokenKind::Gt) {
                lhs = Expr::BinOp(BinOp::Gt, Box::new(lhs), Box::new(self.parse_add_expr()?));
            } else if self.eat(&TokenKind::Ge) {
                lhs = Expr::BinOp(BinOp::Ge, Box::new(lhs), Box::new(self.parse_add_expr()?));
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
                // post-increment: treat as (e += 1) - 1 ... simplified to AddAssign
                // For simplicity we treat i++ as ++i (side effect only, not returning old value)
                e = Expr::BinOp(BinOp::AddAssign, Box::new(e), Box::new(Expr::Num(1)));
            } else if self.eat(&TokenKind::MinusMinus) {
                e = Expr::BinOp(BinOp::SubAssign, Box::new(e), Box::new(Expr::Num(1)));
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let pos = self.cur_pos();
        match self.peek().clone() {
            TokenKind::Number(n) => { self.advance(); Ok(Expr::Num(n)) }
            TokenKind::CharLit(c) => { self.advance(); Ok(Expr::Num(c as i32)) }
            TokenKind::StringLit(s) => { self.advance(); Ok(Expr::StringLit(s)) }
            TokenKind::Ident(name) => {
                self.advance();
                // function call?
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
                self.advance();
                let e = self.parse_expr()?;
                self.expect(&TokenKind::RParen)?;
                Ok(e)
            }
            got => Err(ParseError::new(pos, format!("unexpected token in expression: {:?}", got))),
        }
    }
}

pub fn parse(tokens: Vec<Token>) -> Result<Program, ParseError> {
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}
