use std::rc::Rc;

use crate::error::Error;
use crate::lexer::{StringPart, Token, lex};

pub type Block = Vec<(usize, Stmt)>;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Any,
    Number,
    Bool,
    Str,
    None,
    List(Box<Type>),
    Record,
    Function(Vec<Type>, Box<Type>),
    Optional(Box<Type>),
    Named(String),
    Param(String),
    Struct(String),
    StructDef(String),
    Enum(String),
    EnumDef(String),
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Type::Any => write!(f, "any"),
            Type::Number => write!(f, "number"),
            Type::Bool => write!(f, "bool"),
            Type::Str => write!(f, "string"),
            Type::None => write!(f, "none"),
            Type::List(inner) if **inner == Type::Any => write!(f, "list"),
            Type::List(inner) => write!(f, "list<{inner}>"),
            Type::Record => write!(f, "record"),
            Type::Function(params, returns) => {
                let params: Vec<String> = params.iter().map(Type::to_string).collect();
                write!(f, "fn({}) -> {returns}", params.join(", "))
            }
            Type::Optional(inner) => write!(f, "{inner}?"),
            Type::Named(name)
            | Type::Param(name)
            | Type::Struct(name)
            | Type::StructDef(name)
            | Type::Enum(name)
            | Type::EnumDef(name) => write!(f, "{name}"),
        }
    }
}

#[derive(Debug)]
pub enum Import {
    Module(String),
    Names(Vec<String>),
}

#[derive(Debug)]
pub struct StructDecl {
    pub name: String,
    pub fields: Vec<(String, Type)>,
    pub methods: Vec<(String, Expr)>,
}

#[derive(Debug)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<(String, Vec<Type>)>,
}

#[derive(Debug)]
pub enum Pattern {
    Wildcard,
    Bind(String),
    Literal(Expr),
    Variant {
        enum_name: Option<String>,
        variant: String,
        bindings: Vec<String>,
    },
}

#[derive(Debug)]
pub enum ArmBody {
    Expr(Expr),
    Block(Block),
}

#[derive(Debug)]
pub struct Arm {
    pub pattern: Pattern,
    pub body: ArmBody,
    pub line: usize,
}

#[derive(Debug)]
pub enum Stmt {
    Let(String, Option<Type>, Expr),
    Assign(String, Expr),
    SetIndex(Expr, Expr, Expr),
    SetField(Expr, String, Expr),
    If(Expr, Block, Option<Block>),
    While(Expr, Block),
    For(String, Expr, Block),
    Break,
    Continue,
    Return(Option<Expr>),
    Import(String, Import),
    Struct(Rc<StructDecl>),
    Enum(Rc<EnumDecl>),
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
    OptionalField(Box<Expr>, String),
    None,
    Variable(String),
    Negate(Box<Expr>),
    Not(Box<Expr>),
    Binary(Box<Expr>, Op, Box<Expr>),
    Function {
        name: String,
        type_params: Vec<String>,
        params: Vec<String>,
        param_types: Vec<Option<Type>>,
        returns: Option<Type>,
        body: Rc<Block>,
    },
    Call(Box<Expr>, Vec<Expr>),
    StructLit(String, Vec<(String, Expr)>),
    Match(Box<Expr>, Vec<Arm>),
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
    Coalesce,
}

pub struct Parser {
    tokens: Vec<(Token, usize)>,
    pos: usize,
    line: usize,
    function_depth: usize,
    loop_depth: usize,
    no_struct: bool,
}

impl Parser {
    pub fn new(tokens: Vec<(Token, usize)>) -> Self {
        Parser {
            tokens,
            pos: 0,
            line: 1,
            function_depth: 0,
            loop_depth: 0,
            no_struct: false,
        }
    }

    pub fn parse_program(&mut self) -> Result<Block, Error> {
        self.statements(None)
    }

    fn statements(&mut self, closing: Option<Token>) -> Result<Block, Error> {
        let outer = std::mem::replace(&mut self.no_struct, false);
        let result = self.statements_inner(closing);
        self.no_struct = outer;
        result
    }

    fn restricted_expression(&mut self) -> Result<Expr, Error> {
        let outer = std::mem::replace(&mut self.no_struct, true);
        let result = self.expression();
        self.no_struct = outer;
        result
    }

    fn free_expression(&mut self) -> Result<Expr, Error> {
        let outer = std::mem::replace(&mut self.no_struct, false);
        let result = self.expression();
        self.no_struct = outer;
        result
    }

