use std::collections::HashMap;

use crate::error::Error;
use crate::parser::{Block, Expr, Import, Op, Stmt, Type};

struct Function {
    name: String,
    returns: Option<Type>,
}

pub struct Checker {
    scopes: Vec<HashMap<String, Type>>,
    functions: Vec<Function>,
    line: usize,
}

impl Checker {
    pub fn new() -> Self {
        let any = || Box::new(Type::Any);
        let globals = HashMap::from([
            ("print".to_string(), Type::Any),
            ("range".to_string(), Type::Any),
            (
                "len".to_string(),
                Type::Function(vec![Type::Any], Box::new(Type::Number)),
            ),
            (
                "push".to_string(),
                Type::Function(vec![Type::Any, Type::Any], Box::new(Type::None)),
            ),
            ("keys".to_string(), Type::Function(vec![Type::Any], any())),
        ]);
        Checker {
            scopes: vec![globals],
            functions: Vec::new(),
            line: 1,
        }
    }

    pub fn check(&mut self, program: &Block) -> Result<(), Error> {
        let saved = self.scopes[0].clone();
        let result = self.block(program);
        if result.is_err() {
            self.scopes.truncate(1);
            self.scopes[0] = saved;
            self.functions.clear();
        }
        result
    }

    fn fail<T>(&self, message: impl Into<String>) -> Result<T, Error> {
        Err(Error {
            line: self.line,
            message: message.into(),
        })
    }

    fn declare(&mut self, name: &str, ty: Type) {
        self.scopes
            .last_mut()
            .expect("there is always a scope")
            .insert(name.to_string(), ty);
    }

    fn lookup(&self, name: &str) -> Option<&Type> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn scoped(&mut self, block: &Block) -> Result<(), Error> {
        self.scopes.push(HashMap::new());
        let result = self.block(block);
        self.scopes.pop();
        result
    }

    fn block(&mut self, block: &Block) -> Result<(), Error> {
        for (line, stmt) in block {
            if let Stmt::Let(
                name,
                None,
                Expr::Function {
                    param_types,
                    returns,
                    ..
                },
            ) = stmt
            {
                self.line = *line;
                let ty = signature(param_types, returns);
                self.declare(name, ty);
            }
        }
        for (line, stmt) in block {
            self.line = *line;
            self.statement(stmt)?;
        }
        Ok(())
    }

    fn statement(&mut self, stmt: &Stmt) -> Result<(), Error> {
        match stmt {
            Stmt::Let(name, annotation, expr) => {
                let actual = self.expression(expr)?;
                let ty = match annotation {
                    Some(expected) => {
                        self.expect(&actual, expected, || {
                            format!("'{name}' should be {expected}, got {actual}")
                        })?;
                        expected.clone()
                    }
                    None => widen(actual),
                };
                self.declare(name, ty);
            }
            Stmt::Assign(name, expr) => {
                let actual = self.expression(expr)?;
                if let Some(expected) = self.lookup(name).cloned() {
                    self.expect(&actual, &expected, || {
                        format!("'{name}' should be {expected}, got {actual}")
                    })?;
                }
            }
            Stmt::SetIndex(target, index, value) => {
                self.expression(target)?;
                self.expression(index)?;
                self.expression(value)?;
            }
            Stmt::SetField(target, _, value) => {
                self.expression(target)?;
                self.expression(value)?;
            }
            Stmt::If(condition, then_block, else_block) => {
                self.condition(condition, "if")?;
                self.scoped(then_block)?;
                if let Some(else_block) = else_block {
                    self.scoped(else_block)?;
                }
            }
            Stmt::While(condition, body) => {
                self.condition(condition, "while")?;
                self.scoped(body)?;
            }
            Stmt::For(name, iterable, body) => {
                let item = match self.expression(iterable)? {
                    Type::List | Type::Any => Type::Any,
                    Type::Str => Type::Str,
                    Type::Record => {
                        return self.fail("for needs a list or a string, got record, try 'for key in keys(record)'");
                    }
                    other => {
                        return self.fail(format!("for needs a list or a string, got {other}"));
                    }
                };
                self.scopes.push(HashMap::from([(name.clone(), item)]));
                let result = self.block(body);
                self.scopes.pop();
                result?;
            }
            Stmt::Return(expr) => {
                let actual = match expr {
                    Some(expr) => self.expression(expr)?,
                    None => Type::None,
                };
                if let Some(function) = self.functions.last()
                    && let Some(expected) = &function.returns
                {
                    let name = function.name.clone();
                    let expected = expected.clone();
                    self.expect(&actual, &expected, || {
                        format!("'{name}' should return {expected}, got {actual}")
                    })?;
                }
            }
            Stmt::Import(_, binding) => match binding {
                Import::Module(name) => self.declare(name, Type::Record),
                Import::Names(names) => {
                    for name in names {
                        self.declare(name, Type::Any);
                    }
                }
            },
            Stmt::Break | Stmt::Continue => {}
            Stmt::Expr(expr) => {
                self.expression(expr)?;
            }
        }
        Ok(())
    }

