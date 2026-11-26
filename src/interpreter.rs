use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::error::Error;
use crate::parser::{Block, Expr, Op, Stmt};

const MAX_CALL_DEPTH: usize = 1000;

type NativeFn = fn(&[Value]) -> Result<Value, String>;

#[derive(Clone)]
pub enum Value {
    Number(f64),
    Bool(bool),
    Str(String),
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
}

impl Value {
    fn type_name(&self) -> &'static str {
        match self {
            Value::Number(_) => "number",
            Value::Bool(_) => "bool",
            Value::Str(_) => "string",
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
        globals
            .borrow_mut()
            .vars
            .insert("print".to_string(), Value::Native("print", print));
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
            Stmt::While(condition, body) => {
                while self.eval_bool(condition, "while")? {
                    self.run_block(body)?;
                }
                Ok(None)
            }
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

fn binary(op: Op, a: Value, b: Value) -> Result<Value, String> {
    use Value::{Bool, Number, Str};

    match (op, a, b) {
        (Op::Add, Number(x), Number(y)) => Ok(Number(x + y)),
        (Op::Add, Str(x), Str(y)) => Ok(Str(x + &y)),
        (Op::Add, a @ Str(_), b) | (Op::Add, a, b @ Str(_)) => Err(format!(
            "cannot use '+' on {} and {}, use \"${{...}}\" to put values inside a string",
            a.type_name(),
            b.type_name()
        )),
        (Op::Equal, Str(x), Str(y)) => Ok(Bool(x == y)),
        (Op::NotEqual, Str(x), Str(y)) => Ok(Bool(x != y)),
        (Op::Sub, Number(x), Number(y)) => Ok(Number(x - y)),
        (Op::Mul, Number(x), Number(y)) => Ok(Number(x * y)),
        (Op::Div, Number(x), Number(y)) => Ok(Number(x / y)),
        (Op::Rem, Number(x), Number(y)) => Ok(Number(x % y)),
        (Op::Less, Number(x), Number(y)) => Ok(Bool(x < y)),
        (Op::LessEqual, Number(x), Number(y)) => Ok(Bool(x <= y)),
        (Op::Greater, Number(x), Number(y)) => Ok(Bool(x > y)),
        (Op::GreaterEqual, Number(x), Number(y)) => Ok(Bool(x >= y)),
        (Op::Equal, Number(x), Number(y)) => Ok(Bool(x == y)),
        (Op::Equal, Bool(x), Bool(y)) => Ok(Bool(x == y)),
        (Op::NotEqual, Number(x), Number(y)) => Ok(Bool(x != y)),
        (Op::NotEqual, Bool(x), Bool(y)) => Ok(Bool(x != y)),
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
}
