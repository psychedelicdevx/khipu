use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process;
use std::rc::Rc;
use std::thread;

use khipu::error::Error;
use khipu::interpreter::{Interpreter, Value};
use khipu::parser::{Expr, Parser, Stmt};
use khipu::{format, lexer, lsp, parser};

const STACK_SIZE: usize = 256 * 1024 * 1024;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let khipu = thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(move || match args.get(1).map(String::as_str) {
            Some("fmt") => format_files(&args[2..]),
            Some("test") => test_files(&args[2..]),
            Some("lsp") => lsp::serve(),
            Some(path) => run_file(path),
            None => repl(),
        })
        .expect("could not start the interpreter thread");

    if khipu.join().is_err() {
        process::exit(1);
    }
}

fn parse(source: &str) -> Result<parser::Block, Error> {
    Parser::new(lexer::lex(source)?).parse_program()
}

fn run_file(path: &str) {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("Could not read {path}: {error}");
            process::exit(1);
        }
    };

    khipu::task::set_program(path, &source);
    khipu::stdlib::set_args(std::env::args().skip(2).collect());
    let dir = Path::new(path).parent().unwrap_or(Path::new("."));
    if let Err(error) =
        parse(&source).and_then(|program| Interpreter::in_dir(dir).run_main(&program))
    {
        eprintln!("{}", error.render(path, &source));
        process::exit(1);
    }
}

fn format_files(args: &[String]) {
    let check = args.iter().any(|arg| arg == "--check");
    let paths: Vec<&String> = args.iter().filter(|arg| *arg != "--check").collect();
    if paths.is_empty() {
        eprintln!("usage: khipu fmt [--check] <file.kp>...");
        process::exit(2);
    }
    let mut failed = false;
    for path in paths {
        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("Could not read {path}: {error}");
                process::exit(1);
            }
        };
        let formatted = match format::format(&source) {
            Ok(formatted) => formatted,
            Err(error) => {
                eprintln!("{}", error.render(path, &source));
                failed = true;
                continue;
            }
        };
        if formatted == source {
            continue;
        }
        if check {
            println!("{path} is not formatted");
            failed = true;
        } else if let Err(error) = fs::write(path, formatted) {
            eprintln!("Could not write {path}: {error}");
            failed = true;
        } else {
            println!("formatted {path}");
        }
    }
    if failed {
        process::exit(1);
    }
}

fn test_files(paths: &[String]) {
    if paths.is_empty() {
        eprintln!("usage: khipu test <file.kp>...");
        process::exit(2);
    }
    let (mut passed, mut failed) = (0, 0);
    for path in paths {
        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("Could not read {path}: {error}");
                process::exit(1);
            }
        };
        let program = match parse(&source) {
            Ok(program) => program,
            Err(error) => {
                eprintln!("{}", error.render(path, &source));
                failed += 1;
                continue;
            }
        };
        let (tests, setup): (Vec<_>, Vec<_>) = program
            .into_iter()
            .partition(|(_, stmt)| matches!(stmt, Stmt::Test(..)));
        let dir = Path::new(path).parent().unwrap_or(Path::new("."));
        let mut interpreter = Interpreter::in_dir(dir);
        if let Err(error) = interpreter.run(&setup) {
            eprintln!("{}", error.render(path, &source));
            failed += 1;
            continue;
        }
        println!("{path}");
        for (line, stmt) in tests {
            let Stmt::Test(title, body) = stmt else {
                unreachable!()
            };
            let call = Expr::Call(
                Box::new(Expr::Function {
                    name: title.clone(),
                    type_params: Vec::new(),
                    params: Vec::new(),
                    param_types: Vec::new(),
                    returns: None,
                    body: Rc::new(body),
                }),
                Vec::new(),
            );
            match interpreter.run(&vec![(line, Stmt::Expr(call))]) {
                Ok(_) => {
                    passed += 1;
                    println!("  ✓ {title}");
                }
                Err(error) => {
                    failed += 1;
                    println!("  ✗ {title}");
                    eprintln!("{}\n", error.render(path, &source));
                }
            }
        }
    }
    println!("\n{passed} passed, {failed} failed");
    if failed > 0 {
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

        let open = source.matches(['{', '[']).count();
        let close = source.matches(['}', ']']).count();
        if open > close {
            continue;
        }

        match parse(&source).and_then(|program| interpreter.run(&program)) {
            Ok(Some(Value::None)) | Ok(None) => {}
            Ok(Some(value)) => println!("= {value}"),
            Err(error) => eprintln!("{}", error.render("repl", &source)),
        }
        source.clear();
    }
}
