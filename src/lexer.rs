use thiserror::Error;

#[derive(Debug, Error, Clone)]
#[error("lex error at position {pos}: {msg}")]
pub struct LexError {
    pub pos: usize,
    pub msg: String,
}

impl LexError {
    fn new(pos: usize, msg: impl Into<String>) -> Self {
        Self { pos, msg: msg.into() }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    Number(i32),
    CharLit(i16),
    StringLit(String),
    Ident(String),
    // Keywords
    KwInt,
    KwChar,
    KwVoid,
    KwReturn,
    KwIf,
    KwElse,
    KwWhile,
    KwFor,
    KwSizeof,
    KwStruct,
    // Punctuation
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Semicolon,
    Comma,
    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Amp,
    Pipe,
    Bang,
    Tilde,
    Assign,
    PlusAssign,
    MinusAssign,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    AmpAmp,
    PipePipe,
    PlusPlus,
    MinusMinus,
    Arrow,
    Dot,
    // End
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub pos: usize,
}

pub fn lex(source: &str) -> Result<Vec<Token>, LexError> {
    let bytes = source.as_bytes();
    let mut pos = 0;
    let mut tokens = Vec::new();

    while pos < bytes.len() {
        // Skip whitespace
        if bytes[pos].is_ascii_whitespace() {
            pos += 1;
            continue;
        }
        // Line comments
        if bytes[pos] == b'/' && pos + 1 < bytes.len() && bytes[pos + 1] == b'/' {
            while pos < bytes.len() && bytes[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }
        // Block comments
        if bytes[pos] == b'/' && pos + 1 < bytes.len() && bytes[pos + 1] == b'*' {
            pos += 2;
            loop {
                if pos + 1 >= bytes.len() {
                    return Err(LexError::new(pos, "unterminated block comment"));
                }
                if bytes[pos] == b'*' && bytes[pos + 1] == b'/' {
                    pos += 2;
                    break;
                }
                pos += 1;
            }
            continue;
        }
        let start = pos;
        let kind = match bytes[pos] {
            b'0'..=b'9' => {
                let s = start;
                while pos < bytes.len() && bytes[pos].is_ascii_digit() {
                    pos += 1;
                }
                let n: i32 = source[s..pos].parse().map_err(|_| {
                    LexError::new(s, "integer literal out of range")
                })?;
                tokens.push(Token { kind: TokenKind::Number(n), pos: start });
                continue;
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                while pos < bytes.len()
                    && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'_')
                {
                    pos += 1;
                }
                let word = &source[start..pos];
                let kind = match word {
                    "int"    => TokenKind::KwInt,
                    "char"   => TokenKind::KwChar,
                    "void"   => TokenKind::KwVoid,
                    "return" => TokenKind::KwReturn,
                    "if"     => TokenKind::KwIf,
                    "else"   => TokenKind::KwElse,
                    "while"  => TokenKind::KwWhile,
                    "for"    => TokenKind::KwFor,
                    "sizeof" => TokenKind::KwSizeof,
                    "struct" => TokenKind::KwStruct,
                    _        => TokenKind::Ident(word.to_string()),
                };
                tokens.push(Token { kind, pos: start });
                continue;
            }
            b'\'' => {
                pos += 1;
                let ch = lex_char_escape(bytes, &mut pos)
                    .map_err(|e| LexError::new(start, e))?;
                if pos >= bytes.len() || bytes[pos] != b'\'' {
                    return Err(LexError::new(start, "unterminated char literal"));
                }
                pos += 1;
                tokens.push(Token { kind: TokenKind::CharLit(ch), pos: start });
                continue;
            }
            b'"' => {
                pos += 1;
                let mut s = String::new();
                loop {
                    if pos >= bytes.len() {
                        return Err(LexError::new(start, "unterminated string literal"));
                    }
                    if bytes[pos] == b'"' { pos += 1; break; }
                    let ch = lex_char_escape(bytes, &mut pos)
                        .map_err(|e| LexError::new(start, e))?;
                    s.push(ch as u8 as char);
                }
                tokens.push(Token { kind: TokenKind::StringLit(s), pos: start });
                continue;
            }
            b'(' => { pos += 1; TokenKind::LParen }
            b')' => { pos += 1; TokenKind::RParen }
            b'{' => { pos += 1; TokenKind::LBrace }
            b'}' => { pos += 1; TokenKind::RBrace }
            b'[' => { pos += 1; TokenKind::LBracket }
            b']' => { pos += 1; TokenKind::RBracket }
            b';' => { pos += 1; TokenKind::Semicolon }
            b',' => { pos += 1; TokenKind::Comma }
            b'.' => { pos += 1; TokenKind::Dot }
            b'~' => { pos += 1; TokenKind::Tilde }
            b'+' => {
                pos += 1;
                if pos < bytes.len() && bytes[pos] == b'=' { pos += 1; TokenKind::PlusAssign }
                else if pos < bytes.len() && bytes[pos] == b'+' { pos += 1; TokenKind::PlusPlus }
                else { TokenKind::Plus }
            }
            b'-' => {
                pos += 1;
                if pos < bytes.len() && bytes[pos] == b'=' { pos += 1; TokenKind::MinusAssign }
                else if pos < bytes.len() && bytes[pos] == b'-' { pos += 1; TokenKind::MinusMinus }
                else if pos < bytes.len() && bytes[pos] == b'>' { pos += 1; TokenKind::Arrow }
                else { TokenKind::Minus }
            }
            b'*' => { pos += 1; TokenKind::Star }
            b'/' => { pos += 1; TokenKind::Slash }
            b'%' => { pos += 1; TokenKind::Percent }
            b'&' => {
                pos += 1;
                if pos < bytes.len() && bytes[pos] == b'&' { pos += 1; TokenKind::AmpAmp }
                else { TokenKind::Amp }
            }
            b'|' => {
                pos += 1;
                if pos < bytes.len() && bytes[pos] == b'|' { pos += 1; TokenKind::PipePipe }
                else { TokenKind::Pipe }
            }
            b'!' => {
                pos += 1;
                if pos < bytes.len() && bytes[pos] == b'=' { pos += 1; TokenKind::Ne }
                else { TokenKind::Bang }
            }
            b'=' => {
                pos += 1;
                if pos < bytes.len() && bytes[pos] == b'=' { pos += 1; TokenKind::Eq }
                else { TokenKind::Assign }
            }
            b'<' => {
                pos += 1;
                if pos < bytes.len() && bytes[pos] == b'=' { pos += 1; TokenKind::Le }
                else { TokenKind::Lt }
            }
            b'>' => {
                pos += 1;
                if pos < bytes.len() && bytes[pos] == b'=' { pos += 1; TokenKind::Ge }
                else { TokenKind::Gt }
            }
            c => return Err(LexError::new(pos, format!("unexpected character '{}'", c as char))),
        };
        tokens.push(Token { kind, pos: start });
    }
    tokens.push(Token { kind: TokenKind::Eof, pos });
    Ok(tokens)
}

/// Parse one character value from `bytes[*pos]`, advancing `*pos`.
/// Handles backslash escape sequences. Returns the character as i16.
/// On error returns a string message (caller wraps in LexError).
fn lex_char_escape(bytes: &[u8], pos: &mut usize) -> Result<i16, String> {
    if *pos >= bytes.len() {
        return Err("unexpected end of input in char/string literal".into());
    }
    if bytes[*pos] != b'\\' {
        let c = bytes[*pos] as i16;
        *pos += 1;
        return Ok(c);
    }
    // escape sequence
    *pos += 1;
    if *pos >= bytes.len() {
        return Err("unexpected end of input after '\\'".into());
    }
    let esc = bytes[*pos];
    *pos += 1;
    Ok(match esc {
        b'n'  => 10,
        b't'  => 9,
        b'r'  => 13,
        b'0'  => 0,
        b'\\'  => 92,
        b'\'' => 39,
        b'"'  => 34,
        b'a'  => 7,
        b'b'  => 8,
        b'f'  => 12,
        b'v'  => 11,
        c => return Err(format!("unknown escape sequence '\\{}'", c as char)),
    })
}
