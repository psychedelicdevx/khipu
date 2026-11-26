use std::collections::HashMap;
use std::fmt;

use crate::error::Error;
use crate::parser::{Block, Expr, Op, Stmt};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    Bool(bool),
    Str(String),
}

impl Value {
    fn type_name(&self) -> &'static str {
        match self {
            Value::Number(_) => "number",
            Value::Bool(_) => "bool",
            Value::Str(_) => "string",
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{n}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Str(s) => write!(f, "{s}"),
        }
    }
}

pub struct Interpreter {
    scopes: Vec<HashMap<String, Value>>,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            scopes: vec![HashMap::new()],
        }
    }

    pub fn run(&mut self, program: &Block) -> Result<Option<Value>, Error> {
        let mut last = None;
        for (line, stmt) in program {
            last = self.execute(*line, stmt)?;
        }
        Ok(last)
    }

    fn run_block(&mut self, block: &Block) -> Result<(), Error> {
        self.scopes.push(HashMap::new());
        let result = self.run(block);
        self.scopes.pop();
        result.map(|_| ())
    }

    fn execute(&mut self, line: usize, stmt: &Stmt) -> Result<Option<Value>, Error> {
        let at_line = |message| Error { line, message };

        match stmt {
            Stmt::Let(name, expr) => {
                let value = self.eval(expr).map_err(at_line)?;
                self.scopes
                    .last_mut()
                    .expect("the global scope always exists")
                    .insert(name.clone(), value);
                Ok(None)
            }
            Stmt::Assign(name, expr) => {
                let value = self.eval(expr).map_err(at_line)?;
                self.assign(name, value).map_err(at_line)?;
                Ok(None)
            }
            Stmt::Print(expr) => {
                println!("{}", self.eval(expr).map_err(at_line)?);
                Ok(None)
            }
            Stmt::If(condition, then_block, else_block) => {
                if self.eval_bool(condition, "if").map_err(at_line)? {
                    self.run_block(then_block)?;
                } else if let Some(block) = else_block {
                    self.run_block(block)?;
                }
                Ok(None)
            }
            Stmt::While(condition, body) => {
                while self.eval_bool(condition, "while").map_err(at_line)? {
                    self.run_block(body)?;
                }
                Ok(None)
            }
            Stmt::Expr(expr) => Ok(Some(self.eval(expr).map_err(at_line)?)),
        }
    }

    fn lookup(&self, name: &str) -> Option<Value> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .cloned()
    }

    fn assign(&mut self, name: &str, value: Value) -> Result<(), String> {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(slot) = scope.get_mut(name) {
                *slot = value;
                return Ok(());
            }
        }
        Err(format!(
            "'{name}' is not defined, use 'let {name} = ...' first"
        ))
    }

    fn eval(&self, expr: &Expr) -> Result<Value, String> {
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
            Expr::Variable(name) => self
                .lookup(name)
                .ok_or_else(|| format!("'{name}' is not defined")),
            Expr::Negate(inner) => match self.eval(inner)? {
                Value::Number(n) => Ok(Value::Number(-n)),
                other => Err(format!("'-' needs a number, got {}", other.type_name())),
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
                binary(*op, a, b)
            }
        }
    }

    fn eval_bool(&self, expr: &Expr, what: &str) -> Result<bool, String> {
        match self.eval(expr)? {
            Value::Bool(b) => Ok(b),
            other => Err(format!(
                "{what} needs true or false, got {}",
                other.type_name()
            )),
        }
    }
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
}
