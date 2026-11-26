mod interpreter;
mod lexer;
mod parser;

use std::io::{self, Write};

use interpreter::Interpreter;

fn main() {
    let stdin = io::stdin();
    let mut interpreter = Interpreter::new();

    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut line = String::new();
        if stdin.read_line(&mut line).unwrap() == 0 {
            break; // Ctrl-D
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

        let stmt = match parser::Parser::new(tokens).parse() {
            Ok(stmt) => stmt,
            Err(error) => {
                println!("Error: {error}");
                continue;
            }
        };

        match interpreter.execute(&stmt) {
            Ok(Some(value)) => println!("= {value}"),
            Ok(None) => {}
            Err(error) => println!("Error: {error}"),
        }
    }
}
