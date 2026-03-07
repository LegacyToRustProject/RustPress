//! Plural form expression parser and evaluator.
//!
//! WordPress `.mo` files include a `Plural-Forms` header like:
//! ```text
//! Plural-Forms: nplurals=2; plural=(n != 1);
//! ```
//!
//! This module parses the `plural=EXPR` part and evaluates it for a given `n`.
//! The expression language is a subset of C ternary expressions.

/// A parsed plural form expression that can be evaluated for any `n`.
#[derive(Debug, Clone)]
pub struct PluralExpression {
    ast: Expr,
    pub nplurals: usize,
}

/// AST node for plural expressions.
#[derive(Debug, Clone)]
enum Expr {
    /// The variable `n`.
    N,
    /// A numeric literal.
    Literal(u64),
    /// Binary operation.
    BinOp {
        op: BinOpKind,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// Ternary: condition ? true_expr : false_expr
    Ternary {
        condition: Box<Expr>,
        true_expr: Box<Expr>,
        false_expr: Box<Expr>,
    },
    /// Logical NOT: !expr
    Not(Box<Expr>),
}

#[derive(Debug, Clone, Copy)]
enum BinOpKind {
    // Arithmetic
    Mod, // %
    // Comparison
    Eq,  // ==
    Ne,  // !=
    Lt,  // <
    Gt,  // >
    Le,  // <=
    Ge,  // >=
    // Logical
    And, // &&
    Or,  // ||
}

/// Tokenizer for plural expressions.
#[derive(Debug, Clone)]
struct Token {
    kind: TokenKind,
}

#[derive(Debug, Clone, PartialEq)]
enum TokenKind {
    N,
    Number(u64),
    Percent,   // %
    EqEq,     // ==
    BangEq,   // !=
    Lt,        // <
    Gt,        // >
    LtEq,     // <=
    GtEq,     // >=
    AmpAmp,   // &&
    PipePipe,  // ||
    Bang,      // !
    Question,  // ?
    Colon,     // :
    LParen,    // (
    RParen,    // )
    Semicolon, // ;
}

fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b' ' | b'\t' | b'\n' | b'\r' => {
                i += 1;
            }
            b'n' => {
                // Could be 'n' variable or 'nplurals' etc. - we only care about standalone 'n'
                if i + 1 < bytes.len() && bytes[i + 1].is_ascii_alphanumeric() {
                    // Skip non-'n' identifiers
                    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                        i += 1;
                    }
                } else {
                    tokens.push(Token { kind: TokenKind::N });
                    i += 1;
                }
            }
            b'0'..=b'9' => {
                let start = i;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                let num: u64 = input[start..i].parse().unwrap_or(0);
                tokens.push(Token {
                    kind: TokenKind::Number(num),
                });
            }
            b'%' => {
                tokens.push(Token { kind: TokenKind::Percent });
                i += 1;
            }
            b'=' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    tokens.push(Token { kind: TokenKind::EqEq });
                    i += 2;
                } else {
                    // Skip lone '=' (assignment in nplurals=N)
                    i += 1;
                }
            }
            b'!' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    tokens.push(Token { kind: TokenKind::BangEq });
                    i += 2;
                } else {
                    tokens.push(Token { kind: TokenKind::Bang });
                    i += 1;
                }
            }
            b'<' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    tokens.push(Token { kind: TokenKind::LtEq });
                    i += 2;
                } else {
                    tokens.push(Token { kind: TokenKind::Lt });
                    i += 1;
                }
            }
            b'>' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    tokens.push(Token { kind: TokenKind::GtEq });
                    i += 2;
                } else {
                    tokens.push(Token { kind: TokenKind::Gt });
                    i += 1;
                }
            }
            b'&' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'&' {
                    tokens.push(Token { kind: TokenKind::AmpAmp });
                    i += 2;
                } else {
                    i += 1;
                }
            }
            b'|' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'|' {
                    tokens.push(Token { kind: TokenKind::PipePipe });
                    i += 2;
                } else {
                    i += 1;
                }
            }
            b'?' => {
                tokens.push(Token { kind: TokenKind::Question });
                i += 1;
            }
            b':' => {
                tokens.push(Token { kind: TokenKind::Colon });
                i += 1;
            }
            b'(' => {
                tokens.push(Token { kind: TokenKind::LParen });
                i += 1;
            }
            b')' => {
                tokens.push(Token { kind: TokenKind::RParen });
                i += 1;
            }
            b';' => {
                tokens.push(Token { kind: TokenKind::Semicolon });
                i += 1;
            }
            _ => {
                // Skip unknown characters
                i += 1;
            }
        }
    }

    tokens
}

