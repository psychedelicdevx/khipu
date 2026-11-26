use std::collections::HashMap;

use crate::parser::{Expr, Op, Stmt};

#[derive(Debug)]
pub enum RuntimeError {
    UndefinedVariable(String),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            RuntimeError::UndefinedVariable(name) => write!(f, "'{name}' is not defined"),
        }
    }
}

pub struct Interpreter {
    variables: HashMap<String, f64>,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            variables: HashMap::new(),
        }
    }

    pub fn execute(&mut self, stmt: &Stmt) -> Result<Option<f64>, RuntimeError> {
        match stmt {
            Stmt::Let(name, expr) => {
                let value = self.eval(expr)?;
                self.variables.insert(name.clone(), value);
                Ok(None)
            }
            Stmt::Print(expr) => {
                println!("{}", self.eval(expr)?);
                Ok(None)
            }
            Stmt::Expr(expr) => Ok(Some(self.eval(expr)?)),
        }
    }

    fn eval(&self, expr: &Expr) -> Result<f64, RuntimeError> {
        match expr {
            Expr::Number(n) => Ok(*n),
            Expr::Variable(name) => match self.variables.get(name) {
                Some(value) => Ok(*value),
                None => Err(RuntimeError::UndefinedVariable(name.clone())),
            },
            Expr::Negate(inner) => Ok(-self.eval(inner)?),
            Expr::Binary(left, op, right) => {
                let a = self.eval(left)?;
                let b = self.eval(right)?;
                Ok(match op {
                    Op::Add => a + b,
                    Op::Sub => a - b,
                    Op::Mul => a * b,
                    Op::Div => a / b,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Interpreter;
    use crate::{lexer::lex, parser::Parser};

    fn run(lines: &[&str]) -> Option<f64> {
        let mut interpreter = Interpreter::new();
        let mut last = None;
        for line in lines {
            let stmt = Parser::new(lex(line).unwrap()).parse().unwrap();
            last = interpreter.execute(&stmt).unwrap();
        }
        last
    }

    #[test]
    fn respects_precedence_and_parentheses() {
        assert_eq!(run(&["2 + 3 * 4"]), Some(14.0));
        assert_eq!(run(&["(2 + 3) * 4"]), Some(20.0));
        assert_eq!(run(&["8 / 2 / 2"]), Some(2.0));
        assert_eq!(run(&["-2 * 3"]), Some(-6.0));
    }

    #[test]
    fn remembers_variables() {
        assert_eq!(run(&["let x = 2 + 3", "let y = x * 4", "y"]), Some(20.0));
    }

    #[test]
    fn reports_undefined_variables() {
        let stmt = Parser::new(lex("nope + 1").unwrap()).parse().unwrap();
        assert!(Interpreter::new().execute(&stmt).is_err());
    }
}
