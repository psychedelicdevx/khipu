use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::error::Error;
use crate::parser::{Block, Expr, Op, Stmt};

const MAX_CALL_DEPTH: usize = 1000;

type NativeFn = fn(&[Value]) -> Result<Value, String>;
type Fields = Vec<(String, Value)>;

#[derive(Clone)]
pub enum Value {
    Number(f64),
    Bool(bool),
    Str(String),
    List(Rc<RefCell<Vec<Value>>>),
    Record(Rc<RefCell<Fields>>),
    Function(Rc<Function>),
    Native(&'static str, NativeFn),
    None,
}

pub struct Function {
    name: String,
    params: Vec<String>,
    body: Rc<Block>,
    closure: Env,
}

type Env = Rc<RefCell<Scope>>;

struct Scope {
    vars: HashMap<String, Value>,
    parent: Option<Env>,
}

impl Scope {
    fn new(parent: Option<Env>) -> Env {
        Rc::new(RefCell::new(Scope {
            vars: HashMap::new(),
            parent,
        }))
    }
}

enum Exit {
    Error(Error),
    Return(Value),
    Break,
    Continue,
}

impl Value {
    fn type_name(&self) -> &'static str {
        match self {
            Value::Number(_) => "number",
            Value::Bool(_) => "bool",
            Value::Str(_) => "string",
            Value::List(_) => "list",
            Value::Record(_) => "record",
            Value::Function(_) | Value::Native(..) => "function",
            Value::None => "none",
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{n}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Str(s) => write!(f, "{s}"),
            Value::List(items) => {
                let items: Vec<String> = items
                    .borrow()
                    .iter()
                    .map(|item| format!("{item:?}"))
                    .collect();
                write!(f, "[{}]", items.join(", "))
            }
            Value::Record(fields) => {
                let fields: Vec<String> = fields
                    .borrow()
                    .iter()
                    .map(|(name, value)| format!("{name}: {value:?}"))
                    .collect();
                write!(f, "{{{}}}", fields.join(", "))
            }
            Value::Function(function) => write!(f, "<fn {}>", function_label(function)),
            Value::Native(name, _) => write!(f, "<fn {name}>"),
            Value::None => write!(f, "none"),
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Str(s) => write!(f, "{s:?}"),
            other => write!(f, "{other}"),
        }
    }
}

fn function_label(function: &Function) -> &str {
    if function.name.is_empty() {
        "anonymous"
    } else {
        &function.name
    }
}

pub struct Interpreter {
    env: Env,
    line: usize,
    depth: usize,
}

impl Interpreter {
    pub fn new() -> Self {
        let globals = Scope::new(None);
        for (name, native) in [
            ("print", print as NativeFn),
            ("len", len),
            ("push", push),
            ("range", range),
            ("keys", keys),
        ] {
            globals
                .borrow_mut()
                .vars
                .insert(name.to_string(), Value::Native(name, native));
        }
        Interpreter {
            env: globals,
            line: 1,
            depth: 0,
        }
    }

    pub fn run(&mut self, program: &Block) -> Result<Option<Value>, Error> {
        match self.run_statements(program) {
            Ok(value) => Ok(value),
            Err(Exit::Error(error)) => Err(error),
            Err(Exit::Return(_)) => Err(self.error("'return' can only be used inside a function")),
            Err(Exit::Break) | Err(Exit::Continue) => {
                Err(self.error("'break' and 'continue' can only be used inside a loop"))
            }
        }
    }

    fn run_statements(&mut self, block: &Block) -> Result<Option<Value>, Exit> {
        let mut last = None;
        for (line, stmt) in block {
            self.line = *line;
            last = self.execute(stmt)?;
        }
        Ok(last)
    }

    fn run_in(&mut self, env: Env, block: &Block) -> Result<Option<Value>, Exit> {
        let previous = std::mem::replace(&mut self.env, env);
        let line = self.line;
        let result = self.run_statements(block);
        self.env = previous;
        self.line = line;
        result
    }

    fn run_block(&mut self, block: &Block) -> Result<(), Exit> {
        let env = Scope::new(Some(self.env.clone()));
        self.run_in(env, block).map(|_| ())
    }

