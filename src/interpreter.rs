use crate::parser::{Expr, Op};

pub fn eval(expr: &Expr) -> f64 {
    match expr {
        Expr::Number(n) => *n,
        Expr::Negate(inner) => -eval(inner),
        Expr::Binary(left, op, right) => {
            let a = eval(left);
            let b = eval(right);
            match op {
                Op::Add => a + b,
                Op::Sub => a - b,
                Op::Mul => a * b,
                Op::Div => a / b,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{lexer::lex, parser::Parser};

    fn run(source: &str) -> f64 {
        let tokens = lex(source).unwrap();
        let expr = Parser::new(tokens).parse().unwrap();
        super::eval(&expr)
    }

    #[test]
    fn respects_precedence_and_parentheses() {
        assert_eq!(run("2 + 3 * 4"), 14.0);
        assert_eq!(run("(2 + 3) * 4"), 20.0);
        assert_eq!(run("8 / 2 / 2"), 2.0);
        assert_eq!(run("-2 * 3"), -6.0);
    }
}