    fn expect(
        &self,
        actual: &Type,
        expected: &Type,
        message: impl Fn() -> String,
    ) -> Result<(), Error> {
        if assignable(actual, expected) {
            return Ok(());
        }
        let mut message = message();
        if let Type::Optional(inner) = actual
            && assignable(inner, expected)
        {
            message.push_str(" (it might be none, use ?? to give a fallback)");
        }
        self.fail(message)
    }

    fn condition(&mut self, condition: &Expr, what: &str) -> Result<(), Error> {
        match self.expression(condition)? {
            Type::Bool | Type::Any => Ok(()),
            other => self.fail(format!("{what} needs true or false, got {other}")),
        }
    }

    fn expression(&mut self, expr: &Expr) -> Result<Type, Error> {
        Ok(match expr {
            Expr::Number(_) => Type::Number,
            Expr::Bool(_) => Type::Bool,
            Expr::Str(_) => Type::Str,
            Expr::None => Type::None,
            Expr::Template(parts) => {
                for part in parts {
                    self.expression(part)?;
                }
                Type::Str
            }
            Expr::List(items) => {
                for item in items {
                    self.expression(item)?;
                }
                Type::List
            }
            Expr::Record(fields) => {
                for (_, value) in fields {
                    self.expression(value)?;
                }
                Type::Record
            }
            Expr::Variable(name) => self.lookup(name).cloned().unwrap_or(Type::Any),
            Expr::Index(target, index) => {
                let target = self.expression(target)?;
                let index = self.expression(index)?;
                match (&target, &index) {
                    (Type::List | Type::Str, Type::Number | Type::Any) => {
                        if target == Type::Str {
                            Type::Str
                        } else {
                            Type::Any
                        }
                    }
                    (Type::List | Type::Str, other) => {
                        return self.fail(format!("index must be a number, got {other}"));
                    }
                    (Type::Record, Type::Number) => {
                        return self.fail("record fields are named with strings, got number");
                    }
                    (Type::Record | Type::Any, _) => Type::Any,
                    (Type::Optional(_), _) => {
                        return self.fail("cannot index into a value that might be none");
                    }
                    (other, _) => return self.fail(format!("cannot index into {other}")),
                }
            }
            Expr::Field(target, name) => match self.expression(target)? {
                Type::Record | Type::Any => Type::Any,
                Type::None => {
                    return self.fail(format!(
                        "cannot read field '{name}' of none, use '?.{name}' if it might be none"
                    ));
                }
                Type::Optional(_) => {
                    return self.fail(format!(
                        "cannot read field '{name}' of a value that might be none, use '?.{name}'"
                    ));
                }
                other => return self.fail(format!("cannot read field '{name}' of {other}")),
            },
            Expr::OptionalField(target, name) => match self.expression(target)? {
                Type::Record | Type::Any | Type::None | Type::Optional(_) => Type::Any,
                other => return self.fail(format!("cannot read field '{name}' of {other}")),
            },
            Expr::Negate(inner) => match self.expression(inner)? {
                Type::Number | Type::Any => Type::Number,
                other => return self.fail(format!("'-' needs a number, got {other}")),
            },
            Expr::Not(inner) => match self.expression(inner)? {
                Type::Bool | Type::Any => Type::Bool,
                other => return self.fail(format!("'!' needs true or false, got {other}")),
            },
            Expr::Binary(left, op @ (Op::And | Op::Or), right) => {
                let symbol = if matches!(op, Op::And) {
                    "'&&'"
                } else {
                    "'||'"
                };
                for side in [left, right] {
                    match self.expression(side)? {
                        Type::Bool | Type::Any => {}
                        other => {
                            return self.fail(format!("{symbol} needs true or false, got {other}"));
                        }
                    }
                }
                Type::Bool
            }
            Expr::Binary(left, Op::Coalesce, right) => {
                let left = self.expression(left)?;
                let right = self.expression(right)?;
                match left {
                    Type::Optional(inner) if assignable(&right, &inner) => *inner,
                    Type::None => right,
                    Type::Any | Type::Optional(_) => Type::Any,
                    other => other,
                }
            }
            Expr::Binary(left, op, right) => {
                let left = self.expression(left)?;
                let right = self.expression(right)?;
                self.binary(*op, &left, &right)?
            }
            Expr::Function {
                name,
                params,
                param_types,
                returns,
                body,
            } => {
                let mut scope = HashMap::new();
                for (param, ty) in params.iter().zip(param_types) {
                    scope.insert(param.clone(), ty.clone().unwrap_or(Type::Any));
                }
                let label = if name.is_empty() {
                    "this function".to_string()
                } else {
                    name.clone()
                };
                self.scopes.push(scope);
                self.functions.push(Function {
                    name: label.clone(),
                    returns: returns.clone(),
                });
                let result = self.block(body);
                self.functions.pop();
                self.scopes.pop();
                result?;

                if let Some(expected) = returns
                    && !matches!(expected, Type::Any | Type::None | Type::Optional(_))
                    && !always_returns(body)
                {
                    return self.fail(format!(
                        "'{label}' should return {expected}, but can reach the end without returning"
                    ));
                }
                signature(param_types, returns)
            }
            Expr::Call(callee, args) => {
                let callee_type = self.expression(callee)?;
                let mut arg_types = Vec::with_capacity(args.len());
                for arg in args {
                    arg_types.push(self.expression(arg)?);
                }
                match callee_type {
                    Type::Function(params, returns) => {
                        let name = match &**callee {
                            Expr::Variable(name) => name.clone(),
                            _ => "this function".to_string(),
                        };
                        if params.len() != args.len() {
                            return self.fail(format!(
                                "{name} expects {} argument{}, got {}",
                                params.len(),
                                if params.len() == 1 { "" } else { "s" },
                                args.len()
                            ));
                        }
                        for (index, (actual, expected)) in arg_types.iter().zip(&params).enumerate()
                        {
                            self.expect(actual, expected, || {
                                format!(
                                    "argument {} of '{name}' should be {expected}, got {actual}",
                                    index + 1
                                )
                            })?;
                        }
                        *returns
                    }
                    Type::Any => Type::Any,
                    other => return self.fail(format!("{other} is not a function")),
                }
            }
        })
    }

