use crate::error::Error;
use crate::lexer::Token;

#[derive(Debug)]
pub enum Stmt {
    Let(String, Expr),
    Print(Expr),
    Expr(Expr),
}

#[derive(Debug)]
pub enum Expr {
    Number(f64),
    Variable(String),
    Negate(Box<Expr>),
    Binary(Box<Expr>, Op, Box<Expr>),
}

#[derive(Debug, Clone, Copy)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
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

    pub fn parse_program(&mut self) -> Result<Vec<(usize, Stmt)>, Error> {
        let mut program = Vec::new();

        loop {
            while self.peek() == Some(&Token::Newline) {
                self.pos += 1;
            }
            let line = match self.tokens.get(self.pos) {
                Some((_, line)) => *line,
                None => break,
            };

            let stmt = self.statement()?;
            program.push((line, stmt));

            match self.advance() {
                None | Some(Token::Newline) => {}
                Some(token) => return Err(self.error(format!("unexpected {token:?}"))),
            }
        }

        Ok(program)
    }

    fn statement(&mut self) -> Result<Stmt, Error> {
        match self.peek() {
            Some(Token::Let) => {
                self.pos += 1;
                let name = match self.advance() {
                    Some(Token::Ident(name)) => name,
                    Some(token) => return Err(self.error(format!("unexpected {token:?}"))),
                    None => return Err(self.error("unexpected end of input".to_string())),
                };
                self.expect(Token::Equals)?;
                Ok(Stmt::Let(name, self.expression()?))
            }
            Some(Token::Print) => {
                self.pos += 1;
                Ok(Stmt::Print(self.expression()?))
            }
            _ => Ok(Stmt::Expr(self.expression()?)),
        }
    }

    fn expression(&mut self) -> Result<Expr, Error> {
        let mut left = self.term()?;
        loop {
            let op = match self.peek() {
                Some(Token::Plus) => Op::Add,
                Some(Token::Minus) => Op::Sub,
                _ => break,
            };
            self.pos += 1;
            let right = self.term()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn term(&mut self) -> Result<Expr, Error> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek() {
                Some(Token::Star) => Op::Mul,
                Some(Token::Slash) => Op::Div,
                _ => break,
            };
            self.pos += 1;
            let right = self.unary()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr, Error> {
        if let Some(Token::Minus) = self.peek() {
            self.pos += 1;
            return Ok(Expr::Negate(Box::new(self.unary()?)));
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, Error> {
        match self.advance() {
            Some(Token::Number(n)) => Ok(Expr::Number(n)),
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

    fn advance(&mut self) -> Option<Token> {
        let (token, line) = self.tokens.get(self.pos).cloned()?;
        self.line = line;
        self.pos += 1;
        Some(token)
    }
}
