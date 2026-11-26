use std::rc::Rc;

use crate::error::Error;
use crate::lexer::{StringPart, Token, lex};

pub type Block = Vec<(usize, Stmt)>;

#[derive(Debug)]
pub enum Stmt {
    Let(String, Expr),
    Assign(String, Expr),
    SetIndex(Expr, Expr, Expr),
    SetField(Expr, String, Expr),
    If(Expr, Block, Option<Block>),
    While(Expr, Block),
    For(String, Expr, Block),
    Break,
    Continue,
    Return(Option<Expr>),
    Expr(Expr),
}

#[derive(Debug)]
pub enum Expr {
    Number(f64),
    Bool(bool),
    Str(String),
    Template(Vec<Expr>),
    List(Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Record(Vec<(String, Expr)>),
    Field(Box<Expr>, String),
    Variable(String),
    Negate(Box<Expr>),
    Not(Box<Expr>),
    Binary(Box<Expr>, Op, Box<Expr>),
    Function {
        name: String,
        params: Vec<String>,
        body: Rc<Block>,
    },
    Call(Box<Expr>, Vec<Expr>),
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
    function_depth: usize,
    loop_depth: usize,
}

impl Parser {
    pub fn new(tokens: Vec<(Token, usize)>) -> Self {
        Parser {
            tokens,
            pos: 0,
            line: 1,
            function_depth: 0,
            loop_depth: 0,
        }
    }

    pub fn parse_program(&mut self) -> Result<Block, Error> {
        self.statements(None)
    }

    fn statements(&mut self, closing: Option<Token>) -> Result<Block, Error> {
        let mut statements = Vec::new();

        loop {
            self.skip_newlines();

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
                let value = match self.expression()? {
                    Expr::Function { params, body, .. } => Expr::Function {
                        name: name.clone(),
                        params,
                        body,
                    },
                    other => other,
                };
                Ok(Stmt::Let(name, value))
            }
            Some(Token::Fn) if matches!(self.peek_next(), Some(Token::Ident(_))) => {
                self.pos += 1;
                let name = self.identifier()?;
                let function = self.function(name.clone())?;
                Ok(Stmt::Let(name, function))
            }
            Some(Token::Return) => {
                self.pos += 1;
                if self.function_depth == 0 {
                    return Err(
                        self.error("'return' can only be used inside a function".to_string())
                    );
                }
                match self.peek() {
                    None | Some(Token::Newline) | Some(Token::RightBrace) => Ok(Stmt::Return(None)),
                    _ => Ok(Stmt::Return(Some(self.expression()?))),
                }
            }
            Some(Token::If) => {
                self.pos += 1;
                self.if_statement()
            }
            Some(Token::While) => {
                self.pos += 1;
                let condition = self.expression()?;
                let body = self.loop_body()?;
                Ok(Stmt::While(condition, body))
            }
            Some(Token::For) => {
                self.pos += 1;
                let name = self.identifier()?;
                self.expect(Token::In)?;
                let iterable = self.expression()?;
                let body = self.loop_body()?;
                Ok(Stmt::For(name, iterable, body))
            }
            Some(Token::Break) | Some(Token::Continue) => {
                let stmt = match self.advance() {
                    Some(Token::Break) => Stmt::Break,
                    _ => Stmt::Continue,
                };
                if self.loop_depth == 0 {
                    let word = if matches!(stmt, Stmt::Break) {
                        "break"
                    } else {
                        "continue"
                    };
                    return Err(self.error(format!("'{word}' can only be used inside a loop")));
                }
                Ok(stmt)
            }
            _ => {
                let target = self.expression()?;
                if self.peek() != Some(&Token::Equals) {
                    return Ok(Stmt::Expr(target));
                }
                self.pos += 1;
                let value = self.expression()?;
                match target {
                    Expr::Variable(name) => Ok(Stmt::Assign(name, value)),
                    Expr::Index(list, index) => Ok(Stmt::SetIndex(*list, *index, value)),
                    Expr::Field(record, name) => Ok(Stmt::SetField(*record, name, value)),
                    _ => Err(self.error(
                        "only a name, a list item or a field can be assigned to".to_string(),
                    )),
                }
            }
        }
    }