    fn binary(&self, op: Op, left: &Type, right: &Type) -> Result<Type, Error> {
        use Type::{Any, Bool, Number, Str};
        let symbol = crate::value::symbol(op);
        let mismatch = || format!("cannot use '{symbol}' on {left} and {right}");
        Ok(match op {
            Op::Add => match (left, right) {
                (Number, Number) => Number,
                (Str, Str) => Str,
                (Any, _) | (_, Any) => Any,
                (Str, _) | (_, Str) => {
                    return self.fail(format!(
                        "{}, use \"${{...}}\" to put values inside a string",
                        mismatch()
                    ));
                }
                _ => return self.fail(mismatch()),
            },
            Op::Sub | Op::Mul | Op::Div | Op::Rem => match (left, right) {
                (Number | Any, Number | Any) => Number,
                _ => return self.fail(mismatch()),
            },
            Op::Less | Op::LessEqual | Op::Greater | Op::GreaterEqual => match (left, right) {
                (Number | Any, Number | Any) => Bool,
                _ => return self.fail(mismatch()),
            },
            Op::Equal | Op::NotEqual => {
                if comparable(left, right) {
                    Bool
                } else {
                    return self.fail(mismatch());
                }
            }
            Op::And | Op::Or | Op::Coalesce => unreachable!("handled in expression"),
        })
    }
}

fn signature(param_types: &[Option<Type>], returns: &Option<Type>) -> Type {
    Type::Function(
        param_types
            .iter()
            .map(|ty| ty.clone().unwrap_or(Type::Any))
            .collect(),
        Box::new(returns.clone().unwrap_or(Type::Any)),
    )
}

fn widen(ty: Type) -> Type {
    match ty {
        Type::None => Type::Any,
        other => other,
    }
}

fn strip(ty: &Type) -> &Type {
    match ty {
        Type::Optional(inner) => inner,
        other => other,
    }
}

fn comparable(left: &Type, right: &Type) -> bool {
    if matches!(left, Type::Any | Type::None) || matches!(right, Type::Any | Type::None) {
        return true;
    }
    match (strip(left), strip(right)) {
        (Type::Function(..), Type::Function(..)) => true,
        (left, right) => left == right,
    }
}

pub fn assignable(actual: &Type, expected: &Type) -> bool {
    match (actual, expected) {
        (Type::Any, _) | (_, Type::Any) => true,
        (Type::None, Type::Optional(_)) => true,
        (Type::Optional(actual), Type::Optional(expected)) => assignable(actual, expected),
        (actual, Type::Optional(expected)) => assignable(actual, expected),
        (Type::Function(actual_params, actual_returns), Type::Function(params, returns)) => {
            actual_params.len() == params.len()
                && actual_params
                    .iter()
                    .zip(params)
                    .all(|(actual, expected)| assignable(expected, actual))
                && assignable(actual_returns, returns)
        }
        (actual, expected) => actual == expected,
    }
}

fn always_returns(block: &Block) -> bool {
    match block.last() {
        Some((_, Stmt::Return(_))) => true,
        Some((_, Stmt::If(_, then_block, Some(else_block)))) => {
            always_returns(then_block) && always_returns(else_block)
        }
        _ => false,
    }
}