    fn execute(&mut self, stmt: &Stmt) -> Result<Option<Value>, Exit> {
        match stmt {
            Stmt::Let(name, expr) => {
                let value = self.eval(expr)?;
                self.env.borrow_mut().vars.insert(name.clone(), value);
                Ok(None)
            }
            Stmt::Assign(name, expr) => {
                let value = self.eval(expr)?;
                self.assign(name, value)?;
                Ok(None)
            }
            Stmt::If(condition, then_block, else_block) => {
                if self.eval_bool(condition, "if")? {
                    self.run_block(then_block)?;
                } else if let Some(block) = else_block {
                    self.run_block(block)?;
                }
                Ok(None)
            }
            Stmt::SetIndex(target, index, value) => {
                let target = self.eval(target)?;
                let index = self.eval(index)?;
                let value = self.eval(value)?;
                match target {
                    Value::List(items) => {
                        let position = self.position(&index, items.borrow().len())?;
                        items.borrow_mut()[position] = value;
                        Ok(None)
                    }
                    Value::Record(fields) => match index {
                        Value::Str(name) => self.set_field(&fields, &name, value),
                        other => Err(self.fail(format!(
                            "record fields are named with strings, got {}",
                            other.type_name()
                        ))),
                    },
                    Value::Str(_) => Err(self.fail("strings can't be changed in place")),
                    other => Err(self.fail(format!("cannot index into {}", other.type_name()))),
                }
            }
            Stmt::SetField(target, name, value) => {
                let target = self.eval(target)?;
                let value = self.eval(value)?;
                match target {
                    Value::Record(fields) => self.set_field(&fields, name, value),
                    other => Err(self.fail(format!(
                        "cannot set field '{name}' on {}",
                        other.type_name()
                    ))),
                }
            }
            Stmt::While(condition, body) => {
                while self.eval_bool(condition, "while")? {
                    match self.run_block(body) {
                        Ok(()) | Err(Exit::Continue) => {}
                        Err(Exit::Break) => break,
                        Err(other) => return Err(other),
                    }
                }
                Ok(None)
            }
            Stmt::For(name, iterable, body) => {
                let items = match self.eval(iterable)? {
                    Value::List(items) => items.borrow().clone(),
                    Value::Str(s) => s.chars().map(|c| Value::Str(c.to_string())).collect(),
                    Value::Record(_) => {
                        return Err(self.fail(
                            "for needs a list or a string, got record, try 'for key in keys(record)'",
                        ));
                    }
                    other => {
                        return Err(self.fail(format!(
                            "for needs a list or a string, got {}",
                            other.type_name()
                        )));
                    }
                };
                for item in items {
                    let env = Scope::new(Some(self.env.clone()));
                    env.borrow_mut().vars.insert(name.clone(), item);
                    match self.run_in(env, body) {
                        Ok(_) | Err(Exit::Continue) => {}
                        Err(Exit::Break) => break,
                        Err(other) => return Err(other),
                    }
                }
                Ok(None)
            }
            Stmt::Break => Err(Exit::Break),
            Stmt::Continue => Err(Exit::Continue),
            Stmt::Return(expr) => {
                let value = match expr {
                    Some(expr) => self.eval(expr)?,
                    None => Value::None,
                };
                Err(Exit::Return(value))
            }
            Stmt::Expr(expr) => Ok(Some(self.eval(expr)?)),
        }
    }

    fn lookup(&self, name: &str) -> Option<Value> {
        let mut scope = Some(self.env.clone());
        while let Some(env) = scope {
            if let Some(value) = env.borrow().vars.get(name) {
                return Some(value.clone());
            }
            scope = env.borrow().parent.clone();
        }
        None
    }

    fn assign(&mut self, name: &str, value: Value) -> Result<(), Exit> {
        let mut scope = Some(self.env.clone());
        while let Some(env) = scope {
            if let Some(slot) = env.borrow_mut().vars.get_mut(name) {
                *slot = value;
                return Ok(());
            }
            scope = env.borrow().parent.clone();
        }
        Err(self.fail(format!(
            "'{name}' is not defined, use 'let {name} = ...' first"
        )))
    }

