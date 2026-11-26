use std::collections::{HashMap, HashSet};

use crate::error::Error;
use crate::parser::{
    Arm, ArmBody, Block, EnumDecl, Expr, Import, Op, Pattern, Stmt, StructDecl, Type,
};

struct Function {
    name: String,
    returns: Option<Type>,
}

#[derive(Clone)]
struct Method {
    has_self: bool,
    params: Vec<Type>,
    returns: Type,
}

#[derive(Clone, Default)]
struct StructInfo {
    fields: Vec<(String, Type)>,
    methods: HashMap<String, Method>,
}

#[derive(Clone)]
struct Saved {
    globals: HashMap<String, Type>,
    structs: HashMap<String, StructInfo>,
    enums: HashMap<String, Vec<(String, Vec<Type>)>>,
}

pub struct Checker {
    scopes: Vec<HashMap<String, Type>>,
    functions: Vec<Function>,
    generics: Vec<Vec<String>>,
    structs: HashMap<String, StructInfo>,
    enums: HashMap<String, Vec<(String, Vec<Type>)>>,
    line: usize,
}

impl Checker {
    pub fn new() -> Self {
        let globals = HashMap::from([
            ("print".to_string(), Type::Any),
            ("range".to_string(), Type::Any),
            ("assert".to_string(), Type::Any),
            (
                "assert_eq".to_string(),
                Type::Function(vec![Type::Any, Type::Any], Box::new(Type::None)),
            ),
            (
                "len".to_string(),
                Type::Function(vec![Type::Any], Box::new(Type::Number)),
            ),
            (
                "push".to_string(),
                Type::Function(vec![Type::Any, Type::Any], Box::new(Type::None)),
            ),
            (
                "keys".to_string(),
                Type::Function(vec![Type::Any], Box::new(Type::List(Box::new(Type::Str)))),
            ),
        ]);
        Checker {
            scopes: vec![globals],
            functions: Vec::new(),
            generics: Vec::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            line: 1,
        }
    }

    pub fn check(&mut self, program: &Block) -> Result<(), Error> {
        let saved = Saved {
            globals: self.scopes[0].clone(),
            structs: self.structs.clone(),
            enums: self.enums.clone(),
        };
        let result = self.block(program);
        if result.is_err() {
            self.scopes.truncate(1);
            self.scopes[0] = saved.globals;
            self.structs = saved.structs;
            self.enums = saved.enums;
            self.functions.clear();
            self.generics.clear();
        }
        result
    }

