use crate::error::Error;
use crate::lexer::{StringPart, Token, lex};

pub type Block = Vec<(usize, Stmt)>;

#[derive(Debug)]
pub enum Stmt {
    Let(String, Expr),
    Assign(String, Expr),
    Print(Expr),
    If(Expr, Block, Option<Block>),
    While(Expr, Block),
    Expr(Expr),
}

#[derive(Debug)]
pub enum Expr {
    Number(f64),
    Bool(bool),
    Str(String),
    Template(Vec<Expr>),
    Variable(String),
    Negate(Box<Expr>),
    Not(Box<Expr>),
    Binary(Box<Expr>, Op, Box<Expr>),
}

#[derive(Debug, Clone, Copy)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}

pub struct Parser {
    tokens: Vec<(Token, usize)>,
    pos: usize,
    line: usize,
}

impl Parser {
    pub fn new(tokens: Vec<(Token, usize)>) -> Self {
        Parser {
            tokens,
            pos: 0,
            line: 1,
        }
    }

    pub fn parse_program(&mut self) -> Result<Block, Error> {
        self.statements(None)
    }

    fn statements(&mut self, closing: Option<Token>) -> Result<Block, Error> {
        let mut statements = Vec::new();

        loop {
            while self.peek() == Some(&Token::Newline) {
                self.pos += 1;
            }

            match (self.peek(), &closing) {
                (None, None) => break,
                (None, Some(token)) => {
                    return Err(self.error(format!("expected {token:?}, found end of input")));
                }
                (Some(next), Some(token)) if next == token => {
                    self.pos += 1;
                    break;
                }
                _ => {}
            }

            let line = self.tokens[self.pos].1;
            let stmt = self.statement()?;
            statements.push((line, stmt));

            match self.peek() {
                None | Some(Token::Newline) => {}
                Some(next) if Some(next) == closing.as_ref() => {}
                Some(next) => return Err(self.error(format!("unexpected {next:?}"))),
            }
        }

        Ok(statements)
    }

    fn statement(&mut self) -> Result<Stmt, Error> {
        match self.peek() {
            Some(Token::Let) => {
                self.pos += 1;
                let name = self.identifier()?;
                self.expect(Token::Equals)?;
                Ok(Stmt::Let(name, self.expression()?))
            }
            Some(Token::Print) => {
                self.pos += 1;
                Ok(Stmt::Print(self.expression()?))
            }
            Some(Token::If) => {
                self.pos += 1;
                self.if_statement()
            }
            Some(Token::While) => {
                self.pos += 1;
                let condition = self.expression()?;
                let body = self.block()?;
                Ok(Stmt::While(condition, body))
            }
            Some(Token::Ident(_)) if self.peek_next() == Some(&Token::Equals) => {
                let name = self.identifier()?;
                self.pos += 1;
                Ok(Stmt::Assign(name, self.expression()?))
            }
            _ => Ok(Stmt::Expr(self.expression()?)),
        }
    }

    fn if_statement(&mut self) -> Result<Stmt, Error> {
        let condition = self.expression()?;
        let then_block = self.block()?;

        let else_block = if self.peek() == Some(&Token::Else) {
            self.pos += 1;
            if self.peek() == Some(&Token::If) {
                let line = self.tokens[self.pos].1;
                self.pos += 1;
                Some(vec![(line, self.if_statement()?)])
            } else {
                Some(self.block()?)
            }
        } else {
            None
        };

        Ok(Stmt::If(condition, then_block, else_block))
    }

    fn block(&mut self) -> Result<Block, Error> {
        self.expect(Token::LeftBrace)?;
        self.statements(Some(Token::RightBrace))
    }

    fn expression(&mut self) -> Result<Expr, Error> {
        self.or()
    }

    fn or(&mut self) -> Result<Expr, Error> {
        let mut left = self.and()?;
        while self.peek() == Some(&Token::OrOr) {
            self.pos += 1;
            let right = self.and()?;
            left = Expr::Binary(Box::new(left), Op::Or, Box::new(right));
        }
        Ok(left)
    }

    fn and(&mut self) -> Result<Expr, Error> {
        let mut left = self.equality()?;
        while self.peek() == Some(&Token::AndAnd) {
            self.pos += 1;
            let right = self.equality()?;
            left = Expr::Binary(Box::new(left), Op::And, Box::new(right));
        }
        Ok(left)
    }