    fn eval(&mut self, expr: &Expr) -> Result<Value, Exit> {
        match expr {
            Expr::Number(n) => Ok(Value::Number(*n)),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Str(s) => Ok(Value::Str(s.clone())),
            Expr::Template(parts) => {
                let mut text = String::new();
                for part in parts {
                    text.push_str(&self.eval(part)?.to_string());
                }
                Ok(Value::Str(text))
            }
            Expr::List(items) => {
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    values.push(self.eval(item)?);
                }
                Ok(Value::List(Rc::new(RefCell::new(values))))
            }
            Expr::Index(target, index) => {
                let target = self.eval(target)?;
                let index = self.eval(index)?;
                match target {
                    Value::List(items) => {
                        let items = items.borrow();
                        let position = self.position(&index, items.len())?;
                        Ok(items[position].clone())
                    }
                    Value::Str(s) => {
                        let chars: Vec<char> = s.chars().collect();
                        let position = self.position(&index, chars.len())?;
                        Ok(Value::Str(chars[position].to_string()))
                    }
                    Value::Record(fields) => match index {
                        Value::Str(name) => self.get_field(&fields, &name),
                        other => Err(self.fail(format!(
                            "record fields are named with strings, got {}",
                            other.type_name()
                        ))),
                    },
                    other => Err(self.fail(format!("cannot index into {}", other.type_name()))),
                }
            }
            Expr::Record(fields) => {
                let mut values = Vec::with_capacity(fields.len());
                for (name, expr) in fields {
                    values.push((name.clone(), self.eval(expr)?));
                }
                Ok(Value::Record(Rc::new(RefCell::new(values))))
            }
            Expr::Field(target, name) => match self.eval(target)? {
                Value::Record(fields) => self.get_field(&fields, name),
                other => Err(self.fail(format!(
                    "cannot read field '{name}' of {}",
                    other.type_name()
                ))),
            },
            Expr::Variable(name) => match self.lookup(name) {
                Some(value) => Ok(value),
                None => Err(self.fail(format!("'{name}' is not defined"))),
            },
            Expr::Negate(inner) => match self.eval(inner)? {
                Value::Number(n) => Ok(Value::Number(-n)),
                other => Err(self.fail(format!("'-' needs a number, got {}", other.type_name()))),
            },
            Expr::Not(inner) => Ok(Value::Bool(!self.eval_bool(inner, "'!'")?)),
            Expr::Binary(left, Op::And, right) => Ok(Value::Bool(
                self.eval_bool(left, "'&&'")? && self.eval_bool(right, "'&&'")?,
            )),
            Expr::Binary(left, Op::Or, right) => Ok(Value::Bool(
                self.eval_bool(left, "'||'")? || self.eval_bool(right, "'||'")?,
            )),
            Expr::Binary(left, op, right) => {
                let a = self.eval(left)?;
                let b = self.eval(right)?;
                binary(*op, a, b).map_err(|message| self.fail(message))
            }
            Expr::Function { name, params, body } => Ok(Value::Function(Rc::new(Function {
                name: name.clone(),
                params: params.clone(),
                body: Rc::clone(body),
                closure: self.env.clone(),
            }))),
            Expr::Call(callee, args) => {
                let callee = self.eval(callee)?;
                let mut values = Vec::with_capacity(args.len());
                for arg in args {
                    values.push(self.eval(arg)?);
                }
                self.call(callee, values)
            }
        }
    }

    fn call(&mut self, callee: Value, args: Vec<Value>) -> Result<Value, Exit> {
        let function = match callee {
            Value::Function(function) => function,
            Value::Native(_, native) => {
                return native(&args).map_err(|message| self.fail(message));
            }
            other => {
                return Err(self.fail(format!("{} is not a function", other.type_name())));
            }
        };

        if args.len() != function.params.len() {
            let expected = function.params.len();
            return Err(self.fail(format!(
                "{} expects {expected} argument{}, got {}",
                function_label(&function),
                if expected == 1 { "" } else { "s" },
                args.len()
            )));
        }
        if self.depth >= MAX_CALL_DEPTH {
            return Err(self.fail(format!(
                "too much recursion: more than {MAX_CALL_DEPTH} calls deep"
            )));
        }

        let env = Scope::new(Some(function.closure.clone()));
        for (param, arg) in function.params.iter().zip(args) {
            env.borrow_mut().vars.insert(param.clone(), arg);
        }

        self.depth += 1;
        let result = self.run_in(env, &function.body);
        self.depth -= 1;

        match result {
            Ok(_) => Ok(Value::None),
            Err(Exit::Return(value)) => Ok(value),
            Err(error) => Err(error),
        }
    }

    fn get_field(&self, fields: &RefCell<Fields>, name: &str) -> Result<Value, Exit> {
        match fields.borrow().iter().find(|(field, _)| field == name) {
            Some((_, value)) => Ok(value.clone()),
            None => Err(self.fail(format!("record has no field '{name}'"))),
        }
    }

    fn set_field(
        &self,
        fields: &RefCell<Fields>,
        name: &str,
        value: Value,
    ) -> Result<Option<Value>, Exit> {
        match fields
            .borrow_mut()
            .iter_mut()
            .find(|(field, _)| field == name)
        {
            Some((_, slot)) => {
                *slot = value;
                Ok(None)
            }
            None => Err(self.fail(format!(
                "record has no field '{name}', fields can only be added when the record is created"
            ))),
        }
    }

    fn position(&self, index: &Value, len: usize) -> Result<usize, Exit> {
        match index {
            Value::Number(n) if n.fract() != 0.0 => {
                Err(self.fail(format!("index must be a whole number, got {n}")))
            }
            Value::Number(n) if *n >= 0.0 && (*n as usize) < len => Ok(*n as usize),
            Value::Number(n) => {
                Err(self.fail(format!("index {n} is out of range, the length is {len}")))
            }
            other => Err(self.fail(format!("index must be a number, got {}", other.type_name()))),
        }
    }

    fn eval_bool(&mut self, expr: &Expr, what: &str) -> Result<bool, Exit> {
        match self.eval(expr)? {
            Value::Bool(b) => Ok(b),
            other => Err(self.fail(format!(
                "{what} needs true or false, got {}",
                other.type_name()
            ))),
        }
    }

    fn error(&self, message: impl Into<String>) -> Error {
        Error {
            line: self.line,
            message: message.into(),
        }
    }

    fn fail(&self, message: impl Into<String>) -> Exit {
        Exit::Error(self.error(message))
    }
}

