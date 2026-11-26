use std::collections::HashMap;
use std::rc::Rc;

use crate::parser::{Block, Expr as Ast, Op, Stmt as AstStmt, Type};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Num,
    Bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arith {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Num(f64),
    Bool(bool),
    Local(u32),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Arith(Arith, Box<Expr>, Box<Expr>),
    Compare(Cmp, Kind, Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Call(u32, Vec<Expr>),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Set(u32, Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    Break,
    Continue,
    Return(Expr),
    Discard(Expr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub params: Vec<Kind>,
    pub returns: Kind,
}

#[derive(Debug, Clone)]
pub struct Kernel {
    pub name: String,
    pub signature: Signature,
    pub locals: Vec<Kind>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejected {
    Unsupported(&'static str),
    Needs(String),
}

#[derive(Debug)]
pub struct Source {
    pub name: String,
    pub params: Vec<String>,
    pub param_types: Vec<Option<Type>>,
    pub returns: Option<Type>,
    pub body: Rc<Block>,
    pub looping: bool,
}

pub fn has_loop(block: &Block) -> bool {
    block.iter().any(|(_, stmt)| match stmt {
        AstStmt::While(..) | AstStmt::For(..) => true,
        AstStmt::If(_, then, otherwise) => {
            has_loop(then) || otherwise.as_ref().is_some_and(has_loop)
        }
        _ => false,
    })
}

pub fn kind_of(ty: Option<&Type>) -> Result<Kind, Rejected> {
    match ty {
        None | Some(Type::Number) => Ok(Kind::Num),
        Some(Type::Bool) => Ok(Kind::Bool),
        Some(_) => Err(Rejected::Unsupported("a type other than number or bool")),
    }
}

pub fn assumed_signature(source: &Source) -> Result<Signature, Rejected> {
    let params = source
        .param_types
        .iter()
        .map(|ty| kind_of(ty.as_ref()))
        .collect::<Result<Vec<_>, _>>()?;
    if params.len() > 16 {
        return Err(Rejected::Unsupported("more than 16 parameters"));
    }
    Ok(Signature {
        params,
        returns: kind_of(source.returns.as_ref())?,
    })
}

struct Lowering<'a> {
    callees: &'a dyn Fn(&str) -> Option<(u32, Signature)>,
    locals: Vec<Kind>,
    scopes: Vec<HashMap<String, u32>>,
    returns: Kind,
    loops: usize,
}

type Lowered<T> = Result<T, Rejected>;

fn unsupported<T>(what: &'static str) -> Lowered<T> {
    Err(Rejected::Unsupported(what))
}

pub fn lower(
    source: &Source,
    signature: &Signature,
    callees: &dyn Fn(&str) -> Option<(u32, Signature)>,
) -> Lowered<Kernel> {
    let mut lowering = Lowering {
        callees,
        locals: signature.params.clone(),
        scopes: vec![HashMap::new()],
        returns: signature.returns,
        loops: 0,
    };
    for (index, name) in source.params.iter().enumerate() {
        lowering.scopes[0].insert(name.clone(), index as u32);
    }
    let body = lowering.block(&source.body)?;
    if !always_returns(&body) {
        return unsupported("a path that ends without a return value");
    }
    Ok(Kernel {
        name: source.name.clone(),
        signature: signature.clone(),
        locals: lowering.locals,
        body,
    })
}

fn always_returns(block: &[Stmt]) -> bool {
    block.iter().any(|stmt| match stmt {
        Stmt::Return(_) => true,
        Stmt::If(_, then, otherwise) => always_returns(then) && always_returns(otherwise),
        Stmt::While(Expr::Bool(true), body) => !escapes(body),
        _ => false,
    })
}

fn escapes(block: &[Stmt]) -> bool {
    block.iter().any(|stmt| match stmt {
        Stmt::Break => true,
        Stmt::If(_, then, otherwise) => escapes(then) || escapes(otherwise),
        _ => false,
    })
}

impl Lowering<'_> {
    fn resolve(&self, name: &str) -> Option<u32> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    fn declare(&mut self, name: &str, kind: Kind) -> u32 {
        let index = self.locals.len() as u32;
        self.locals.push(kind);
        self.scopes
            .last_mut()
            .expect("a scope is always open")
            .insert(name.to_string(), index);
        index
    }

    fn block(&mut self, block: &Block) -> Lowered<Vec<Stmt>> {
        self.scopes.push(HashMap::new());
        let mut out = Vec::new();
        let mut result = Ok(());
        for (_, stmt) in block {
            match self.statement(stmt) {
                Ok(Some(lowered)) => out.push(lowered),
                Ok(None) => {}
                Err(error) => {
                    result = Err(error);
                    break;
                }
            }
        }
        self.scopes.pop();
        result.map(|_| out)
    }

    fn statement(&mut self, stmt: &AstStmt) -> Lowered<Option<Stmt>> {
        Ok(Some(match stmt {
            AstStmt::Let(name, annotation, value) => {
                let (value, kind) = self.expression(value)?;
                if let Some(annotation) = annotation
                    && kind_of(Some(annotation))? != kind
                {
                    return unsupported("a variable whose annotation disagrees with its value");
                }
                let slot = self.declare(name, kind);
                Stmt::Set(slot, value)
            }
            AstStmt::Assign(name, value) => {
                let Some(slot) = self.resolve(name) else {
                    return unsupported("writing a global variable");
                };
                let (value, kind) = self.expression(value)?;
                if self.locals[slot as usize] != kind {
                    return unsupported("a variable that changes type");
                }
                Stmt::Set(slot, value)
            }
            AstStmt::If(condition, then, otherwise) => {
                let condition = self.condition(condition)?;
                let then = self.block(then)?;
                let otherwise = match otherwise {
                    Some(block) => self.block(block)?,
                    None => Vec::new(),
                };
                Stmt::If(condition, then, otherwise)
            }
            AstStmt::While(condition, body) => {
                let condition = self.condition(condition)?;
                self.loops += 1;
                let body = self.block(body);
                self.loops -= 1;
                Stmt::While(condition, body?)
            }
            AstStmt::Break => Stmt::Break,
            AstStmt::Continue => Stmt::Continue,
            AstStmt::Return(Some(value)) => {
                let (value, kind) = self.expression(value)?;
                if kind != self.returns {
                    return unsupported("a return value of another type");
                }
                Stmt::Return(value)
            }
            AstStmt::Return(None) => return unsupported("a return without a value"),
            AstStmt::Expr(expr) => Stmt::Discard(self.expression(expr)?.0),
            AstStmt::Test(..) => return Ok(None),
            AstStmt::For(..) => return unsupported("a for loop"),
            AstStmt::SetIndex(..) | AstStmt::SetField(..) => {
                return unsupported("a list, record or struct");
            }
            AstStmt::Import(..) => return unsupported("an import"),
            AstStmt::Struct(_) | AstStmt::Enum(_) => return unsupported("a struct or enum"),
        }))
    }

    fn condition(&mut self, expr: &Ast) -> Lowered<Expr> {
        match self.expression(expr)? {
            (expr, Kind::Bool) => Ok(expr),
            _ => unsupported("a condition that is not a bool"),
        }
    }

    fn operand(&mut self, expr: &Ast, kind: Kind, what: &'static str) -> Lowered<Expr> {
        match self.expression(expr)? {
            (expr, found) if found == kind => Ok(expr),
            _ => unsupported(what),
        }
    }

    fn expression(&mut self, expr: &Ast) -> Lowered<(Expr, Kind)> {
        Ok(match expr {
            Ast::Number(n) => (Expr::Num(*n), Kind::Num),
            Ast::Bool(b) => (Expr::Bool(*b), Kind::Bool),
            Ast::Variable(name) => match self.resolve(name) {
                Some(slot) => (Expr::Local(slot), self.locals[slot as usize]),
                None => return unsupported("reading a global variable"),
            },
            Ast::Negate(inner) => {
                let inner = self.operand(inner, Kind::Num, "negating something not a number")?;
                (Expr::Neg(Box::new(inner)), Kind::Num)
            }
            Ast::Not(inner) => {
                let inner = self.operand(inner, Kind::Bool, "'!' on something not a bool")?;
                (Expr::Not(Box::new(inner)), Kind::Bool)
            }
            Ast::Binary(left, Op::And, right) => {
                let left = self.operand(left, Kind::Bool, "'&&' on something not a bool")?;
                let right = self.operand(right, Kind::Bool, "'&&' on something not a bool")?;
                (Expr::And(Box::new(left), Box::new(right)), Kind::Bool)
            }
            Ast::Binary(left, Op::Or, right) => {
                let left = self.operand(left, Kind::Bool, "'||' on something not a bool")?;
                let right = self.operand(right, Kind::Bool, "'||' on something not a bool")?;
                (Expr::Or(Box::new(left), Box::new(right)), Kind::Bool)
            }
            Ast::Binary(_, Op::Coalesce, _) => return unsupported("'??'"),
            Ast::Binary(left, op @ (Op::Equal | Op::NotEqual), right) => {
                let (left, kind) = self.expression(left)?;
                let right = self.operand(right, kind, "comparing values of different types")?;
                let cmp = if matches!(op, Op::Equal) {
                    Cmp::Eq
                } else {
                    Cmp::Ne
                };
                (
                    Expr::Compare(cmp, kind, Box::new(left), Box::new(right)),
                    Kind::Bool,
                )
            }
            Ast::Binary(left, op, right) => {
                let left = self.operand(left, Kind::Num, "arithmetic on something not a number")?;
                let right =
                    self.operand(right, Kind::Num, "arithmetic on something not a number")?;
                let (left, right) = (Box::new(left), Box::new(right));
                match op {
                    Op::Add => (Expr::Arith(Arith::Add, left, right), Kind::Num),
                    Op::Sub => (Expr::Arith(Arith::Sub, left, right), Kind::Num),
                    Op::Mul => (Expr::Arith(Arith::Mul, left, right), Kind::Num),
                    Op::Div => (Expr::Arith(Arith::Div, left, right), Kind::Num),
                    Op::Rem => (Expr::Arith(Arith::Rem, left, right), Kind::Num),
                    Op::Less => (Expr::Compare(Cmp::Lt, Kind::Num, left, right), Kind::Bool),
                    Op::LessEqual => (Expr::Compare(Cmp::Le, Kind::Num, left, right), Kind::Bool),
                    Op::Greater => (Expr::Compare(Cmp::Gt, Kind::Num, left, right), Kind::Bool),
                    Op::GreaterEqual => {
                        (Expr::Compare(Cmp::Ge, Kind::Num, left, right), Kind::Bool)
                    }
                    Op::Equal | Op::NotEqual | Op::And | Op::Or | Op::Coalesce => {
                        unreachable!("handled above")
                    }
                }
            }
            Ast::Call(callee, args) => {
                let Ast::Variable(name) = &**callee else {
                    return unsupported("calling something other than a named function");
                };
                if self.resolve(name).is_some() {
                    return unsupported("calling a local variable");
                }
                let Some((index, signature)) = (self.callees)(name) else {
                    return Err(Rejected::Needs(name.clone()));
                };
                if signature.params.len() != args.len() {
                    return unsupported("a call with the wrong number of arguments");
                }
                let mut lowered = Vec::with_capacity(args.len());
                for (arg, kind) in args.iter().zip(&signature.params) {
                    lowered.push(self.operand(arg, *kind, "an argument of another type")?);
                }
                (Expr::Call(index, lowered), signature.returns)
            }
            Ast::Str(_) | Ast::Template(_) => return unsupported("a string"),
            Ast::List(_) | Ast::Index(..) => return unsupported("a list"),
            Ast::Record(_) | Ast::Field(..) | Ast::OptionalField(..) | Ast::StructLit(..) => {
                return unsupported("a record or struct");
            }
            Ast::None => return unsupported("none"),
            Ast::Function { .. } => return unsupported("a nested function"),
            Ast::Match(..) => return unsupported("match"),
        })
    }
}

pub fn group(root: &str, find: &dyn Fn(&str) -> Option<Rc<Source>>) -> Lowered<Vec<Kernel>> {
    let mut order: Vec<Rc<Source>> = vec![find(root).ok_or(Rejected::Needs(root.to_string()))?];
    let mut signatures: HashMap<String, (u32, Signature)> = HashMap::new();
    signatures.insert(root.to_string(), (0, assumed_signature(&order[0])?));
    loop {
        let mut kernels = Vec::with_capacity(order.len());
        let mut missing = None;
        for source in &order {
            let signature = signatures[&source.name].1.clone();
            match lower(source, &signature, &|callee| {
                signatures.get(callee).cloned()
            }) {
                Ok(kernel) => kernels.push(kernel),
                Err(Rejected::Needs(callee)) => {
                    missing = Some(callee);
                    break;
                }
                Err(error) => return Err(error),
            }
        }
        let Some(callee) = missing else {
            return Ok(kernels);
        };
        let Some(source) = find(&callee) else {
            return unsupported("a call to something that is not a plain function");
        };
        if order.len() >= 64 {
            return unsupported("too many functions calling each other");
        }
        signatures.insert(callee, (order.len() as u32, assumed_signature(&source)?));
        order.push(source);
    }
}

pub fn group_all(roots: &[Rc<Source>]) -> Vec<Kernel> {
    let mut candidates: Vec<Rc<Source>> = roots.to_vec();
    loop {
        let mut signatures: HashMap<String, (u32, Signature)> = HashMap::new();
        for (index, source) in candidates.iter().enumerate() {
            if let Ok(signature) = assumed_signature(source) {
                signatures.insert(source.name.clone(), (index as u32, signature));
            }
        }
        let mut kernels = Vec::new();
        let mut failed = Vec::new();
        for source in &candidates {
            let lowered = signatures.get(&source.name).and_then(|(_, signature)| {
                lower(source, signature, &|callee| signatures.get(callee).cloned()).ok()
            });
            match lowered {
                Some(kernel) => kernels.push(kernel),
                None => failed.push(source.name.clone()),
            }
        }
        if failed.is_empty() {
            return kernels;
        }
        candidates.retain(|source| !failed.contains(&source.name));
    }
}

pub fn top_level(program: &Block) -> Vec<Rc<Source>> {
    program
        .iter()
        .filter_map(|(_, stmt)| match stmt {
            AstStmt::Let(
                name,
                _,
                crate::parser::Expr::Function {
                    params,
                    param_types,
                    returns,
                    body,
                    ..
                },
            ) => Some(Rc::new(Source {
                name: name.clone(),
                params: params.clone(),
                param_types: param_types.clone(),
                returns: returns.clone(),
                looping: has_loop(body),
                body: Rc::clone(body),
            })),
            _ => None,
        })
        .collect()
}
