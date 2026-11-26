mod error;
mod interpreter;
mod lexer;
mod parser;

use std::fs;
use std::io::{self, Write};
use std::process;

use error::Error;
use interpreter::Interpreter;
use parser::Parser;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1) {
        Some(path) => run_file(path),
        None => repl(),
    }
}

fn run_source(source: &str, interpreter: &mut Interpreter) -> Result<Option<f64>, Error> {
    let tokens = lexer::lex(source)?;
    let program = Parser::new(tokens).parse_program()?;
    interpreter.run(&program)
}

fn run_file(path: &str) {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("Could not read {path}: {error}");
            process::exit(1);
        }
    };

    if let Err(error) = run_source(&source, &mut Interpreter::new()) {
        eprintln!("Error on line {}: {}", error.line, error.message);
        process::exit(1);
    }
}

fn repl() {
    let stdin = io::stdin();
    let mut interpreter = Interpreter::new();

    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut line = String::new();
        if stdin.read_line(&mut line).unwrap() == 0 {
            break;
        }

        match run_source(&line, &mut interpreter) {
            Ok(Some(value)) => println!("= {value}"),
            Ok(None) => {}
            Err(error) => println!("Error: {}", error.message),
        }
    }
}