fn print(args: &[Value]) -> Result<Value, String> {
    let parts: Vec<String> = args.iter().map(Value::to_string).collect();
    println!("{}", parts.join(" "));
    Ok(Value::None)
}

fn arguments<'a, const N: usize>(name: &str, args: &'a [Value]) -> Result<&'a [Value; N], String> {
    args.try_into().map_err(|_| {
        format!(
            "{name} expects {N} argument{}, got {}",
            if N == 1 { "" } else { "s" },
            args.len()
        )
    })
}

fn len(args: &[Value]) -> Result<Value, String> {
    match arguments::<1>("len", args)? {
        [Value::List(items)] => Ok(Value::Number(items.borrow().len() as f64)),
        [Value::Str(s)] => Ok(Value::Number(s.chars().count() as f64)),
        [Value::Record(fields)] => Ok(Value::Number(fields.borrow().len() as f64)),
        [other] => Err(format!(
            "len needs a list, a string or a record, got {}",
            other.type_name()
        )),
    }
}

fn push(args: &[Value]) -> Result<Value, String> {
    match arguments::<2>("push", args)? {
        [Value::List(items), value] => {
            items.borrow_mut().push(value.clone());
            Ok(Value::None)
        }
        [other, _] => Err(format!("push needs a list, got {}", other.type_name())),
    }
}

fn range(args: &[Value]) -> Result<Value, String> {
    let (start, end) = match args {
        [Value::Number(end)] => (0.0, *end),
        [Value::Number(start), Value::Number(end)] => (*start, *end),
        _ => {
            return Err(
                "range expects one or two numbers: range(end) or range(start, end)".to_string(),
            );
        }
    };
    if start.fract() != 0.0 || end.fract() != 0.0 {
        return Err("range needs whole numbers".to_string());
    }
    let items = (start as i64..end as i64)
        .map(|n| Value::Number(n as f64))
        .collect();
    Ok(Value::List(Rc::new(RefCell::new(items))))
}