    fn equality(&mut self) -> Result<Expr, Error> {
        let mut left = self.comparison()?;
        loop {
            let op = match self.peek() {
                Some(Token::EqualEqual) => Op::Equal,
                Some(Token::BangEqual) => Op::NotEqual,
                _ => break,
            };
            self.pos += 1;
            let right = self.comparison()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn comparison(&mut self) -> Result<Expr, Error> {
        let mut left = self.term()?;
        loop {
            let op = match self.peek() {
                Some(Token::Less) => Op::Less,
                Some(Token::LessEqual) => Op::LessEqual,
                Some(Token::Greater) => Op::Greater,
                Some(Token::GreaterEqual) => Op::GreaterEqual,
                _ => break,
            };
            self.pos += 1;
            let right = self.term()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn term(&mut self) -> Result<Expr, Error> {
        let mut left = self.factor()?;
        loop {
            let op = match self.peek() {
                Some(Token::Plus) => Op::Add,
                Some(Token::Minus) => Op::Sub,
                _ => break,
            };
            self.pos += 1;
            let right = self.factor()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn factor(&mut self) -> Result<Expr, Error> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek() {
                Some(Token::Star) => Op::Mul,
                Some(Token::Slash) => Op::Div,
                Some(Token::Percent) => Op::Rem,
                _ => break,
            };
            self.pos += 1;
            let right = self.unary()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr, Error> {
        match self.peek() {
            Some(Token::Minus) => {
                self.pos += 1;
                Ok(Expr::Negate(Box::new(self.unary()?)))
            }
            Some(Token::Bang) => {
                self.pos += 1;
                Ok(Expr::Not(Box::new(self.unary()?)))
            }
            _ => self.primary(),
        }
    }

    fn primary(&mut self) -> Result<Expr, Error> {
        match self.advance() {
            Some(Token::Number(n)) => Ok(Expr::Number(n)),
            Some(Token::True) => Ok(Expr::Bool(true)),
            Some(Token::False) => Ok(Expr::Bool(false)),
            Some(Token::Str(text)) => Ok(Expr::Str(text)),
            Some(Token::Template(parts)) => self.template(parts),
            Some(Token::Ident(name)) => Ok(Expr::Variable(name)),
            Some(Token::LeftParen) => {
                let expr = self.expression()?;
                self.expect(Token::RightParen)?;
                Ok(expr)
            }
            Some(token) => Err(self.error(format!("unexpected {token:?}"))),
            None => Err(self.error("unexpected end of input".to_string())),
        }
    }

    fn template(&mut self, parts: Vec<StringPart>) -> Result<Expr, Error> {
        let mut pieces = Vec::new();
        for part in parts {
            match part {
                StringPart::Text(text) if text.is_empty() => {}
                StringPart::Text(text) => pieces.push(Expr::Str(text)),
                StringPart::Code(code) => pieces.push(self.embedded(&code)?),
            }
        }
        Ok(Expr::Template(pieces))
    }

    fn embedded(&self, code: &str) -> Result<Expr, Error> {
        let line = self.line;
        let tokens = lex(code)
            .map_err(|error| Error {
                line,
                message: error.message,
            })?
            .into_iter()
            .map(|(token, _)| (token, line))
            .collect();

        let mut parser = Parser::new(tokens);
        parser.line = line;
        let expr = parser.expression()?;
        match parser.peek() {
            None => Ok(expr),
            Some(token) => Err(self.error(format!("unexpected {token:?} inside ${{...}}"))),
        }
    }

    fn identifier(&mut self) -> Result<String, Error> {
        match self.advance() {
            Some(Token::Ident(name)) => Ok(name),
            Some(token) => Err(self.error(format!("expected a name, found {token:?}"))),
            None => Err(self.error("expected a name, found end of input".to_string())),
        }
    }

    fn expect(&mut self, expected: Token) -> Result<(), Error> {
        match self.advance() {
            Some(token) if token == expected => Ok(()),
            Some(token) => Err(self.error(format!("expected {expected:?}, found {token:?}"))),
            None => Err(self.error(format!("expected {expected:?}, found end of input"))),
        }
    }

    fn error(&self, message: String) -> Error {
        Error {
            line: self.line,
            message,
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|(token, _)| token)
    }

    fn peek_next(&self) -> Option<&Token> {
        self.tokens.get(self.pos + 1).map(|(token, _)| token)
    }

    fn advance(&mut self) -> Option<Token> {
        let (token, line) = self.tokens.get(self.pos).cloned()?;
        self.line = line;
        self.pos += 1;
        Some(token)
    }
}
