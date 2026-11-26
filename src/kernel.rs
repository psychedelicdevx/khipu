use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::native::Tier;

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
    For {
        counter: u32,
        limit: u32,
        var: u32,
        body: Vec<Stmt>,
    },
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
    pub loop_vars: usize,
}

impl Kernel {
    pub fn arity(&self) -> usize {
        self.signature.params.len() + self.loop_vars
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Local(u32),
    Global(u32),
}

pub struct LoopSource {
    pub owner: String,
    pub index: u32,
    pub stmt: AstStmt,
    pub vars: Vec<(String, Place)>,
    pub tier: Cell<Tier>,
}

impl LoopSource {
    pub fn key(&self) -> String {
        format!("{}\u{0}{}", self.owner, self.index)
    }
}

pub fn loop_names(stmt: &AstStmt) -> Vec<String> {
    let mut used = Vec::new();
    let mut called = Vec::new();
    let mut bound = Vec::new();
    walk_stmt(stmt, &mut used, &mut called, &mut bound);
    used.retain(|name| !called.contains(name) && !bound.contains(name));
    used
}

fn remember(names: &mut Vec<String>, name: &str) {
    if !names.iter().any(|n| n == name) {
        names.push(name.to_string());
    }
}

type Names<'a> = (
    &'a mut Vec<String>,
    &'a mut Vec<String>,
    &'a mut Vec<String>,
);

fn walk_block(
    block: &Block,
    used: &mut Vec<String>,
    called: &mut Vec<String>,
    bound: &mut Vec<String>,
) {
    for (_, stmt) in block {
        walk_stmt(stmt, used, called, bound);
    }
}

fn walk_stmt(
    stmt: &AstStmt,
    used: &mut Vec<String>,
    called: &mut Vec<String>,
    bound: &mut Vec<String>,
) {
    match stmt {
        AstStmt::Let(name, _, value) => {
            remember(bound, name);
            walk_expr(value, (used, called, bound));
        }
        AstStmt::Expr(value) => walk_expr(value, (used, called, bound)),
        AstStmt::Assign(name, value) => {
            remember(used, name);
            walk_expr(value, (used, called, bound));
        }
        AstStmt::SetIndex(a, b, c) => {
            walk_expr(a, (used, called, bound));
            walk_expr(b, (used, called, bound));
            walk_expr(c, (used, called, bound));
        }
        AstStmt::SetField(a, _, b) => {
            walk_expr(a, (used, called, bound));
            walk_expr(b, (used, called, bound));
        }
        AstStmt::If(condition, then, otherwise) => {
            walk_expr(condition, (used, called, bound));
            walk_block(then, used, called, bound);
            if let Some(otherwise) = otherwise {
                walk_block(otherwise, used, called, bound);
            }
        }
        AstStmt::While(condition, body) => {
            walk_expr(condition, (used, called, bound));
            walk_block(body, used, called, bound);
        }
        AstStmt::For(name, iterable, body) => {
            remember(bound, name);
            walk_expr(iterable, (used, called, bound));
            walk_block(body, used, called, bound);
        }
        AstStmt::Return(Some(value)) => walk_expr(value, (used, called, bound)),
        AstStmt::Return(None)
        | AstStmt::Break
        | AstStmt::Continue
        | AstStmt::Import(..)
        | AstStmt::Struct(_)
        | AstStmt::Enum(_)
        | AstStmt::Test(..) => {}
    }
}

fn walk_expr(expr: &Ast, names: Names) {
    let (used, called, bound) = names;
    match expr {
        Ast::Variable(name) => remember(used, name),
        Ast::Call(callee, args) => {
            match &**callee {
                Ast::Variable(name) => remember(called, name),
                other => walk_expr(other, (used, called, bound)),
            }
            for arg in args {
                walk_expr(arg, (used, called, bound));
            }
        }
        Ast::Number(_) | Ast::Bool(_) | Ast::Str(_) | Ast::None => {}
        Ast::Template(items) | Ast::List(items) => {
            for item in items {
                walk_expr(item, (used, called, bound));
            }
        }
        Ast::Record(fields) | Ast::StructLit(_, fields) => {
            for (_, value) in fields {
                walk_expr(value, (used, called, bound));
            }
        }
        Ast::Index(a, b) | Ast::Binary(a, _, b) => {
            walk_expr(a, (used, called, bound));
            walk_expr(b, (used, called, bound));
        }
        Ast::Field(inner, _)
        | Ast::OptionalField(inner, _)
        | Ast::Negate(inner)
        | Ast::Not(inner) => walk_expr(inner, (used, called, bound)),
        Ast::Function { body, .. } => walk_block(body, used, called, bound),
        Ast::Match(subject, arms) => {
            walk_expr(subject, (used, called, bound));
            for arm in arms {
                match &arm.body {
                    crate::parser::ArmBody::Expr(value) => walk_expr(value, (used, called, bound)),
                    crate::parser::ArmBody::Block(block) => walk_block(block, used, called, bound),
                }
            }
        }
    }
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
    returns: Option<Kind>,
    loops: usize,
    range_is_builtin: bool,
}

