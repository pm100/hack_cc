use thiserror::Error;

#[derive(Debug, Error, Clone)]
#[error("lex error at {line}:{col}: {msg}")]
pub struct LexError {
    pub line: u32,
    pub col: u32,
    pub msg: String,
}

impl LexError {
    fn new(line: u32, col: u32, msg: impl Into<String>) -> Self {
        Self { line, col, msg: msg.into() }
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
    KwDo,
    KwBreak,
    KwContinue,
    KwSwitch,
    KwCase,
    KwDefault,
    KwSizeof,
    KwStruct,
    KwTypedef,
    KwEnum,
    KwUnsigned,
    KwSigned,
    KwLong,
    KwShort,
    KwConst,
    KwExtern,
    KwStatic,
    KwGoto,
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
    Caret,
    Question,
    Colon,
    Assign,
    PlusAssign,
    MinusAssign,
    StarAssign,
    SlashAssign,
    PercentAssign,
    AmpAssign,
    PipeAssign,
    CaretAssign,
    LtLt,
    GtGt,
    LtLtAssign,
    GtGtAssign,
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
    DotDotDot,
    // End
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: u32,
    pub col: u32,
}

pub fn lex(source: &str) -> Result<Vec<Token>, LexError> {
    let bytes = source.as_bytes();
    let mut pos = 0;
    let mut tokens = Vec::new();

    // Precompute line-start byte offsets for O(log n) pos → (line, col).
    let line_starts: Vec<usize> = std::iter::once(0)
        .chain(bytes.iter().enumerate().filter_map(|(i, &b)| {
            if b == b'\n' { Some(i + 1) } else { None }
        }))
        .collect();

    let pos_to_lc = |p: usize| -> (u32, u32) {
        let line = line_starts.partition_point(|&s| s <= p) as u32;
        let col  = (p - line_starts[(line - 1) as usize] + 1) as u32;
        (line, col)
    };

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
            let (bl, bc) = pos_to_lc(pos);
            pos += 2;
            loop {
                if pos + 1 >= bytes.len() {
                    return Err(LexError::new(bl, bc, "unterminated block comment"));
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
        let (sl, sc) = pos_to_lc(start);
        let kind = match bytes[pos] {
            b'0'..=b'9' => {
                // Hex literal: 0x… or 0X…
                if bytes[pos] == b'0'
                    && pos + 1 < bytes.len()
                    && (bytes[pos + 1] == b'x' || bytes[pos + 1] == b'X')
                {
                    pos += 2;
                    let hex_start = pos;
                    while pos < bytes.len() && bytes[pos].is_ascii_hexdigit() {
                        pos += 1;
                    }
                    if hex_start == pos {
                        return Err(LexError::new(sl, sc, "expected hex digits after '0x'"));
                    }
                    let hex_end = pos;
                    // Strip integer suffixes (u, U, l, L) — e.g. 0xFFul
                    while pos < bytes.len() && matches!(bytes[pos], b'u' | b'U' | b'l' | b'L') {
                        pos += 1;
                    }
                    let n = i32::from_str_radix(&source[hex_start..hex_end], 16).map_err(|_| {
                        LexError::new(sl, sc, "hex literal out of range")
                    })?;
                    tokens.push(Token { kind: TokenKind::Number(n), line: sl, col: sc });
                    continue;
                }
                // Decimal literal
                while pos < bytes.len() && bytes[pos].is_ascii_digit() {
                    pos += 1;
                }
                let digit_end = pos;
                // Strip integer suffixes (u, U, l, L) — e.g. 100l, 42UL
                while pos < bytes.len() && matches!(bytes[pos], b'u' | b'U' | b'l' | b'L') {
                    pos += 1;
                }
                let n: i32 = source[start..digit_end].parse().map_err(|_| {
                    LexError::new(sl, sc, "integer literal out of range")
                })?;
                tokens.push(Token { kind: TokenKind::Number(n), line: sl, col: sc });
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
                    "sizeof"    => TokenKind::KwSizeof,
                    "struct"    => TokenKind::KwStruct,
                    "do"        => TokenKind::KwDo,
                    "break"     => TokenKind::KwBreak,
                    "continue"  => TokenKind::KwContinue,
                    "switch"    => TokenKind::KwSwitch,
                    "case"      => TokenKind::KwCase,
                    "default"   => TokenKind::KwDefault,
                    "typedef"   => TokenKind::KwTypedef,
                    "enum"      => TokenKind::KwEnum,
                    "unsigned"  => TokenKind::KwUnsigned,
                    "long"      => TokenKind::KwLong,
                    "short"     => TokenKind::KwShort,
                    "const"     => TokenKind::KwConst,
                    "extern"    => TokenKind::KwExtern,
                    "static"    => TokenKind::KwStatic,
                    "signed"    => TokenKind::KwSigned,
                    "goto"      => TokenKind::KwGoto,
                    _           => TokenKind::Ident(word.to_string()),
                };
                tokens.push(Token { kind, line: sl, col: sc });
                continue;
            }
            b'\'' => {
                pos += 1;
                let ch = lex_char_escape(bytes, &mut pos)
                    .map_err(|e| LexError::new(sl, sc, e))?;
                if pos >= bytes.len() || bytes[pos] != b'\'' {
                    return Err(LexError::new(sl, sc, "unterminated char literal"));
                }
                pos += 1;
                tokens.push(Token { kind: TokenKind::CharLit(ch), line: sl, col: sc });
                continue;
            }
            b'"' => {
                pos += 1;
                let mut s = String::new();
                loop {
                    if pos >= bytes.len() {
                        return Err(LexError::new(sl, sc, "unterminated string literal"));
                    }
                    if bytes[pos] == b'"' { pos += 1; break; }
                    let ch = lex_char_escape(bytes, &mut pos)
                        .map_err(|e| LexError::new(sl, sc, e))?;
                    s.push(ch as u8 as char);
                }
                tokens.push(Token { kind: TokenKind::StringLit(s), line: sl, col: sc });
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
            b'.' => {
                if pos + 2 < bytes.len() && bytes[pos+1] == b'.' && bytes[pos+2] == b'.' {
                    pos += 3; TokenKind::DotDotDot
                } else {
                    pos += 1; TokenKind::Dot
                }
            }
            b'~' => { pos += 1; TokenKind::Tilde }
            b'^' => {
                pos += 1;
                if pos < bytes.len() && bytes[pos] == b'=' { pos += 1; TokenKind::CaretAssign }
                else { TokenKind::Caret }
            }
            b'?' => { pos += 1; TokenKind::Question }
            b':' => { pos += 1; TokenKind::Colon }
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
            b'*' => {
                pos += 1;
                if pos < bytes.len() && bytes[pos] == b'=' { pos += 1; TokenKind::StarAssign }
                else { TokenKind::Star }
            }
            b'/' => {
                pos += 1;
                if pos < bytes.len() && bytes[pos] == b'=' { pos += 1; TokenKind::SlashAssign }
                else { TokenKind::Slash }
            }
            b'%' => {
                pos += 1;
                if pos < bytes.len() && bytes[pos] == b'=' { pos += 1; TokenKind::PercentAssign }
                else { TokenKind::Percent }
            }
            b'&' => {
                pos += 1;
                if pos < bytes.len() && bytes[pos] == b'&' { pos += 1; TokenKind::AmpAmp }
                else if pos < bytes.len() && bytes[pos] == b'=' { pos += 1; TokenKind::AmpAssign }
                else { TokenKind::Amp }
            }
            b'|' => {
                pos += 1;
                if pos < bytes.len() && bytes[pos] == b'|' { pos += 1; TokenKind::PipePipe }
                else if pos < bytes.len() && bytes[pos] == b'=' { pos += 1; TokenKind::PipeAssign }
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
                if pos < bytes.len() && bytes[pos] == b'<' {
                    pos += 1;
                    if pos < bytes.len() && bytes[pos] == b'=' { pos += 1; TokenKind::LtLtAssign }
                    else { TokenKind::LtLt }
                } else if pos < bytes.len() && bytes[pos] == b'=' { pos += 1; TokenKind::Le }
                else { TokenKind::Lt }
            }
            b'>' => {
                pos += 1;
                if pos < bytes.len() && bytes[pos] == b'>' {
                    pos += 1;
                    if pos < bytes.len() && bytes[pos] == b'=' { pos += 1; TokenKind::GtGtAssign }
                    else { TokenKind::GtGt }
                } else if pos < bytes.len() && bytes[pos] == b'=' { pos += 1; TokenKind::Ge }
                else { TokenKind::Gt }
            }
            c => return Err(LexError::new(sl, sc, format!("unexpected character '{}'", c as char))),
        };
        tokens.push(Token { kind, line: sl, col: sc });
    }
    let (el, ec) = pos_to_lc(pos);
    tokens.push(Token { kind: TokenKind::Eof, line: el, col: ec });
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