    fn fail<T>(&self, message: impl Into<String>) -> Result<T, Error> {
        Err(Error::new(self.line, message))
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

    fn resolve(&self, ty: &Type) -> Result<Type, Error> {
        Ok(match ty {
            Type::Named(name) => {
                if self.generics.iter().any(|params| params.contains(name)) {
                    Type::Param(name.clone())
                } else if self.structs.contains_key(name) {
                    Type::Struct(name.clone())
                } else if self.enums.contains_key(name) {
                    Type::Enum(name.clone())
                } else {
                    return self.fail(format!("unknown type '{name}'"));
                }
            }
            Type::List(inner) => Type::List(Box::new(self.resolve(inner)?)),
            Type::Optional(inner) => Type::Optional(Box::new(self.resolve(inner)?)),
            Type::Function(params, returns) => Type::Function(
                params
                    .iter()
                    .map(|p| self.resolve(p))
                    .collect::<Result<_, _>>()?,
                Box::new(self.resolve(returns)?),
            ),
            other => other.clone(),
        })
    }

    fn resolve_option(&self, ty: &Option<Type>) -> Result<Option<Type>, Error> {
        ty.as_ref().map(|ty| self.resolve(ty)).transpose()
    }

    fn scoped(&mut self, block: &Block) -> Result<(), Error> {
        self.scopes.push(HashMap::new());
        let result = self.block(block);
        self.scopes.pop();
        result
    }

    fn block(&mut self, block: &Block) -> Result<(), Error> {
        for (line, stmt) in block {
            self.line = *line;
            match stmt {
                Stmt::Struct(decl) => {
                    self.structs
                        .insert(decl.name.clone(), StructInfo::default());
                }
                Stmt::Enum(decl) => {
                    self.enums.insert(decl.name.clone(), Vec::new());
                }
                _ => {}
            }
        }
        for (line, stmt) in block {
            self.line = *line;
            match stmt {
                Stmt::Struct(decl) => self.register_struct(decl)?,
                Stmt::Enum(decl) => self.register_enum(decl)?,
                _ => {}
            }
        }
        for (line, stmt) in block {
            if let Stmt::Let(
                name,
                None,
                Expr::Function {
                    type_params,
                    param_types,
                    returns,
                    ..
                },
            ) = stmt
            {
                self.line = *line;
                self.generics.push(type_params.clone());
                let signature = self.signature(param_types, returns);
                self.generics.pop();
                let signature = signature?;
                self.declare(name, signature);
            }
        }
        for (line, stmt) in block {
            self.line = *line;
            self.statement(stmt)?;
        }
        Ok(())
    }

    fn signature(
        &self,
        param_types: &[Option<Type>],
        returns: &Option<Type>,
    ) -> Result<Type, Error> {
        let params = param_types
            .iter()
            .map(|ty| Ok(self.resolve_option(ty)?.unwrap_or(Type::Any)))
            .collect::<Result<Vec<_>, Error>>()?;
        let returns = self.resolve_option(returns)?.unwrap_or(Type::Any);
        Ok(Type::Function(params, Box::new(returns)))
    }

    fn register_struct(&mut self, decl: &StructDecl) -> Result<(), Error> {
        let mut info = StructInfo::default();
        for (field, ty) in &decl.fields {
            info.fields.push((field.clone(), self.resolve(ty)?));
        }
        for (method, function) in &decl.methods {
            let Expr::Function {
                type_params,
                params,
                param_types,
                returns,
                ..
            } = function
            else {
                continue;
            };
            if info.fields.iter().any(|(field, _)| field == method) {
                return self.fail(format!(
                    "{} has both a field and a method called '{method}'",
                    decl.name
                ));
            }
            let has_self = params.first().is_some_and(|p| p == "self");
            self.generics.push(type_params.clone());
            let signature = self.signature(&param_types[usize::from(has_self)..], returns);
            self.generics.pop();
            let Type::Function(params, returns) = signature? else {
                continue;
            };
            info.methods.insert(
                method.clone(),
                Method {
                    has_self,
                    params,
                    returns: *returns,
                },
            );
        }
        self.structs.insert(decl.name.clone(), info);
        self.declare(&decl.name, Type::StructDef(decl.name.clone()));
        Ok(())
    }

    fn register_enum(&mut self, decl: &EnumDecl) -> Result<(), Error> {
        let mut variants = Vec::new();
        for (variant, payload) in &decl.variants {
            let payload = payload
                .iter()
                .map(|ty| self.resolve(ty))
                .collect::<Result<Vec<_>, _>>()?;
            variants.push((variant.clone(), payload));
        }
        self.enums.insert(decl.name.clone(), variants);
        self.declare(&decl.name, Type::EnumDef(decl.name.clone()));
        Ok(())
    }

    fn statement(&mut self, stmt: &Stmt) -> Result<(), Error> {
        match stmt {
            Stmt::Let(name, annotation, expr) => {
                let actual = self.expression(expr)?;
                let ty = match annotation {
                    Some(expected) => {
                        let expected = self.resolve(expected)?;
                        self.expect(&actual, &expected, || {
                            format!("'{name}' should be {expected}, got {actual}")
                        })?;
                        expected
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
                let target = self.expression(target)?;
                self.expression(index)?;
                let value = self.expression(value)?;
                if let Type::List(item) = &target {
                    self.expect(&value, item, || format!("cannot put {value} into {target}"))?;
                }
            }
            Stmt::SetField(target, field, value) => {
                let target = self.expression(target)?;
                let value = self.expression(value)?;
                if let Type::Struct(name) = &target {
                    let info = self.structs.get(name).cloned().unwrap_or_default();
                    match info.fields.iter().find(|(f, _)| f == field) {
                        Some((_, expected)) => {
                            self.expect(&value, expected, || {
                                format!("'{name}.{field}' should be {expected}, got {value}")
                            })?;
                        }
                        None => return self.fail(format!("{name} has no field '{field}'")),
                    }
                }
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
                    Type::List(item) => *item,
                    Type::Any | Type::Param(_) => Type::Any,
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
            Stmt::Struct(decl) => {
                let self_type = Type::Struct(decl.name.clone());
                for (method, function) in &decl.methods {
                    if let Expr::Function {
                        type_params,
                        params,
                        param_types,
                        returns,
                        body,
                        ..
                    } = function
                    {
                        let label = format!("{}.{method}", decl.name);
                        self.function(
                            &label,
                            type_params,
                            params,
                            param_types,
                            returns,
                            body,
                            Some(&self_type),
                        )?;
                    }
                }
            }
            Stmt::Enum(_) | Stmt::Break | Stmt::Continue | Stmt::Test(..) => {}
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
            Type::Bool | Type::Any | Type::Param(_) => Ok(()),
            other => self.fail(format!("{what} needs true or false, got {other}")),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn function(
        &mut self,
        label: &str,
        type_params: &[String],
        params: &[String],
        param_types: &[Option<Type>],
        returns: &Option<Type>,
        body: &Block,
        self_type: Option<&Type>,
    ) -> Result<Type, Error> {
        self.generics.push(type_params.to_vec());
        let result = self.function_inner(label, params, param_types, returns, body, self_type);
        self.generics.pop();
        result
    }

    fn function_inner(
        &mut self,
        label: &str,
        params: &[String],
        param_types: &[Option<Type>],
        returns: &Option<Type>,
        body: &Block,
        self_type: Option<&Type>,
    ) -> Result<Type, Error> {
        let mut scope = HashMap::new();
        let mut signature = Vec::new();
        for (index, (param, ty)) in params.iter().zip(param_types).enumerate() {
            let ty = match (index, param.as_str(), self_type, ty) {
                (0, "self", Some(self_type), None) => self_type.clone(),
                _ => self.resolve_option(ty)?.unwrap_or(Type::Any),
            };
            scope.insert(param.clone(), ty.clone());
            signature.push(ty);
        }
        let returns = self.resolve_option(returns)?;

        self.scopes.push(scope);
        self.functions.push(Function {
            name: label.to_string(),
            returns: returns.clone(),
        });
        let result = self.block(body);
        self.functions.pop();
        self.scopes.pop();
        result?;

        if let Some(expected) = &returns
            && !matches!(expected, Type::Any | Type::None | Type::Optional(_))
            && !always_returns(body)
        {
            return self.fail(format!(
                "'{label}' should return {expected}, but can reach the end without returning"
            ));
        }
        Ok(Type::Function(
            signature,
            Box::new(returns.unwrap_or(Type::Any)),
        ))
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
                let mut element: Option<Type> = None;
                for item in items {
                    let ty = self.expression(item)?;
                    element = match element {
                        None => Some(ty),
                        Some(seen) if seen == ty => Some(seen),
                        Some(_) => Some(Type::Any),
                    };
                }
                Type::List(Box::new(widen(element.unwrap_or(Type::Any))))
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
                let index_ok = matches!(index, Type::Number | Type::Any | Type::Param(_));
                match &target {
                    Type::List(item) if index_ok => (**item).clone(),
                    Type::Str if index_ok => Type::Str,
                    Type::List(_) | Type::Str => {
                        return self.fail(format!("index must be a number, got {index}"));
                    }
                    Type::Record if index == Type::Number => {
                        return self.fail("record fields are named with strings, got number");
                    }
                    Type::Record | Type::Any | Type::Param(_) => Type::Any,
                    Type::Optional(_) => {
                        return self.fail("cannot index into a value that might be none");
                    }
                    other => return self.fail(format!("cannot index into {other}")),
                }
            }
            Expr::Field(target, name) => {
                let target = self.expression(target)?;
                self.field(&target, name)?
            }
            Expr::OptionalField(target, name) => match self.expression(target)? {
                Type::None | Type::Any | Type::Param(_) => Type::Any,
                Type::Optional(inner) => match self.field(&inner, name)? {
                    Type::Optional(inner) => Type::Optional(inner),
                    Type::Any => Type::Any,
                    other => Type::Optional(Box::new(other)),
                },
                other => match self.field(&other, name)? {
                    Type::Any => Type::Any,
                    found => found,
                },
            },
            Expr::Negate(inner) => match self.expression(inner)? {
                Type::Number | Type::Any | Type::Param(_) => Type::Number,
                other => return self.fail(format!("'-' needs a number, got {other}")),
            },
            Expr::Not(inner) => match self.expression(inner)? {
                Type::Bool | Type::Any | Type::Param(_) => Type::Bool,
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
                        Type::Bool | Type::Any | Type::Param(_) => {}
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
                self.binary(*op, &loosen(left), &loosen(right))?
            }
            Expr::Function {
                name,
                type_params,
                params,
                param_types,
                returns,
                body,
            } => {
                let label = if name.is_empty() {
                    "this function"
                } else {
                    name.as_str()
                };
                self.function(label, type_params, params, param_types, returns, body, None)?
            }
            Expr::Call(callee, args) => self.call(callee, args)?,
            Expr::StructLit(name, fields) => self.struct_literal(name, fields)?,
            Expr::Match(scrutinee, arms) => self.match_expr(scrutinee, arms)?,
        })
    }

    fn field(&self, target: &Type, name: &str) -> Result<Type, Error> {
        Ok(match target {
            Type::Record | Type::Any | Type::Param(_) => Type::Any,
            Type::Struct(owner) => {
                let info = self.structs.get(owner).cloned().unwrap_or_default();
                if let Some((_, ty)) = info.fields.iter().find(|(field, _)| field == name) {
                    ty.clone()
                } else if info.methods.contains_key(name) {
                    return self.fail(format!(
                        "'{name}' is a method of {owner}, call it like value.{name}(...)"
                    ));
                } else {
                    return self.fail(format!("{owner} has no field '{name}'"));
                }
            }
            Type::StructDef(owner) => {
                let info = self.structs.get(owner).cloned().unwrap_or_default();
                match info.methods.get(name) {
                    Some(method) if !method.has_self => {
                        Type::Function(method.params.clone(), Box::new(method.returns.clone()))
                    }
                    Some(_) => {
                        return self.fail(format!(
                            "{owner}.{name} needs a {owner}, call it on a value instead"
                        ));
                    }
                    None => return self.fail(format!("{owner} has no method '{name}'")),
                }
            }
            Type::EnumDef(owner) => match self.variant(owner, name)? {
                payload if payload.is_empty() => Type::Enum(owner.clone()),
                payload => {
                    return self.fail(format!(
                        "{owner}.{name} holds {} value{}, write {owner}.{name}(...)",
                        payload.len(),
                        if payload.len() == 1 { "" } else { "s" }
                    ));
                }
            },
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
        })
    }

    fn variant(&self, owner: &str, name: &str) -> Result<Vec<Type>, Error> {
        match self
            .enums
            .get(owner)
            .and_then(|variants| variants.iter().find(|(variant, _)| variant == name))
        {
            Some((_, payload)) => Ok(payload.clone()),
            None => self.fail(format!("{owner} has no variant '{name}'")),
        }
    }

    fn call(&mut self, callee: &Expr, args: &[Expr]) -> Result<Type, Error> {
        if let Expr::Field(target, method) = callee {
            let target_type = self.expression(target)?;
            let label = match &**target {
                Expr::Variable(name) => format!("{name}.{method}"),
                _ => method.clone(),
            };
            match &target_type {
                Type::Struct(owner) => {
                    let info = self.structs.get(owner).cloned().unwrap_or_default();
                    if let Some(found) = info.methods.get(method) {
                        if !found.has_self {
                            return self.fail(format!(
                                "{owner}.{method} has no 'self', call it as {owner}.{method}(...)"
                            ));
                        }
                        return self.check_args(&label, &found.params, &found.returns, args);
                    }
                    let field = self.field(&target_type, method)?;
                    return self.call_value(&label, field, args);
                }
                Type::EnumDef(owner) => {
                    let payload = self.variant(owner, method)?;
                    self.check_args(&format!("{owner}.{method}"), &payload, &Type::None, args)?;
                    return Ok(Type::Enum(owner.clone()));
                }
                _ => {
                    let field = self.field(&target_type, method)?;
                    return self.call_value(&label, field, args);
                }
            }
        }

        let callee_type = self.expression(callee)?;
        let name = match callee {
            Expr::Variable(name) => name.clone(),
            _ => "this function".to_string(),
        };
        if let Expr::Variable(builtin) = callee
            && matches!(builtin.as_str(), "range" | "push")
            && self
                .scopes
                .iter()
                .skip(1)
                .all(|scope| !scope.contains_key(builtin))
        {
            let arg_types = args
                .iter()
                .map(|arg| self.expression(arg))
                .collect::<Result<Vec<_>, _>>()?;
            if builtin == "range" {
                return Ok(Type::List(Box::new(Type::Number)));
            }
            if let [Type::List(item), value] = &arg_types[..] {
                self.expect(value, item, || {
                    format!("cannot push {value} into list<{item}>")
                })?;
            }
            if arg_types.len() != 2 {
                return self.fail(format!("push expects 2 arguments, got {}", arg_types.len()));
            }
            return Ok(Type::None);
        }
        self.call_value(&name, callee_type, args)
    }

    fn call_value(&mut self, name: &str, callee: Type, args: &[Expr]) -> Result<Type, Error> {
        match callee {
            Type::Function(params, returns) => self.check_args(name, &params, &returns, args),
            Type::Any | Type::Param(_) => {
                for arg in args {
                    self.expression(arg)?;
                }
                Ok(Type::Any)
            }
            other => self.fail(format!("{other} is not a function")),
        }
    }

    fn check_args(
        &mut self,
        name: &str,
        params: &[Type],
        returns: &Type,
        args: &[Expr],
    ) -> Result<Type, Error> {
        if params.len() != args.len() {
            return self.fail(format!(
                "{name} expects {} argument{}, got {}",
                params.len(),
                if params.len() == 1 { "" } else { "s" },
                args.len()
            ));
        }
        let mut bindings = HashMap::new();
        for (index, (arg, param)) in args.iter().zip(params).enumerate() {
            let actual = self.expression(arg)?;
            if !unify(param, &actual, &mut bindings) {
                let expected = substitute(param, &bindings);
                let mut message = format!(
                    "argument {} of '{name}' should be {expected}, got {actual}",
                    index + 1
                );
                if let Type::Optional(inner) = &actual
                    && assignable(inner, &expected)
                {
                    message.push_str(" (it might be none, use ?? to give a fallback)");
                }
                return self.fail(message);
            }
        }
        Ok(substitute(returns, &bindings))
    }

    fn struct_literal(&mut self, name: &str, fields: &[(String, Expr)]) -> Result<Type, Error> {
        let Some(info) = self.structs.get(name).cloned() else {
            for (_, value) in fields {
                self.expression(value)?;
            }
            return Ok(Type::Any);
        };
        for (field, value) in fields {
            let actual = self.expression(value)?;
            match info.fields.iter().find(|(f, _)| f == field) {
                Some((_, expected)) => {
                    self.expect(&actual, expected, || {
                        format!("'{name}.{field}' should be {expected}, got {actual}")
                    })?;
                }
                None => return self.fail(format!("{name} has no field '{field}'")),
            }
        }
        for (field, _) in &info.fields {
            if !fields.iter().any(|(given, _)| given == field) {
                return self.fail(format!("{name} is missing field '{field}'"));
            }
        }
        Ok(Type::Struct(name.to_string()))
    }

    fn match_expr(&mut self, scrutinee: &Expr, arms: &[Arm]) -> Result<Type, Error> {
        let subject = self.expression(scrutinee)?;
        let start_line = self.line;
        let mut result: Option<Type> = None;
        let mut covered: HashSet<String> = HashSet::new();
        let mut catch_all = false;

        for arm in arms {
            self.line = arm.line;
            let mut scope = HashMap::new();
            match &arm.pattern {
                Pattern::Wildcard => catch_all = true,
                Pattern::Bind(name) => {
                    catch_all = true;
                    scope.insert(name.clone(), subject.clone());
                }
                Pattern::Literal(literal) => {
                    let literal_type = self.expression(literal)?;
                    if !comparable(&subject, &literal_type) {
                        return self.fail(format!(
                            "this match is on {subject}, but the pattern is a {literal_type}"
                        ));
                    }
                    if let Expr::Bool(value) = literal {
                        covered.insert(value.to_string());
                    }
                }
                Pattern::Variant {
                    enum_name,
                    variant,
                    bindings,
                } => {
                    let owner = match (&subject, enum_name) {
                        (Type::Enum(subject_enum), Some(named)) if subject_enum != named => {
                            return self.fail(format!(
                                "this match is on {subject_enum}, but the pattern is {named}.{variant}"
                            ));
                        }
                        (Type::Enum(subject_enum), _) => Some(subject_enum.clone()),
                        (Type::Any | Type::Param(_), Some(named)) => Some(named.clone()),
                        (Type::Any | Type::Param(_), None) => None,
                        (other, _) => {
                            return self.fail(format!(
                                "this match is on {other}, which has no variants like {variant}"
                            ));
                        }
                    };
                    match owner {
                        Some(owner) => {
                            let payload = self.variant(&owner, variant)?;
                            if payload.len() != bindings.len() {
                                return self.fail(format!(
                                    "{owner}.{variant} holds {} value{}, but the pattern names {}",
                                    payload.len(),
                                    if payload.len() == 1 { "" } else { "s" },
                                    bindings.len()
                                ));
                            }
                            for (binding, ty) in bindings.iter().zip(payload) {
                                scope.insert(binding.clone(), ty);
                            }
                            covered.insert(variant.clone());
                        }
                        None => {
                            for binding in bindings {
                                scope.insert(binding.clone(), Type::Any);
                            }
                        }
                    }
                }
            }

            self.scopes.push(scope);
            let arm_type = match &arm.body {
                ArmBody::Expr(expr) => self.expression(expr),
                ArmBody::Block(block) => self.block(block).map(|_| Type::None),
            };
            self.scopes.pop();
            let arm_type = arm_type?;
            result = Some(match result {
                None => arm_type,
                Some(seen) if seen == arm_type => seen,
                Some(_) => Type::Any,
            });
        }

        self.line = start_line;
        if !catch_all {
            match &subject {
                Type::Enum(owner) => {
                    let missing: Vec<String> = self
                        .enums
                        .get(owner)
                        .map(|variants| {
                            variants
                                .iter()
                                .filter(|(variant, _)| !covered.contains(variant))
                                .map(|(variant, _)| format!("{owner}.{variant}"))
                                .collect()
                        })
                        .unwrap_or_default();
                    if !missing.is_empty() {
                        return self.fail(format!(
                            "match on {owner} is missing {}",
                            missing.join(", ")
                        ));
                    }
                }
                Type::Bool => {
                    if !(covered.contains("true") && covered.contains("false")) {
                        return self.fail("match on bool needs both true and false, or a '_' arm");
                    }
                }
                Type::Any | Type::Param(_) => {}
                other => {
                    return self.fail(format!(
                        "match on {other} needs a '_' arm for the other values"
                    ));
                }
            }
        }
        Ok(result.unwrap_or(Type::None))
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

fn widen(ty: Type) -> Type {
    match ty {
        Type::None => Type::Any,
        other => other,
    }
}

fn loosen(ty: Type) -> Type {
    match ty {
        Type::Param(_) => Type::Any,
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
    let loose = |ty: &Type| matches!(ty, Type::Any | Type::None | Type::Param(_));
    if loose(left) || loose(right) {
        return true;
    }
    match (strip(left), strip(right)) {
        (Type::Function(..), Type::Function(..)) => true,
        (Type::List(_), Type::List(_)) => true,
        (left, right) => left == right,
    }
}

pub fn assignable(actual: &Type, expected: &Type) -> bool {
    match (actual, expected) {
        (Type::Any | Type::Param(_), _) | (_, Type::Any | Type::Param(_)) => true,
        (Type::None, Type::Optional(_)) => true,
        (Type::Optional(actual), Type::Optional(expected)) => assignable(actual, expected),
        (actual, Type::Optional(expected)) => assignable(actual, expected),
        (Type::List(actual), Type::List(expected)) => {
            assignable(actual, expected) && assignable(expected, actual)
        }
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

fn unify(param: &Type, actual: &Type, bindings: &mut HashMap<String, Type>) -> bool {
    match (param, actual) {
        (Type::Param(name), actual) => match bindings.get(name) {
            Some(bound) => {
                let bound = bound.clone();
                assignable(actual, &bound)
            }
            None => {
                bindings.insert(name.clone(), actual.clone());
                true
            }
        },
        (Type::List(param), Type::List(actual)) => unify(param, actual, bindings),
        (Type::Optional(_), Type::None) => true,
        (Type::Optional(param), Type::Optional(actual)) => unify(param, actual, bindings),
        (Type::Optional(param), actual) => unify(param, actual, bindings),
        (Type::Function(params, returns), Type::Function(actual_params, actual_returns)) => {
            params.len() == actual_params.len()
                && params
                    .iter()
                    .zip(actual_params)
                    .all(|(param, actual)| unify(param, actual, bindings))
                && unify(returns, actual_returns, bindings)
        }
        (param, actual) => assignable(actual, param),
    }
}

fn substitute(ty: &Type, bindings: &HashMap<String, Type>) -> Type {
    match ty {
        Type::Param(name) => bindings.get(name).cloned().unwrap_or(Type::Any),
        Type::List(inner) => Type::List(Box::new(substitute(inner, bindings))),
        Type::Optional(inner) => Type::Optional(Box::new(substitute(inner, bindings))),
        Type::Function(params, returns) => Type::Function(
            params.iter().map(|p| substitute(p, bindings)).collect(),
            Box::new(substitute(returns, bindings)),
        ),
        other => other.clone(),
    }
}

fn always_returns(block: &Block) -> bool {
    match block.last() {
        Some((_, Stmt::Return(_))) => true,
        Some((_, Stmt::If(_, then_block, Some(else_block)))) => {
            always_returns(then_block) && always_returns(else_block)
        }
        Some((_, Stmt::Expr(Expr::Match(_, arms)))) => arms.iter().all(|arm| match &arm.body {
            ArmBody::Block(block) => always_returns(block),
            ArmBody::Expr(_) => false,
        }),
        _ => false,
    }
}