type Lowered<T> = Result<T, Rejected>;

fn unsupported<T>(what: &'static str) -> Lowered<T> {
    Err(Rejected::Unsupported(what))
}

pub fn lower(
    source: &Source,
    signature: &Signature,
    callees: &dyn Fn(&str) -> Option<(u32, Signature)>,
    range_is_builtin: bool,
) -> Lowered<Kernel> {
    let mut lowering = Lowering {
        callees,
        locals: signature.params.clone(),
        scopes: vec![HashMap::new()],
        returns: Some(signature.returns),
        loops: 0,
        range_is_builtin,
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
        loop_vars: 0,
    })
}

pub fn lower_loop(
    source: &LoopSource,
    kinds: &[Kind],
    callees: &dyn Fn(&str) -> Option<(u32, Signature)>,
    range_is_builtin: bool,
) -> Lowered<Kernel> {
    let mut lowering = Lowering {
        callees,
        locals: kinds.to_vec(),
        scopes: vec![HashMap::new()],
        returns: None,
        loops: 0,
        range_is_builtin,
    };
    for (index, (name, _)) in source.vars.iter().enumerate() {
        lowering.scopes[0].insert(name.clone(), index as u32);
    }
    let mut body = lowering.statement(&source.stmt)?;
    body.push(Stmt::Return(Expr::Num(0.0)));
    Ok(Kernel {
        name: source.key(),
        signature: Signature {
            params: Vec::new(),
            returns: Kind::Num,
        },
        locals: lowering.locals,
        body,
        loop_vars: kinds.len(),
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
                Ok(lowered) => out.extend(lowered),
                Err(error) => {
                    result = Err(error);
                    break;
                }
            }
        }
        self.scopes.pop();
        result.map(|_| out)
    }

    fn statement(&mut self, stmt: &AstStmt) -> Lowered<Vec<Stmt>> {
        Ok(vec![match stmt {
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
                if self.returns != Some(kind) {
                    return unsupported("a return value of another type");
                }
                Stmt::Return(value)
            }
            AstStmt::Return(None) => return unsupported("a return without a value"),
            AstStmt::Expr(expr) => Stmt::Discard(self.expression(expr)?.0),
            AstStmt::Test(..) => return Ok(Vec::new()),
            AstStmt::For(name, iterable, body) => {
                let Ast::Call(callee, args) = iterable else {
                    return unsupported("a for loop over something other than range");
                };
                let is_range = matches!(&**callee, Ast::Variable(callee) if callee == "range")
                    && self.resolve("range").is_none()
                    && self.range_is_builtin;
                if !is_range || args.is_empty() || args.len() > 2 {
                    return unsupported("a for loop over something other than range");
                }
                let (start, end) = match &args[..] {
                    [end] => (
                        Expr::Num(0.0),
                        self.operand(end, Kind::Num, "a range bound that is not a number")?,
                    ),
                    [start, end] => (
                        self.operand(start, Kind::Num, "a range bound that is not a number")?,
                        self.operand(end, Kind::Num, "a range bound that is not a number")?,
                    ),
                    _ => unreachable!("checked above"),
                };
                let counter = self.declare("", Kind::Num);
                let limit = self.declare("", Kind::Num);
                self.scopes.push(HashMap::new());
                let var = self.declare(name, Kind::Num);
                self.loops += 1;
                let lowered = self.block(body);
                self.loops -= 1;
                self.scopes.pop();
                return Ok(vec![
                    Stmt::Set(counter, start),
                    Stmt::Set(limit, end),
                    Stmt::For {
                        counter,
                        limit,
                        var,
                        body: lowered?,
                    },
                ]);
            }
            AstStmt::SetIndex(..) | AstStmt::SetField(..) => {
                return unsupported("a list, record or struct");
            }
            AstStmt::Import(..) => return unsupported("an import"),
            AstStmt::Struct(_) | AstStmt::Enum(_) => return unsupported("a struct or enum"),
        }])
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

