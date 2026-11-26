mod error;
mod interpreter;
mod lexer;
mod parser;

use std::fs;
use std::io::{self, Write};
use std::process;
use std::thread;

use error::Error;
use interpreter::{Interpreter, Value};
use parser::Parser;

const STACK_SIZE: usize = 256 * 1024 * 1024;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let khipu = thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(move || match args.get(1) {
            Some(path) => run_file(path),
            None => repl(),
        })
        .expect("could not start the interpreter thread");

    if khipu.join().is_err() {
        process::exit(1);
    }
}

fn run_source(source: &str, interpreter: &mut Interpreter) -> Result<Option<Value>, Error> {
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
    let mut source = String::new();

    loop {
        print!("{}", if source.is_empty() { "> " } else { "... " });
        io::stdout().flush().unwrap();

        let mut line = String::new();
        if stdin.read_line(&mut line).unwrap() == 0 {
            break;
        }
        source.push_str(&line);

        let open = source.matches('{').count();
        let close = source.matches('}').count();
        if open > close {
            continue;
        }

        match run_source(&source, &mut interpreter) {
            Ok(Some(Value::None)) | Ok(None) => {}
            Ok(Some(value)) => println!("= {value}"),
            Err(error) => println!("Error: {}", error.message),
        }
        source.clear();
    }
}