/// Recursive descent parser for plural expressions.
///
/// Grammar (precedence low to high):
/// ```text
/// expr       = or_expr ('?' expr ':' expr)?
/// or_expr    = and_expr ('||' and_expr)*
/// and_expr   = eq_expr  ('&&' eq_expr)*
/// eq_expr    = rel_expr (('==' | '!=') rel_expr)*
/// rel_expr   = mod_expr (('<' | '>' | '<=' | '>=') mod_expr)*
/// mod_expr   = unary    ('%' unary)*
/// unary      = '!' unary | primary
/// primary    = 'n' | NUMBER | '(' expr ')'
/// ```
struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&TokenKind> {
        self.tokens.get(self.pos).map(|t| &t.kind)
    }

    fn advance(&mut self) -> Option<TokenKind> {
        if self.pos < self.tokens.len() {
            let kind = self.tokens[self.pos].kind.clone();
            self.pos += 1;
            Some(kind)
        } else {
            None
        }
    }

    fn expect(&mut self, expected: &TokenKind) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn parse_expr(&mut self) -> Expr {
        let expr = self.parse_or();

        // Ternary
        if self.expect(&TokenKind::Question) {
            let true_expr = self.parse_expr();
            self.expect(&TokenKind::Colon);
            let false_expr = self.parse_expr();
            return Expr::Ternary {
                condition: Box::new(expr),
                true_expr: Box::new(true_expr),
                false_expr: Box::new(false_expr),
            };
        }

        expr
    }

    fn parse_or(&mut self) -> Expr {
        let mut left = self.parse_and();
        while self.peek() == Some(&TokenKind::PipePipe) {
            self.advance();
            let right = self.parse_and();
            left = Expr::BinOp {
                op: BinOpKind::Or,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        left
    }

    fn parse_and(&mut self) -> Expr {
        let mut left = self.parse_equality();
        while self.peek() == Some(&TokenKind::AmpAmp) {
            self.advance();
            let right = self.parse_equality();
            left = Expr::BinOp {
                op: BinOpKind::And,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        left
    }

    fn parse_equality(&mut self) -> Expr {
        let mut left = self.parse_relational();
        loop {
            match self.peek() {
                Some(&TokenKind::EqEq) => {
                    self.advance();
                    let right = self.parse_relational();
                    left = Expr::BinOp {
                        op: BinOpKind::Eq,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                Some(&TokenKind::BangEq) => {
                    self.advance();
                    let right = self.parse_relational();
                    left = Expr::BinOp {
                        op: BinOpKind::Ne,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                _ => break,
            }
        }
        left
    }

    fn parse_relational(&mut self) -> Expr {
        let mut left = self.parse_mod();
        loop {
            match self.peek() {
                Some(&TokenKind::Lt) => {
                    self.advance();
                    let right = self.parse_mod();
                    left = Expr::BinOp {
                        op: BinOpKind::Lt,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                Some(&TokenKind::Gt) => {
                    self.advance();
                    let right = self.parse_mod();
                    left = Expr::BinOp {
                        op: BinOpKind::Gt,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                Some(&TokenKind::LtEq) => {
                    self.advance();
                    let right = self.parse_mod();
                    left = Expr::BinOp {
                        op: BinOpKind::Le,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                Some(&TokenKind::GtEq) => {
                    self.advance();
                    let right = self.parse_mod();
                    left = Expr::BinOp {
                        op: BinOpKind::Ge,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                }
                _ => break,
            }
        }
        left
    }

    fn parse_mod(&mut self) -> Expr {
        let mut left = self.parse_unary();
        while self.peek() == Some(&TokenKind::Percent) {
            self.advance();
            let right = self.parse_unary();
            left = Expr::BinOp {
                op: BinOpKind::Mod,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        left
    }

    fn parse_unary(&mut self) -> Expr {
        if self.peek() == Some(&TokenKind::Bang) {
            self.advance();
            let expr = self.parse_unary();
            return Expr::Not(Box::new(expr));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Expr {
        match self.peek().cloned() {
            Some(TokenKind::N) => {
                self.advance();
                Expr::N
            }
            Some(TokenKind::Number(n)) => {
                self.advance();
                Expr::Literal(n)
            }
            Some(TokenKind::LParen) => {
                self.advance();
                let expr = self.parse_expr();
                self.expect(&TokenKind::RParen);
                expr
            }
            _ => {
                // Fallback: return 0
                Expr::Literal(0)
            }
        }
    }
}

fn eval(expr: &Expr, n: u64) -> u64 {
    match expr {
        Expr::N => n,
        Expr::Literal(v) => *v,
        Expr::BinOp { op, left, right } => {
            let l = eval(left, n);
            let r = eval(right, n);
            match op {
                BinOpKind::Mod => {
                    if r == 0 {
                        0
                    } else {
                        l % r
                    }
                }
                BinOpKind::Eq => (l == r) as u64,
                BinOpKind::Ne => (l != r) as u64,
                BinOpKind::Lt => (l < r) as u64,
                BinOpKind::Gt => (l > r) as u64,
                BinOpKind::Le => (l <= r) as u64,
                BinOpKind::Ge => (l >= r) as u64,
                BinOpKind::And => ((l != 0) && (r != 0)) as u64,
                BinOpKind::Or => ((l != 0) || (r != 0)) as u64,
            }
        }
        Expr::Ternary {
            condition,
            true_expr,
            false_expr,
        } => {
            if eval(condition, n) != 0 {
                eval(true_expr, n)
            } else {
                eval(false_expr, n)
            }
        }
        Expr::Not(inner) => (eval(inner, n) == 0) as u64,
    }
}

impl PluralExpression {
    /// Evaluate the plural expression for a given number `n`.
    /// Returns the index into the plural forms array.
    pub fn evaluate(&self, n: u64) -> usize {
        let result = eval(&self.ast, n) as usize;
        // Clamp to valid range
        if result >= self.nplurals {
            0
        } else {
            result
        }
    }
}

/// Parse a `Plural-Forms` header value into a `PluralExpression`.
///
/// Input format: `nplurals=N; plural=EXPR;`
///
/// # Examples
/// ```
/// use rustpress_i18n::plural::parse_plural_expression;
///
/// let expr = parse_plural_expression("nplurals=2; plural=(n != 1);");
/// assert_eq!(expr.evaluate(0), 1);
/// assert_eq!(expr.evaluate(1), 0);
/// assert_eq!(expr.evaluate(5), 1);
/// ```
pub fn parse_plural_expression(header: &str) -> PluralExpression {
    let mut nplurals = 2usize;
    let mut plural_expr_str = "n != 1";

    for part in header.split(';') {
        let part = part.trim();
        if let Some(val) = part.strip_prefix("nplurals=").or_else(|| part.strip_prefix("nplurals =")) {
            nplurals = val.trim().parse().unwrap_or(2);
        } else if let Some(val) = part.strip_prefix("plural=").or_else(|| part.strip_prefix("plural =")) {
            plural_expr_str = val.trim();
        }
    }

    let tokens = tokenize(plural_expr_str);
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_expr();

    PluralExpression { ast, nplurals }
}

/// Get the default plural expression for a locale.
///
/// Returns a reasonable default based on common locale plural rules.
pub fn default_plural_expression(locale: &str) -> PluralExpression {
    let lang = locale.split('_').next().unwrap_or(locale);
    match lang {
        // Languages with no plural forms (always index 0)
        "ja" | "ko" | "zh" | "vi" | "th" | "lo" | "id" | "ms" | "ka" | "tr" => {
            parse_plural_expression("nplurals=1; plural=0;")
        }
        // French-style: plural for n > 1
        "fr" | "pt_BR" => parse_plural_expression("nplurals=2; plural=(n > 1);"),
        // Russian-style
        "ru" | "uk" | "sr" | "hr" | "bs" => parse_plural_expression(
            "nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);",
        ),
        // Arabic
        "ar" => parse_plural_expression(
            "nplurals=6; plural=(n==0 ? 0 : n==1 ? 1 : n==2 ? 2 : n%100>=3 && n%100<=10 ? 3 : n%100>=11 ? 4 : 5);",
        ),
        // Polish
        "pl" => parse_plural_expression(
            "nplurals=3; plural=(n==1 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);",
        ),
        // Default: English-style (n != 1)
        _ => parse_plural_expression("nplurals=2; plural=(n != 1);"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_english_plural() {
        let expr = parse_plural_expression("nplurals=2; plural=(n != 1);");
        assert_eq!(expr.nplurals, 2);
        assert_eq!(expr.evaluate(0), 1); // "0 items" (plural)
        assert_eq!(expr.evaluate(1), 0); // "1 item" (singular)
        assert_eq!(expr.evaluate(2), 1);
        assert_eq!(expr.evaluate(100), 1);
    }

    #[test]
    fn test_french_plural() {
        let expr = parse_plural_expression("nplurals=2; plural=(n > 1);");
        assert_eq!(expr.evaluate(0), 0); // singular in French
        assert_eq!(expr.evaluate(1), 0);
        assert_eq!(expr.evaluate(2), 1);
        assert_eq!(expr.evaluate(100), 1);
    }

    #[test]
    fn test_japanese_plural() {
        let expr = parse_plural_expression("nplurals=1; plural=0;");
        assert_eq!(expr.nplurals, 1);
        assert_eq!(expr.evaluate(0), 0);
        assert_eq!(expr.evaluate(1), 0);
        assert_eq!(expr.evaluate(1000), 0);
    }

    #[test]
    fn test_russian_plural() {
        let expr = parse_plural_expression(
            "nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);",
        );
        assert_eq!(expr.nplurals, 3);
        assert_eq!(expr.evaluate(1), 0);   // 1 яблоко
        assert_eq!(expr.evaluate(2), 1);   // 2 яблока
        assert_eq!(expr.evaluate(5), 2);   // 5 яблок
        assert_eq!(expr.evaluate(11), 2);  // 11 яблок
        assert_eq!(expr.evaluate(21), 0);  // 21 яблоко
        assert_eq!(expr.evaluate(22), 1);  // 22 яблока
        assert_eq!(expr.evaluate(25), 2);  // 25 яблок
        assert_eq!(expr.evaluate(111), 2); // 111 яблок
        assert_eq!(expr.evaluate(112), 2); // 112 яблок
    }

    #[test]
    fn test_modulo_operator() {
        let expr = parse_plural_expression("nplurals=2; plural=(n % 10 == 1);");
        // (n%10==1) evaluates to 1 (true) for n=1, which is plural index 1
        assert_eq!(expr.evaluate(1), 1);
        assert_eq!(expr.evaluate(11), 1);
        assert_eq!(expr.evaluate(21), 1);
        assert_eq!(expr.evaluate(2), 0);
        assert_eq!(expr.evaluate(5), 0);
    }

    #[test]
    fn test_not_operator() {
        let expr = parse_plural_expression("nplurals=2; plural=!(n == 1);");
        assert_eq!(expr.evaluate(0), 1);
        assert_eq!(expr.evaluate(1), 0);
        assert_eq!(expr.evaluate(2), 1);
    }

    #[test]
    fn test_default_plural_expressions() {
        let ja = default_plural_expression("ja");
        assert_eq!(ja.nplurals, 1);
        assert_eq!(ja.evaluate(0), 0);
        assert_eq!(ja.evaluate(42), 0);

        let en = default_plural_expression("en_US");
        assert_eq!(en.nplurals, 2);
        assert_eq!(en.evaluate(1), 0);
        assert_eq!(en.evaluate(2), 1);
    }
}