type Callees<'a> = dyn Fn(&str) -> Option<(u32, Signature)> + 'a;

fn resolve_group(
    root: &dyn Fn(&Callees) -> Lowered<Kernel>,
    root_function: Option<(&str, Signature)>,
    find: &dyn Fn(&str) -> Option<Rc<Source>>,
    range_is_builtin: bool,
) -> Lowered<Vec<Kernel>> {
    let mut order: Vec<Rc<Source>> = Vec::new();
    let mut signatures: HashMap<String, (u32, Signature)> = HashMap::new();
    if let Some((name, signature)) = &root_function {
        signatures.insert(name.to_string(), (0, signature.clone()));
    }
    loop {
        let callees = |callee: &str| signatures.get(callee).cloned();
        let mut kernels = Vec::new();
        let mut missing = None;
        match root(&callees) {
            Ok(kernel) => kernels.push(kernel),
            Err(Rejected::Needs(callee)) => missing = Some(callee),
            Err(error) => return Err(error),
        }
        for source in &order {
            if missing.is_some() {
                break;
            }
            let signature = signatures[&source.name].1.clone();
            match lower(source, &signature, &callees, range_is_builtin) {
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
        signatures.insert(
            callee,
            (order.len() as u32 + 1, assumed_signature(&source)?),
        );
        order.push(source);
    }
}

pub fn group(
    root: &str,
    find: &dyn Fn(&str) -> Option<Rc<Source>>,
    range_is_builtin: bool,
) -> Lowered<Vec<Kernel>> {
    let source = find(root).ok_or(Rejected::Needs(root.to_string()))?;
    let signature = assumed_signature(&source)?;
    resolve_group(
        &|callees| lower(&source, &signature, callees, range_is_builtin),
        Some((root, signature.clone())),
        find,
        range_is_builtin,
    )
}

pub fn group_loop(
    source: &LoopSource,
    kinds: &[Kind],
    find: &dyn Fn(&str) -> Option<Rc<Source>>,
    range_is_builtin: bool,
) -> Lowered<Vec<Kernel>> {
    resolve_group(
        &|callees| lower_loop(source, kinds, callees, range_is_builtin),
        None,
        find,
        range_is_builtin,
    )
}

pub fn group_all(
    functions: &[Rc<Source>],
    loops: &[Rc<LoopSource>],
    range_is_builtin: bool,
) -> Vec<Kernel> {
    let mut candidates: Vec<Rc<Source>> = functions.to_vec();
    let mut kernels = loop {
        let mut signatures: HashMap<String, (u32, Signature)> = HashMap::new();
        for (index, source) in candidates.iter().enumerate() {
            if let Ok(signature) = assumed_signature(source) {
                signatures.insert(source.name.clone(), (index as u32, signature));
            }
        }
        let callees = |callee: &str| signatures.get(callee).cloned();
        let mut kernels = Vec::new();
        let mut failed = Vec::new();
        for source in &candidates {
            let lowered = signatures.get(&source.name).and_then(|(_, signature)| {
                lower(source, signature, &callees, range_is_builtin).ok()
            });
            match lowered {
                Some(kernel) => kernels.push(kernel),
                None => failed.push(source.name.clone()),
            }
        }
        if failed.is_empty() {
            break kernels;
        }
        candidates.retain(|source| !failed.contains(&source.name));
    };
    let signatures: HashMap<String, (u32, Signature)> = kernels
        .iter()
        .enumerate()
        .map(|(index, kernel)| {
            (
                kernel.name.clone(),
                (index as u32, kernel.signature.clone()),
            )
        })
        .collect();
    let callees = |callee: &str| signatures.get(callee).cloned();
    for source in loops {
        let kinds = vec![Kind::Num; source.vars.len()];
        if let Ok(kernel) = lower_loop(source, &kinds, &callees, range_is_builtin) {
            kernels.push(kernel);
        }
    }
    kernels
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

pub fn defines_range(program: &Block) -> bool {
    program
        .iter()
        .any(|(_, stmt)| matches!(stmt, AstStmt::Let(name, _, _) if name == "range"))
}