    fn statements_inner(&mut self, closing: Option<Token>) -> Result<Block, Error> {
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
                let annotation = if self.peek() == Some(&Token::Colon) {
                    self.pos += 1;
                    Some(self.parse_type()?)
                } else {
                    None
                };
                self.expect(Token::Equals)?;
                let value = named(self.expression()?, &name);
                Ok(Stmt::Let(name, annotation, value))
            }
            Some(Token::Fn) if matches!(self.peek_next(), Some(Token::Ident(_))) => {
                self.pos += 1;
                let name = self.identifier()?;
                let function = self.function(name.clone())?;
                Ok(Stmt::Let(name, None, function))
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
            Some(Token::Struct) => {
                self.pos += 1;
                self.struct_decl()
            }
            Some(Token::Enum) => {
                self.pos += 1;
                self.enum_decl()
            }
            Some(Token::While) => {
                self.pos += 1;
                let condition = self.restricted_expression()?;
                let body = self.loop_body()?;
                Ok(Stmt::While(condition, body))
            }
            Some(Token::For) => {
                self.pos += 1;
                let name = self.identifier()?;
                self.expect(Token::In)?;
                let iterable = self.restricted_expression()?;
                let body = self.loop_body()?;
                Ok(Stmt::For(name, iterable, body))
            }
            Some(Token::Import) => {
                self.pos += 1;
                let binding = if self.peek() == Some(&Token::LeftBrace) {
                    self.pos += 1;
                    let mut names = Vec::new();
                    loop {
                        self.skip_newlines();
                        names.push(self.identifier()?);
                        self.skip_newlines();
                        match self.advance() {
                            Some(Token::Comma) => {}
                            Some(Token::RightBrace) => break,
                            Some(token) => {
                                return Err(
                                    self.error(format!("expected ',' or '}}', found {token:?}"))
                                );
                            }
                            None => {
                                return Err(self.error("import list is never closed".to_string()));
                            }
                        }
                    }
                    Import::Names(names)
                } else {
                    Import::Module(self.identifier()?)
                };
                self.expect(Token::From)?;
                match self.advance() {
                    Some(Token::Str(path)) => Ok(Stmt::Import(path, binding)),
                    Some(token) => Err(self.error(format!(
                        "expected a file path like \"math.kp\", found {token:?}"
                    ))),
                    None => Err(self.error("expected a file path like \"math.kp\"".to_string())),
                }
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
        let condition = self.restricted_expression()?;
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
        let mut left = self.or()?;
        while self.peek() == Some(&Token::QuestionQuestion) {
            self.pos += 1;
            let right = self.or()?;
            left = Expr::Binary(Box::new(left), Op::Coalesce, Box::new(right));
        }
        Ok(left)
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
                Some(Token::QuestionDot) => {
                    self.pos += 1;
                    let name = self.identifier()?;
                    expr = Expr::OptionalField(Box::new(expr), name);
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
            items.push(self.free_expression()?);
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
        let mut type_params = Vec::new();
        if self.peek() == Some(&Token::Less) {
            self.pos += 1;
            loop {
                type_params.push(self.identifier()?);
                match self.advance() {
                    Some(Token::Comma) => {}
                    Some(Token::Greater) => break,
                    Some(token) => {
                        return Err(self.error(format!("expected ',' or '>', found {token:?}")));
                    }
                    None => return Err(self.error("type parameters are never closed".to_string())),
                }
            }
        }
        self.expect(Token::LeftParen)?;
        let mut params = Vec::new();
        let mut param_types = Vec::new();
        if self.peek() != Some(&Token::RightParen) {
            loop {
                params.push(self.identifier()?);
                param_types.push(if self.peek() == Some(&Token::Colon) {
                    self.pos += 1;
                    Some(self.parse_type()?)
                } else {
                    None
                });
                if self.peek() != Some(&Token::Comma) {
                    break;
                }
                self.pos += 1;
            }
        }
        self.expect(Token::RightParen)?;
        let returns = if self.peek() == Some(&Token::Arrow) {
            self.pos += 1;
            Some(self.parse_type()?)
        } else {
            None
        };

        let outer_loops = std::mem::replace(&mut self.loop_depth, 0);
        self.function_depth += 1;
        let body = self.block();
        self.function_depth -= 1;
        self.loop_depth = outer_loops;

        Ok(Expr::Function {
            name,
            type_params,
            params,
            param_types,
            returns,
            body: Rc::new(body?),
        })
    }

    fn struct_decl(&mut self) -> Result<Stmt, Error> {
        let name = self.identifier()?;
        self.expect(Token::LeftBrace)?;
        let mut fields: Vec<(String, Type)> = Vec::new();
        let mut methods: Vec<(String, Expr)> = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek() {
                Some(Token::RightBrace) => {
                    self.pos += 1;
                    break;
                }
                Some(Token::Fn) => {
                    self.pos += 1;
                    let method = self.identifier()?;
                    if methods.iter().any(|(existing, _)| *existing == method) {
                        return Err(self.error(format!("method '{method}' is defined twice")));
                    }
                    let function = self.function(method.clone())?;
                    methods.push((method, function));
                }
                Some(Token::Ident(_)) => {
                    let field = self.identifier()?;
                    if fields.iter().any(|(existing, _)| *existing == field) {
                        return Err(self.error(format!("field '{field}' is listed twice")));
                    }
                    self.expect(Token::Colon)?;
                    fields.push((field, self.parse_type()?));
                }
                Some(token) => {
                    let token = token.clone();
                    return Err(
                        self.error(format!("expected a field or a method, found {token:?}"))
                    );
                }
                None => return Err(self.error(format!("struct {name} is never closed"))),
            }
            self.skip_newlines();
            if self.peek() == Some(&Token::Comma) {
                self.pos += 1;
            }
        }
        Ok(Stmt::Struct(Rc::new(StructDecl {
            name,
            fields,
            methods,
        })))
    }

    fn enum_decl(&mut self) -> Result<Stmt, Error> {
        let name = self.identifier()?;
        self.expect(Token::LeftBrace)?;
        let mut variants: Vec<(String, Vec<Type>)> = Vec::new();
        loop {
            self.skip_newlines();
            if self.peek() == Some(&Token::RightBrace) {
                self.pos += 1;
                break;
            }
            let variant = self.identifier()?;
            if variants.iter().any(|(existing, _)| *existing == variant) {
                return Err(self.error(format!("variant '{variant}' is listed twice")));
            }
            let mut payload = Vec::new();
            if self.peek() == Some(&Token::LeftParen) {
                self.pos += 1;
                loop {
                    payload.push(self.parse_type()?);
                    match self.advance() {
                        Some(Token::Comma) => {}
                        Some(Token::RightParen) => break,
                        Some(token) => {
                            return Err(self.error(format!("expected ',' or ')', found {token:?}")));
                        }
                        None => return Err(self.error(format!("enum {name} is never closed"))),
                    }
                }
            }
            variants.push((variant, payload));
            self.skip_newlines();
            if self.peek() == Some(&Token::Comma) {
                self.pos += 1;
            }
        }
        Ok(Stmt::Enum(Rc::new(EnumDecl { name, variants })))
    }

    fn match_expr(&mut self) -> Result<Expr, Error> {
        let scrutinee = self.restricted_expression()?;
        self.expect(Token::LeftBrace)?;
        let mut arms = Vec::new();
        loop {
            self.skip_newlines();
            if self.peek() == Some(&Token::RightBrace) {
                self.pos += 1;
                break;
            }
            let line = match self.tokens.get(self.pos) {
                Some((_, line)) => *line,
                None => return Err(self.error("match is never closed".to_string())),
            };
            let pattern = self.pattern()?;
            self.expect(Token::FatArrow)?;
            let body = if self.peek() == Some(&Token::LeftBrace) {
                ArmBody::Block(self.block()?)
            } else {
                ArmBody::Expr(self.free_expression()?)
            };
            arms.push(Arm {
                pattern,
                body,
                line,
            });
            self.skip_newlines();
            if self.peek() == Some(&Token::Comma) {
                self.pos += 1;
            }
        }
        Ok(Expr::Match(Box::new(scrutinee), arms))
    }

    fn pattern(&mut self) -> Result<Pattern, Error> {
        match self.advance() {
            Some(Token::Ident(name)) if name == "_" => Ok(Pattern::Wildcard),
            Some(Token::Number(n)) => Ok(Pattern::Literal(Expr::Number(n))),
            Some(Token::Minus) => match self.advance() {
                Some(Token::Number(n)) => Ok(Pattern::Literal(Expr::Number(-n))),
                _ => Err(self.error("expected a number after '-' in a pattern".to_string())),
            },
            Some(Token::Str(text)) => Ok(Pattern::Literal(Expr::Str(text))),
            Some(Token::True) => Ok(Pattern::Literal(Expr::Bool(true))),
            Some(Token::False) => Ok(Pattern::Literal(Expr::Bool(false))),
            Some(Token::NoneKeyword) => Ok(Pattern::Literal(Expr::None)),
            Some(Token::Ident(name)) => {
                if self.peek() == Some(&Token::Dot) {
                    self.pos += 1;
                    let variant = self.identifier()?;
                    let bindings = self.pattern_bindings()?;
                    Ok(Pattern::Variant {
                        enum_name: Some(name),
                        variant,
                        bindings,
                    })
                } else if name.starts_with(char::is_uppercase) {
                    let bindings = self.pattern_bindings()?;
                    Ok(Pattern::Variant {
                        enum_name: None,
                        variant: name,
                        bindings,
                    })
                } else {
                    Ok(Pattern::Bind(name))
                }
            }
            Some(token) => Err(self.error(format!("expected a pattern, found {token:?}"))),
            None => Err(self.error("expected a pattern, found end of input".to_string())),
        }
    }

    fn pattern_bindings(&mut self) -> Result<Vec<String>, Error> {
        let mut bindings = Vec::new();
        if self.peek() != Some(&Token::LeftParen) {
            return Ok(bindings);
        }
        self.pos += 1;
        loop {
            bindings.push(self.identifier()?);
            match self.advance() {
                Some(Token::Comma) => {}
                Some(Token::RightParen) => return Ok(bindings),
                Some(token) => {
                    return Err(self.error(format!("expected ',' or ')', found {token:?}")));
                }
                None => return Err(self.error("pattern is never closed".to_string())),
            }
        }
    }

    fn parse_type(&mut self) -> Result<Type, Error> {
        let base = match self.advance() {
            Some(Token::Ident(name)) => match name.as_str() {
                "number" => Type::Number,
                "bool" => Type::Bool,
                "string" => Type::Str,
                "any" => Type::Any,
                "list" => {
                    if self.peek() == Some(&Token::Less) {
                        self.pos += 1;
                        let inner = self.parse_type()?;
                        self.expect(Token::Greater)?;
                        Type::List(Box::new(inner))
                    } else {
                        Type::List(Box::new(Type::Any))
                    }
                }
                "record" => Type::Record,
                _ => Type::Named(name),
            },
            Some(Token::NoneKeyword) => Type::None,
            Some(Token::Fn) => {
                self.expect(Token::LeftParen)?;
                let mut params = Vec::new();
                if self.peek() != Some(&Token::RightParen) {
                    loop {
                        params.push(self.parse_type()?);
                        if self.peek() != Some(&Token::Comma) {
                            break;
                        }
                        self.pos += 1;
                    }
                }
                self.expect(Token::RightParen)?;
                let returns = if self.peek() == Some(&Token::Arrow) {
                    self.pos += 1;
                    self.parse_type()?
                } else {
                    Type::None
                };
                Type::Function(params, Box::new(returns))
            }
            Some(token) => return Err(self.error(format!("expected a type, found {token:?}"))),
            None => return Err(self.error("expected a type, found end of input".to_string())),
        };
        if self.peek() == Some(&Token::Question) {
            self.pos += 1;
            return Ok(match base {
                Type::None | Type::Any | Type::Optional(_) => base,
                other => Type::Optional(Box::new(other)),
            });
        }
        Ok(base)
    }

    fn primary(&mut self) -> Result<Expr, Error> {
        match self.advance() {
            Some(Token::Number(n)) => Ok(Expr::Number(n)),
            Some(Token::True) => Ok(Expr::Bool(true)),
            Some(Token::False) => Ok(Expr::Bool(false)),
            Some(Token::NoneKeyword) => Ok(Expr::None),
            Some(Token::Str(text)) => Ok(Expr::Str(text)),
            Some(Token::Template(parts)) => self.template(parts),
            Some(Token::Ident(name))
                if !self.no_struct
                    && name.starts_with(char::is_uppercase)
                    && self.peek() == Some(&Token::LeftBrace) =>
            {
                self.pos += 1;
                self.struct_literal(name)
            }
            Some(Token::Ident(name)) => Ok(Expr::Variable(name)),
            Some(Token::Match) => self.match_expr(),
            Some(Token::Fn) => self.function(String::new()),
            Some(Token::LeftBracket) => Ok(Expr::List(self.items(Token::RightBracket)?)),
            Some(Token::LeftBrace) => self.record(),
            Some(Token::LeftParen) => {
                let expr = self.free_expression()?;
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
            let value = named(self.free_expression()?, &key);
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

    fn struct_literal(&mut self, name: String) -> Result<Expr, Error> {
        let mut fields: Vec<(String, Expr)> = Vec::new();
        loop {
            self.skip_newlines();
            if self.peek() == Some(&Token::RightBrace) {
                self.pos += 1;
                return Ok(Expr::StructLit(name, fields));
            }
            let field = self.identifier()?;
            if fields.iter().any(|(existing, _)| *existing == field) {
                return Err(self.error(format!("field '{field}' is listed twice")));
            }
            self.expect(Token::Colon)?;
            let value = named(self.free_expression()?, &field);
            fields.push((field, value));
            self.skip_newlines();
            match self.advance() {
                Some(Token::Comma) => {}
                Some(Token::RightBrace) => return Ok(Expr::StructLit(name, fields)),
                Some(token) => {
                    return Err(self.error(format!("expected ',' or '}}', found {token:?}")));
                }
                None => return Err(self.error(format!("{name} {{ ... }} is never closed"))),
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

fn named(expr: Expr, new_name: &str) -> Expr {
    match expr {
        Expr::Function {
            name,
            type_params,
            params,
            param_types,
            returns,
            body,
        } if name.is_empty() => Expr::Function {
            name: new_name.to_string(),
            type_params,
            params,
            param_types,
            returns,
            body,
        },
        other => other,
    }
}