    fn loop_body(&mut self) -> Result<Block, Error> {
        self.loop_depth += 1;
        let body = self.block();
        self.loop_depth -= 1;
        body
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
            _ => self.call(),
        }
    }

    fn call(&mut self) -> Result<Expr, Error> {
        let mut expr = self.primary()?;
        loop {
            match self.peek() {
                Some(Token::LeftParen) => {
                    self.pos += 1;
                    let args = self.items(Token::RightParen)?;
                    expr = Expr::Call(Box::new(expr), args);
                }
                Some(Token::LeftBracket) => {
                    self.pos += 1;
                    let index = self.expression()?;
                    self.expect(Token::RightBracket)?;
                    expr = Expr::Index(Box::new(expr), Box::new(index));
                }
                Some(Token::Dot) => {
                    self.pos += 1;
                    let name = self.identifier()?;
                    expr = Expr::Field(Box::new(expr), name);
                }
                _ => return Ok(expr),
            }
        }
    }

    fn items(&mut self, closing: Token) -> Result<Vec<Expr>, Error> {
        let mut items = Vec::new();
        loop {
            self.skip_newlines();
            if self.peek() == Some(&closing) {
                self.pos += 1;
                return Ok(items);
            }
            items.push(self.expression()?);
            self.skip_newlines();
            match self.advance() {
                Some(Token::Comma) => {}
                Some(token) if token == closing => return Ok(items),
                Some(token) => {
                    return Err(self.error(format!("expected ',' or {closing:?}, found {token:?}")));
                }
                None => return Err(self.error(format!("expected {closing:?}, found end of input"))),
            }
        }
    }

    fn function(&mut self, name: String) -> Result<Expr, Error> {
        self.expect(Token::LeftParen)?;
        let mut params = Vec::new();
        if self.peek() != Some(&Token::RightParen) {
            loop {
                params.push(self.identifier()?);
                if self.peek() != Some(&Token::Comma) {
                    break;
                }
                self.pos += 1;
            }
        }
        self.expect(Token::RightParen)?;

        let outer_loops = std::mem::replace(&mut self.loop_depth, 0);
        self.function_depth += 1;
        let body = self.block();
        self.function_depth -= 1;
        self.loop_depth = outer_loops;

        Ok(Expr::Function {
            name,
            params,
            body: Rc::new(body?),
        })
    }

    fn primary(&mut self) -> Result<Expr, Error> {
        match self.advance() {
            Some(Token::Number(n)) => Ok(Expr::Number(n)),
            Some(Token::True) => Ok(Expr::Bool(true)),
            Some(Token::False) => Ok(Expr::Bool(false)),
            Some(Token::Str(text)) => Ok(Expr::Str(text)),
            Some(Token::Template(parts)) => self.template(parts),
            Some(Token::Ident(name)) => Ok(Expr::Variable(name)),
            Some(Token::Fn) => self.function(String::new()),
            Some(Token::LeftBracket) => Ok(Expr::List(self.items(Token::RightBracket)?)),
            Some(Token::LeftBrace) => self.record(),
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

    fn record(&mut self) -> Result<Expr, Error> {
        let mut fields: Vec<(String, Expr)> = Vec::new();
        loop {
            self.skip_newlines();
            if self.peek() == Some(&Token::RightBrace) {
                self.pos += 1;
                return Ok(Expr::Record(fields));
            }
            let key = match self.advance() {
                Some(Token::Ident(name)) | Some(Token::Str(name)) => name,
                Some(token) => {
                    return Err(self.error(format!("expected a field name, found {token:?}")));
                }
                None => return Err(self.error("record is never closed".to_string())),
            };
            if fields.iter().any(|(existing, _)| *existing == key) {
                return Err(self.error(format!("field '{key}' is listed twice")));
            }
            self.expect(Token::Colon)?;
            let value = match self.expression()? {
                Expr::Function { name, params, body } if name.is_empty() => Expr::Function {
                    name: key.clone(),
                    params,
                    body,
                },
                other => other,
            };
            fields.push((key, value));
            self.skip_newlines();
            match self.advance() {
                Some(Token::Comma) => {}
                Some(Token::RightBrace) => return Ok(Expr::Record(fields)),
                Some(token) => {
                    return Err(self.error(format!("expected ',' or '}}', found {token:?}")));
                }
                None => return Err(self.error("record is never closed".to_string())),
            }
        }
    }

    fn skip_newlines(&mut self) {
        while self.peek() == Some(&Token::Newline) {
            self.pos += 1;
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
