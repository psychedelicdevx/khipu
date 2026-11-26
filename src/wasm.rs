use std::cell::RefCell;

use crate::checker::Checker;
use crate::error::{Error, underline};
use crate::format::format;
use crate::interpreter::Interpreter;
use crate::lexer::lex;
use crate::parser::{Block, Parser};
use crate::value::OUTPUT;

const CAPACITY: usize = 1 << 20;

thread_local! {
    static INPUT: RefCell<Vec<u8>> = RefCell::new(vec![0; CAPACITY]);
    static RESULT: RefCell<String> = const { RefCell::new(String::new()) };
}

fn source(len: usize) -> String {
    INPUT.with(|input| String::from_utf8_lossy(&input.borrow()[..len.min(CAPACITY)]).into_owned())
}

fn finish(text: String) -> usize {
    RESULT.with(|result| {
        *result.borrow_mut() = text;
        result.borrow().len()
    })
}

fn parse(source: &str) -> Result<Block, Error> {
    Parser::new(lex(source)?).parse_program()
}

#[unsafe(no_mangle)]
pub extern "C" fn input_ptr() -> *const u8 {
    INPUT.with(|input| input.borrow().as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn input_capacity() -> usize {
    CAPACITY
}

#[unsafe(no_mangle)]
pub extern "C" fn result_ptr() -> *const u8 {
    RESULT.with(|result| result.borrow().as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn run(len: usize) -> usize {
    let source = source(len);
    OUTPUT.with(|output| output.borrow_mut().clear());
    let outcome = parse(&source).and_then(|program| Interpreter::new().run_main(&program));
    let mut text = OUTPUT.with(|output| std::mem::take(&mut *output.borrow_mut()));
    if let Err(error) = outcome {
        text.push('\u{0}');
        text.push_str(&error.render("main.kp", &source));
    }
    finish(text)
}

#[unsafe(no_mangle)]
pub extern "C" fn check(len: usize) -> usize {
    let source = source(len);
    let Err(error) = parse(&source).and_then(|program| Checker::new().check(&program)) else {
        return finish(String::new());
    };
    let text = source.lines().nth(error.line.wrapping_sub(1)).unwrap_or("");
    let (start, width) = underline(text, &error.message);
    finish(format!(
        "{}\t{start}\t{width}\t{}",
        error.line, error.message
    ))
}

#[unsafe(no_mangle)]
pub extern "C" fn tidy(len: usize) -> usize {
    let source = source(len);
    finish(format(&source).unwrap_or(source))
}