fn keys(args: &[Value]) -> Result<Value, String> {
    match arguments::<1>("keys", args)? {
        [Value::Record(fields)] => {
            let names = fields
                .borrow()
                .iter()
                .map(|(name, _)| Value::Str(name.clone()))
                .collect();
            Ok(Value::List(Rc::new(RefCell::new(names))))
        }
        [other] => Err(format!("keys needs a record, got {}", other.type_name())),
    }
}

fn equal(a: &Value, b: &Value) -> Option<bool> {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => Some(x == y),
        (Value::Bool(x), Value::Bool(y)) => Some(x == y),
        (Value::Str(x), Value::Str(y)) => Some(x == y),
        (Value::List(x), Value::List(y)) => {
            let (x, y) = (x.borrow(), y.borrow());
            Some(
                x.len() == y.len()
                    && x.iter()
                        .zip(y.iter())
                        .all(|(a, b)| equal(a, b).unwrap_or(false)),
            )
        }
        (Value::Record(x), Value::Record(y)) => {
            let (x, y) = (x.borrow(), y.borrow());
            Some(
                x.len() == y.len()
                    && x.iter().all(|(name, a)| {
                        y.iter()
                            .find(|(other, _)| other == name)
                            .is_some_and(|(_, b)| equal(a, b).unwrap_or(false))
                    }),
            )
        }
        (Value::None, Value::None) => Some(true),
        _ => None,
    }
}

fn binary(op: Op, a: Value, b: Value) -> Result<Value, String> {
    use Value::{Bool, Number, Str};

    if let Op::Equal | Op::NotEqual = op {
        return match equal(&a, &b) {
            Some(same) => Ok(Bool(same == matches!(op, Op::Equal))),
            None => Err(format!(
                "cannot use '{}' on {} and {}",
                symbol(op),
                a.type_name(),
                b.type_name()
            )),
        };
    }

    match (op, a, b) {
        (Op::Add, Number(x), Number(y)) => Ok(Number(x + y)),
        (Op::Add, Str(x), Str(y)) => Ok(Str(x + &y)),
        (Op::Add, a @ Str(_), b) | (Op::Add, a, b @ Str(_)) => Err(format!(
            "cannot use '+' on {} and {}, use \"${{...}}\" to put values inside a string",
            a.type_name(),
            b.type_name()
        )),
        (Op::Sub, Number(x), Number(y)) => Ok(Number(x - y)),
        (Op::Mul, Number(x), Number(y)) => Ok(Number(x * y)),
        (Op::Div, Number(x), Number(y)) => Ok(Number(x / y)),
        (Op::Rem, Number(x), Number(y)) => Ok(Number(x % y)),
        (Op::Less, Number(x), Number(y)) => Ok(Bool(x < y)),
        (Op::LessEqual, Number(x), Number(y)) => Ok(Bool(x <= y)),
        (Op::Greater, Number(x), Number(y)) => Ok(Bool(x > y)),
        (Op::GreaterEqual, Number(x), Number(y)) => Ok(Bool(x >= y)),
        (op, a, b) => Err(format!(
            "cannot use '{}' on {} and {}",
            symbol(op),
            a.type_name(),
            b.type_name()
        )),
    }
}

fn symbol(op: Op) -> &'static str {
    match op {
        Op::Add => "+",
        Op::Sub => "-",
        Op::Mul => "*",
        Op::Div => "/",
        Op::Rem => "%",
        Op::Equal => "==",
        Op::NotEqual => "!=",
        Op::Less => "<",
        Op::LessEqual => "<=",
        Op::Greater => ">",
        Op::GreaterEqual => ">=",
        Op::And => "&&",
        Op::Or => "||",
    }
}

#[cfg(test)]
mod tests {
    use super::{Interpreter, Value};
    use crate::error::Error;
    use crate::{lexer::lex, parser::Parser};

    fn run(source: &str) -> Result<Option<Value>, Error> {
        let program = Parser::new(lex(source)?).parse_program()?;
        Interpreter::new().run(&program)
    }

    fn number(source: &str) -> f64 {
        match run(source).unwrap() {
            Some(Value::Number(n)) => n,
            other => panic!("expected a number, got {other:?}"),
        }
    }

