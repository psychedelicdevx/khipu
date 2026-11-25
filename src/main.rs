mod lexer;

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

        match lexer::lex(line.trim_end()) {
            Ok(tokens) => println!("{:?}", tokens),
            Err(error) => println!("Error: {error}"),
        }
    }
}
