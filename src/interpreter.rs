use std::collections::HashMap;

use crate::error::Error;
use crate::parser::{Expr, Op, Stmt};

pub struct Interpreter {
    variables: HashMap<String, f64>,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            variables: HashMap::new(),
        }
    }

    pub fn run(&mut self, program: &[(usize, Stmt)]) -> Result<Option<f64>, Error> {
        let mut last = None;
        for (line, stmt) in program {
            last = self.execute(stmt).map_err(|message| Error {
                line: *line,
                message,
            })?;
        }
        Ok(last)
    }

    fn execute(&mut self, stmt: &Stmt) -> Result<Option<f64>, String> {
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

    fn eval(&self, expr: &Expr) -> Result<f64, String> {
        match expr {
            Expr::Number(n) => Ok(*n),
            Expr::Variable(name) => match self.variables.get(name) {
                Some(value) => Ok(*value),
                None => Err(format!("'{name}' is not defined")),
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
    use crate::error::Error;
    use crate::{lexer::lex, parser::Parser};

    fn run(source: &str) -> Result<Option<f64>, Error> {
        let program = Parser::new(lex(source)?).parse_program()?;
        Interpreter::new().run(&program)
    }

    #[test]
    fn respects_precedence_and_parentheses() {
        assert_eq!(run("2 + 3 * 4").unwrap(), Some(14.0));
        assert_eq!(run("(2 + 3) * 4").unwrap(), Some(20.0));
        assert_eq!(run("8 / 2 / 2").unwrap(), Some(2.0));
        assert_eq!(run("-2 * 3").unwrap(), Some(-6.0));
    }

    #[test]
    fn runs_a_multi_line_program() {
        let source = "// set up\nlet x = 2 + 3\n\nlet y = x * 4 // twenty\ny";
        assert_eq!(run(source).unwrap(), Some(20.0));
    }

    #[test]
    fn reports_the_line_of_an_error() {
        let error = run("let a = 1\n\nprint(b)").unwrap_err();
        assert_eq!(error.line, 3);
        assert_eq!(error.message, "'b' is not defined");
    }

    #[test]
    fn rejects_two_statements_on_one_line() {
        assert_eq!(run("let x = 1\n2 3").unwrap_err().line, 2);
    }
}