    fn boolean(source: &str) -> bool {
        match run(source).unwrap() {
            Some(Value::Bool(b)) => b,
            other => panic!("expected a bool, got {other:?}"),
        }
    }

    fn error(source: &str) -> Error {
        run(source).unwrap_err()
    }

    #[test]
    fn respects_precedence_and_parentheses() {
        assert_eq!(number("2 + 3 * 4"), 14.0);
        assert_eq!(number("(2 + 3) * 4"), 20.0);
        assert_eq!(number("8 / 2 / 2"), 2.0);
        assert_eq!(number("-2 * 3"), -6.0);
        assert_eq!(number("17 % 5"), 2.0);
    }

    #[test]
    fn runs_a_multi_line_program() {
        let source = "// set up\nlet x = 2 + 3\n\nlet y = x * 4 // twenty\ny";
        assert_eq!(number(source), 20.0);
    }

    #[test]
    fn reports_the_line_of_an_error() {
        let error = error("let a = 1\n\nprint(b)");
        assert_eq!(error.line, 3);
        assert_eq!(error.message, "'b' is not defined");
    }

    #[test]
    fn rejects_two_statements_on_one_line() {
        assert_eq!(error("let x = 1\n2 3").line, 2);
    }

    #[test]
    fn compares_and_combines() {
        assert!(boolean("1 + 2 * 3 == 7 && !false"));
        assert!(boolean("3 >= 3 || nope"));
        assert!(!boolean("false && nope"));
        assert!(boolean("2 != 3"));
    }

    #[test]
    fn picks_the_right_branch() {
        let source = "let x = 5\nlet y = 0\nif x > 3 {\n  y = 1\n} else if x > 1 {\n  y = 2\n} else {\n  y = 3\n}\ny";
        assert_eq!(number(source), 1.0);
        assert_eq!(number(&source.replace("x = 5", "x = 2")), 2.0);
        assert_eq!(number(&source.replace("x = 5", "x = 0")), 3.0);
    }

    #[test]
    fn loops_with_while() {
        let source =
            "let i = 1\nlet total = 0\nwhile i <= 5 {\n  total = total + i\n  i = i + 1\n}\ntotal";
        assert_eq!(number(source), 15.0);
    }

    #[test]
    fn keeps_block_variables_inside_the_block() {
        assert_eq!(number("let x = 1\nif true {\n  let x = 2\n}\nx"), 1.0);
        assert_eq!(error("if true { let y = 2 }\ny").line, 2);
    }

    #[test]
    fn has_no_truthiness_or_coercion() {
        assert!(error("if 1 { print(1) }").message.contains("true or false"));
        assert!(error("1 + true").message.contains("cannot use '+'"));
        assert!(error("1 == true").message.contains("cannot use '=='"));
        assert!(error("x = 5").message.contains("not defined"));
    }

    #[test]
    fn reports_errors_inside_blocks_on_their_own_line() {
        assert_eq!(error("let a = 1\nif true {\n  print(nope)\n}").line, 3);
    }

    fn string(source: &str) -> String {
        match run(source).unwrap() {
            Some(Value::Str(s)) => s,
            other => panic!("expected a string, got {other:?}"),
        }
    }

