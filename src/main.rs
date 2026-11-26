mod interpreter;
mod lexer;
mod parser;

use std::io::{self, Write};

fn main() {
    let stdin = io::stdin();

    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut line = String::new();
        if stdin.read_line(&mut line).unwrap() == 0 {
            break;
        }

        let tokens = match lexer::lex(line.trim_end()) {
            Ok(tokens) => tokens,
            Err(error) => {
                println!("Error: {error}");
                continue;
            }
        };
        if tokens.is_empty() {
            continue;
        }

        let expr = match parser::Parser::new(tokens).parse() {
            Ok(expr) => expr,
            Err(error) => {
                println!("Error: {error}");
                continue;
            }
        };

        println!("{expr:?}");
        println!("= {}", interpreter::eval(&expr));
    }
}