    #[test]
    fn joins_and_compares_strings() {
        assert_eq!(string(r#""Hello, " + "world""#), "Hello, world");
        assert!(boolean(r#""a" + "b" == "ab""#));
        assert!(boolean(r#""a" != "b""#));
    }

    #[test]
    fn interpolates_expressions_into_strings() {
        let source = "let price = 20\nlet tax = 4\n\"total: ${price + tax}, paid: ${price > 10}\"";
        assert_eq!(string(source), "total: 24, paid: true");
        assert_eq!(string(r#""${"nested ${1 + 1}"}!""#), "nested 2!");
    }

    #[test]
    fn refuses_to_mix_strings_and_numbers() {
        let message = error(r#""age: " + 36"#).message;
        assert!(message.contains("cannot use '+' on string and number"));
        assert!(message.contains("${...}"));
        assert!(error(r#""1" == 1"#).message.contains("cannot use '=='"));
    }

    #[test]
    fn reports_errors_inside_interpolation_on_the_right_line() {
        let error = error("let a = 1\nprint(\"value: ${nope}\")");
        assert_eq!(error.line, 2);
        assert_eq!(error.message, "'nope' is not defined");
    }

    #[test]
    fn calls_functions_and_returns_values() {
        assert_eq!(number("fn add(a, b) {\n  return a + b\n}\nadd(2, 3)"), 5.0);
        assert_eq!(
            number("let double = fn(n) { return n * 2 }\ndouble(21)"),
            42.0
        );
    }

    #[test]
    fn supports_recursion() {
        let source = "fn fib(n) {\n  if n < 2 {\n    return n\n  }\n  return fib(n - 1) + fib(n - 2)\n}\nfib(15)";
        assert_eq!(number(source), 610.0);
    }

    #[test]
    fn closures_remember_their_variables() {
        let source = "fn counter() {\n  let count = 0\n  return fn() {\n    count = count + 1\n    return count\n  }\n}\nlet next = counter()\nnext()\nnext()\nnext()";
        assert_eq!(number(source), 3.0);
    }

    #[test]
    fn passes_functions_as_values() {
        let source = "fn twice(f, x) {\n  return f(f(x))\n}\ntwice(fn(n) { return n * 2 }, 3)";
        assert_eq!(number(source), 12.0);
    }

    #[test]
    fn functions_without_return_give_none() {
        assert!(matches!(
            run("fn nothing() {\n  let x = 1\n}\nnothing()").unwrap(),
            Some(Value::None)
        ));
        assert_eq!(string("\"${print}\""), "<fn print>");
    }

    #[test]
    fn reports_bad_calls() {
        assert_eq!(
            error("fn add(a, b) {\n  return a + b\n}\nadd(1)").message,
            "add expects 2 arguments, got 1"
        );
        assert_eq!(error("let x = 1\nx(2)").message, "number is not a function");
        assert_eq!(
            error("return 5").message,
            "'return' can only be used inside a function"
        );
        assert!(
            error("fn forever(n) {\n  return forever(n + 1)\n}\nforever(0)")
                .message
                .contains("too much recursion")
        );
    }

    #[test]
    fn reports_errors_inside_functions_on_their_own_line() {
        let error = error("fn broken() {\n  return nope\n}\nbroken()");
        assert_eq!(error.line, 2);
        assert_eq!(error.message, "'nope' is not defined");
    }

    #[test]
    fn builds_and_reads_lists() {
        assert_eq!(number("let xs = [10, 20, 30]\nxs[1]"), 20.0);
        assert_eq!(number("len([1, 2, 3]) + len(\"four\")"), 7.0);
        assert_eq!(
            string("\"${[1, \"two\", [true]]}\""),
            "[1, \"two\", [true]]"
        );
        assert_eq!(string("\"hello\"[1]"), "e");
        assert_eq!(
            number("let grid = [\n  [1, 2],\n  [3, 4],\n]\ngrid[1][0]"),
            3.0
        );
    }

    #[test]
    fn changes_lists_in_place() {
        assert_eq!(
            number("let xs = [1, 2]\npush(xs, 3)\nxs[0] = 9\nxs[0] + xs[2]"),
            12.0
        );
        assert_eq!(number("let a = [1]\nlet b = a\npush(b, 2)\nlen(a)"), 2.0);
    }

    #[test]
    fn compares_lists_by_contents() {
        assert!(boolean("[1, [2, \"x\"]] == [1, [2, \"x\"]]"));
        assert!(boolean("[1, 2] != [2, 1]"));
        assert!(boolean("range(3) == [0, 1, 2]"));
        assert!(boolean("range(2, 5) == [2, 3, 4]"));
    }

    #[test]
    fn loops_over_lists_and_strings() {
        assert_eq!(
            number("let total = 0\nfor n in [1, 2, 3, 4] {\n  total = total + n\n}\ntotal"),
            10.0
        );
        assert_eq!(
            string("let out = \"\"\nfor c in \"abc\" {\n  out = c + out\n}\nout"),
            "cba"
        );
    }

    #[test]
    fn breaks_and_continues() {
        let source = "let total = 0\nfor n in range(10) {\n  if n == 7 {\n    break\n  }\n  if n % 2 == 0 {\n    continue\n  }\n  total = total + n\n}\ntotal";
        assert_eq!(number(source), 1.0 + 3.0 + 5.0);
        let source = "let i = 0\nwhile true {\n  i = i + 1\n  if i == 5 {\n    break\n  }\n}\ni";
        assert_eq!(number(source), 5.0);
    }

    #[test]
    fn each_loop_round_gets_its_own_variable() {
        let source = "let getters = []\nfor i in range(3) {\n  push(getters, fn() { return i })\n}\ngetters[1]()";
        assert_eq!(number(source), 1.0);
    }

    #[test]
    fn reports_list_mistakes() {
        assert_eq!(
            error("[1, 2][5]").message,
            "index 5 is out of range, the length is 2"
        );
        assert_eq!(
            error("[1, 2][0.5]").message,
            "index must be a whole number, got 0.5"
        );
        assert_eq!(
            error("[1][\"a\"]").message,
            "index must be a number, got string"
        );
        assert_eq!(
            error("let s = \"hi\"\ns[0] = \"y\"").message,
            "strings can't be changed in place"
        );
        assert_eq!(
            error("for x in 5 {\n}").message,
            "for needs a list or a string, got number"
        );
        assert_eq!(
            error("break").message,
            "'break' can only be used inside a loop"
        );
        assert_eq!(
            error("while true {\n  fn f() {\n    continue\n  }\n}").message,
            "'continue' can only be used inside a loop"
        );
        assert_eq!(error("push(1, 2)").message, "push needs a list, got number");
        assert_eq!(error("len()").message, "len expects 1 argument, got 0");
        assert_eq!(
            error("[1] == 1").message,
            "cannot use '==' on list and number"
        );
    }

    #[test]
    fn builds_and_reads_records() {
        let person = "let ada = { name: \"Ada\", born: 1815, langs: [\"math\"] }\n";
        assert_eq!(string(&format!("{person}ada.name")), "Ada");
        assert_eq!(
            number(&format!("{person}ada.born + len(ada.langs)")),
            1816.0
        );
        assert_eq!(number(&format!("{person}ada[\"born\"]")), 1815.0);
        assert_eq!(
            string(&format!("{person}\"${{ada}}\"")),
            "{name: \"Ada\", born: 1815, langs: [\"math\"]}"
        );
        assert_eq!(number("let p = {\n  x: 1,\n  y: { z: 2 },\n}\np.y.z"), 2.0);
        assert_eq!(number("len({ a: 1, b: 2 }) + len({})"), 2.0);
    }

    #[test]
    fn changes_existing_fields() {
        assert_eq!(
            number("let p = { age: 36 }\np.age = p.age + 1\np.age"),
            37.0
        );
        assert_eq!(number("let p = { age: 36 }\np[\"age\"] = 40\np.age"), 40.0);
        assert_eq!(number("let a = { n: 1 }\nlet b = a\nb.n = 2\na.n"), 2.0);
    }

    #[test]
    fn lists_keys_and_compares_records() {
        assert!(boolean("keys({ b: 1, a: 2 }) == [\"b\", \"a\"]"));
        assert!(boolean("{ a: 1, b: [2] } == { b: [2], a: 1 }"));
        assert!(boolean("{ a: 1 } != { a: 2 }"));
        assert!(boolean("{ a: 1 } != { a: 1, b: 2 }"));
    }

    #[test]
    fn records_can_hold_functions() {
        let source = "let math = { square: fn(n) { return n * n } }\nmath.square(7)";
        assert_eq!(number(source), 49.0);
    }

    #[test]
    fn reports_record_mistakes() {
        assert_eq!(
            error("let p = { age: 1 }\np.agee").message,
            "record has no field 'agee'"
        );
        assert!(
            error("let p = { age: 1 }\np.email = \"x\"")
                .message
                .starts_with("record has no field 'email', fields can only be added")
        );
        assert_eq!(error("{ a: 1, a: 2 }").message, "field 'a' is listed twice");
        assert_eq!(
            error("let n = 5\nn.size").message,
            "cannot read field 'size' of number"
        );
        assert!(
            error("for k in { a: 1 } {\n}")
                .message
                .contains("keys(record)")
        );
        assert_eq!(
            error("{ a: 1 }[0]").message,
            "record fields are named with strings, got number"
        );
    }
}
