use std::path::Path;
use std::rc::Rc;

use crate::checker::Checker;
use crate::compiler::{Globals, compile_script, mark_builtin};
use crate::error::Error;
use crate::lexer::lex_prelude;
use crate::parser::Block;
use crate::parser::Parser;
use crate::stdlib;
pub use crate::value::Value;
use crate::value::{Fields, NATIVES};
use crate::vm::{Loading, Modules, Vm};

pub struct Interpreter {
    checker: Checker,
    globals: Globals,
    vm: Vm,
    builtins: usize,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter::in_dir(Path::new("."))
    }

    pub fn in_dir(dir: &Path) -> Self {
        Interpreter::with_modules(dir, Rc::default(), Rc::default())
    }

    pub fn with_modules(dir: &Path, modules: Modules, loading: Loading) -> Self {
        let mut globals = Globals::default();
        let mut vm = Vm::new(dir, modules, loading);
        for native in NATIVES.iter().chain(stdlib::NATIVES.iter()) {
            let slot = globals.slot(native.name) as usize;
            vm.sync_globals(&globals.names);
            vm.globals.borrow_mut().values[slot] = Some(Value::Native(native));
        }
        let mut interpreter = Interpreter {
            checker: Checker::new(),
            globals,
            vm,
            builtins: 0,
        };
        let prelude = lex_prelude(stdlib::PRELUDE)
            .and_then(|tokens| Parser::new(tokens).parse_program())
            .and_then(|program| interpreter.execute(&program, false, true));
        if let Err(error) = prelude {
            panic!(
                "the built-in library failed on line {}: {}",
                error.line, error.message
            );
        }
        interpreter.builtins = interpreter.globals.names.len();
        interpreter.vm.load_methods();
        interpreter
    }

    pub fn run(&mut self, program: &Block) -> Result<Option<Value>, Error> {
        self.execute(program, false, false)
    }

    pub fn run_main(&mut self, program: &Block) -> Result<Option<Value>, Error> {
        self.execute(program, true, false)
    }

    fn execute(
        &mut self,
        program: &Block,
        promote: bool,
        builtin: bool,
    ) -> Result<Option<Value>, Error> {
        self.checker.check(program)?;
        let (mut proto, ends_with_value) = compile_script(program, &mut self.globals, promote)?;
        if builtin {
            mark_builtin(&mut proto);
        }
        self.vm.sync_globals(&self.globals.names);
        let value = self.vm.run(Rc::new(proto))?;
        Ok(ends_with_value.then_some(value))
    }

    pub fn methods(&self) -> crate::vm::Methods {
        self.vm.methods()
    }

    pub fn exports(&self) -> Fields {
        let globals = self.vm.globals.borrow();
        let mut fields: Fields = globals
            .names
            .iter()
            .zip(&globals.values)
            .skip(self.builtins)
            .filter_map(|(name, value)| match value {
                Some(Value::Native(..)) | None => None,
                Some(value) => Some((Rc::from(name.as_str()), value.clone())),
            })
            .collect();
        fields.sort_by(|a, b| a.0.cmp(&b.0));
        fields
    }
}

#[cfg(test)]
mod tests {
    use super::{Interpreter, Value};
    use crate::error::Error;
    use crate::{lexer::lex, parser::Parser};

    fn run(source: &str) -> Result<Option<Value>, Error> {
        let program = Parser::new(lex(source)?).parse_program()?;
        Interpreter::new().run_main(&program)
    }

    fn number(source: &str) -> f64 {
        match run(source).unwrap() {
            Some(Value::Number(n)) => n,
            other => panic!("expected a number, got {other:?}"),
        }
    }

    fn boolean(source: &str) -> bool {
        match run(source).unwrap() {
            Some(Value::Bool(b)) => b,
            other => panic!("expected a bool, got {other:?}"),
        }
    }

    fn error(source: &str) -> Error {
        run(source).unwrap_err()
    }

    #[test]
    fn respects_precedence_and_parentheses() {
        assert_eq!(number("2 + 3 * 4"), 14.0);
        assert_eq!(number("(2 + 3) * 4"), 20.0);
        assert_eq!(number("8 / 2 / 2"), 2.0);
        assert_eq!(number("-2 * 3"), -6.0);
        assert_eq!(number("17 % 5"), 2.0);
    }

    #[test]
    fn runs_a_multi_line_program() {
        let source = "// set up\nlet x = 2 + 3\n\nlet y = x * 4 // twenty\ny";
        assert_eq!(number(source), 20.0);
    }

    #[test]
    fn has_a_standard_library() {
        assert_eq!(
            string(
                "\"  a,b ,c \".trim().split(\",\").map(fn(s: string) -> string { return s.trim().upper() }).join(\"\")"
            ),
            "ABC"
        );
        assert_eq!(
            number(
                "[3, 1, 2].sorted().reversed()[0] + [4, 5].sum() + [1, 2, 3].reduce(0, fn(a: number, n: number) -> number { return a + n })"
            ),
            18.0
        );
        assert_eq!(
            number("[1, 2, 3, 4].filter(fn(n: number) -> bool { return n % 2 == 0 }).len()"),
            2.0
        );
        assert_eq!(
            string(
                "[\"bb\", \"a\", \"ccc\"].sort_by(fn(s: string) -> any { return s.len() }).join(\" \")"
            ),
            "a bb ccc"
        );
        assert_eq!(
            number("(2).pow(8) + (9.7).floor() + Math.sqrt(9) + (-1).abs()"),
            269.0
        );
        assert_eq!(number("\"khipu\".index_of(\"pu\") ?? -1"), 3.0);
        assert_eq!(number("\"x\".to_number() ?? -1"), -1.0);
        assert_eq!(string("\"khipu\".slice(-3, 5)"), "ipu");
        assert!(boolean(
            "[1, 2].any(fn(n: number) -> bool { return n > 1 }) && ![1, 2].all(fn(n: number) -> bool { return n > 1 })"
        ));
        assert_eq!(
            number(
                "let xs = [1, 2]\nxs.push(3)\nxs.insert(0, 0)\nxs.remove(1) + xs.len() + (xs.pop() ?? 0)"
            ),
            7.0
        );
        assert_eq!(
            error("[\"a\"].sum()").message,
            "list.sum needs a list<number>, got list<string>"
        );
        assert_eq!(
            error("\"a\".upper(1)").message,
            "string.upper expects 0 arguments, got 1"
        );
        assert_eq!(
            error("\"a\".shout()").message,
            "string has no method 'shout'"
        );
        assert_eq!(
            error("[1].remove(5)").message,
            "remove index 5 is outside a list of length 1"
        );
        assert_eq!(
            error("(-1).sqrt()").message,
            "sqrt needs a number of 0 or more, got -1"
        );

        let folder = std::env::temp_dir().join(format!("khipu-files-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("notes.txt").display().to_string();
        let source = format!(
            "File.write(\"{path}\", \"a\\nb\\n\")\nFile.append(\"{path}\", \"c\")\nlet seen = File.lines(\"{path}\").join(\"\") + File.list(\"{}\").join(\",\")\nFile.delete(\"{path}\")\nif File.exists(\"{path}\") {{\n    seen = \"left behind\"\n}}\nseen",
            folder.display()
        );
        assert_eq!(string(&source), "abcnotes.txt");
        std::fs::remove_dir_all(&folder).unwrap();
        assert_eq!(
            error("File.read(\"/no/such/khipu/file\")").message,
            "cannot read /no/such/khipu/file: it does not exist"
        );
    }

    #[test]
    fn checks_contracts() {
        let divide = "fn divide(a: number, b: number) -> number\n    requires b != 0\n    ensures result * b == a\n    example divide(6, 3) == 2\n{\n    return a / b\n}\n";
        assert_eq!(number(&format!("{divide}divide(9, 3)")), 3.0);
        let broken = error(&format!("{divide}let x = 1\ndivide(x, 0)"));
        assert_eq!(broken.message, "divide requires b != 0 (a = 1, b = 0)");
        assert_eq!(broken.line, 9);

        let wrong = error(
            "fn up(n: number) -> number\n    ensures result > n\n{\n    if n > 0 {\n        return n + 1\n    }\n    return n\n}\nup(1) + up(-1)",
        );
        assert_eq!(wrong.message, "up ensures result > n, but it returned -1");
        assert_eq!(wrong.line, 7);

        assert_eq!(
            error("fn outer() {\n    fn inner()\n        example inner() == none\n    {\n    }\n}")
                .message,
            "examples can only be written on top-level functions and methods"
        );
        assert_eq!(
            number("let requires = 2\nlet example = 3\nrequires * example"),
            6.0
        );
    }

    #[test]
    fn runs_tasks_that_talk_over_channels() {
        let workers = "let done = channel()\nfn square(n: number) {\n    send(done, n * n)\n}\nfor n in range(1, 11) {\n    spawn(square, n)\n}\nlet total = 0\nfor _ in range(10) {\n    total = total + receive(done)\n}\ntotal";
        assert_eq!(number(workers), 385.0);

        let copies = "let box = channel()\nlet items: list = [1, 2]\npush(items, items)\nsend(box, items)\nlet got = receive(box)\npush(got, 3)\nlen(items) * 10 + len(got[2])";
        assert_eq!(number(copies), 34.0);

        let shapes = "struct P {\n    x: number,\n\n    fn double(self) -> number {\n        return self.x * 2\n    }\n}\nenum S {\n    A(number),\n}\nlet back = channel()\nfn echo(p: P, s: S) {\n    send(back, [p, s])\n}\nspawn(echo, P { x: 4 }, S.A(1))\nlet got = receive(back)\ngot[0].double() == 8 && got[1] == S.A(1) && got[0] == P { x: 4 }";
        assert!(boolean(shapes));

        let closures = "let back = channel()\nlet base = 5\nfn countdown(n: number) -> number {\n    if n == 0 {\n        return base\n    }\n    return countdown(n - 1) + 1\n}\nfn run(f: fn(number) -> number) {\n    send(back, f(3))\n}\nspawn(run, countdown)\nreceive(back)";
        assert_eq!(number(closures), 8.0);

        let picks = "let a = channel()\nlet b = channel()\nsend(b, 7)\nlet got = 0\nselect {\n    x = receive(a) => {\n        got = x\n    }\n    y = receive(b) => {\n        got = y * 2\n    }\n}\ngot";
        assert_eq!(number(picks), 14.0);

        let stuck = error(
            "let nobody = channel()\nfn wait() {\n    receive(channel())\n}\nspawn(wait)\nreceive(nobody)",
        );
        assert_eq!(stuck.line, 6);
        assert_eq!(
            stuck.message,
            "deadlock: every task is waiting on a channel that will never get a value"
        );
        assert_eq!(
            error("select {}").message,
            "select needs at least one receive(channel) arm"
        );
        assert_eq!(
            error("send(1, 2)").message,
            "argument 1 of 'send' should be channel, got number"
        );
    }

    #[test]
    fn skips_tests_in_a_normal_run_and_asserts() {
        assert_eq!(
            number("test \"never runs\" {\n    assert(false)\n}\nlet a = 1\na"),
            1.0
        );
        assert_eq!(error("assert_eq(1 + 1, 3)").message, "expected 3, got 2");
        assert_eq!(
            error("assert_eq(\"a\", \"b\")").message,
            "expected \"b\", got \"a\""
        );
        assert_eq!(
            error("assert(1 > 2, \"maths\")").message,
            "assertion failed: maths"
        );
        assert_eq!(
            error("fn f() {\n    test \"x\" {}\n}").message,
            "tests can only be written at the top level"
        );
    }

    #[test]
    fn reports_the_line_of_an_error() {
        let error = error("let a = 1\n\nprint(b)");
        assert_eq!(error.line, 3);
        assert_eq!(error.message, "'b' is not defined");
    }

    #[test]
    fn rejects_two_statements_on_one_line() {
        assert_eq!(error("let x = 1\n2 3").line, 2);
    }

    #[test]
    fn compares_and_combines() {
        assert!(boolean("1 + 2 * 3 == 7 && !false"));
        assert!(boolean("3 >= 3 || nope"));
        assert!(!boolean("false && nope"));
        assert!(boolean("2 != 3"));
    }

    #[test]
    fn picks_the_right_branch() {
        let source = "let x = 5\nlet y = 0\nif x > 3 {\n  y = 1\n} else if x > 1 {\n  y = 2\n} else {\n  y = 3\n}\ny";
        assert_eq!(number(source), 1.0);
        assert_eq!(number(&source.replace("x = 5", "x = 2")), 2.0);
        assert_eq!(number(&source.replace("x = 5", "x = 0")), 3.0);
    }

    #[test]
    fn loops_with_while() {
        let source =
            "let i = 1\nlet total = 0\nwhile i <= 5 {\n  total = total + i\n  i = i + 1\n}\ntotal";
        assert_eq!(number(source), 15.0);
    }

    #[test]
    fn keeps_block_variables_inside_the_block() {
        assert_eq!(number("let x = 1\nif true {\n  let x = 2\n}\nx"), 1.0);
        assert_eq!(error("if true { let y = 2 }\ny").line, 2);
    }

    #[test]
    fn has_no_truthiness_or_coercion() {
        assert!(error("if 1 { print(1) }").message.contains("true or false"));
        assert!(error("1 + true").message.contains("cannot use '+'"));
        assert!(error("1 == true").message.contains("cannot use '=='"));
        assert!(error("x = 5").message.contains("not defined"));
    }

    #[test]
    fn reports_errors_inside_blocks_on_their_own_line() {
        assert_eq!(error("let a = 1\nif true {\n  print(nope)\n}").line, 3);
    }

    fn string(source: &str) -> String {
        match run(source).unwrap() {
            Some(Value::Str(s)) => s.to_string(),
            other => panic!("expected a string, got {other:?}"),
        }
    }

    #[test]
    fn joins_and_compares_strings() {
        assert_eq!(string(r#""Hello, " + "world""#), "Hello, world");
        assert!(boolean(r#""a" + "b" == "ab""#));
        assert!(boolean(r#""a" != "b""#));
    }

    #[test]
    fn interpolates_expressions_into_strings() {
        let source = "let price = 20\nlet tax = 4\n\"total: ${price + tax}, paid: ${price > 10}\"";
        assert_eq!(string(source), "total: 24, paid: true");
        assert_eq!(string(r#""${"nested ${1 + 1}"}!""#), "nested 2!");
    }

    #[test]
    fn refuses_to_mix_strings_and_numbers() {
        let message = error(r#""age: " + 36"#).message;
        assert!(message.contains("cannot use '+' on string and number"));
        assert!(message.contains("${...}"));
        assert!(error(r#""1" == 1"#).message.contains("cannot use '=='"));
    }

    #[test]
    fn reports_errors_inside_interpolation_on_the_right_line() {
        let error = error("let a = 1\nprint(\"value: ${nope}\")");
        assert_eq!(error.line, 2);
        assert_eq!(error.message, "'nope' is not defined");
    }

    #[test]
    fn calls_functions_and_returns_values() {
        assert_eq!(number("fn add(a, b) {\n  return a + b\n}\nadd(2, 3)"), 5.0);
        assert_eq!(
            number("let double = fn(n) { return n * 2 }\ndouble(21)"),
            42.0
        );
    }

    #[test]
    fn supports_recursion() {
        let source = "fn fib(n) {\n  if n < 2 {\n    return n\n  }\n  return fib(n - 1) + fib(n - 2)\n}\nfib(15)";
        assert_eq!(number(source), 610.0);
    }

    #[test]
    fn closures_remember_their_variables() {
        let source = "fn counter() {\n  let count = 0\n  return fn() {\n    count = count + 1\n    return count\n  }\n}\nlet next = counter()\nnext()\nnext()\nnext()";
        assert_eq!(number(source), 3.0);
    }

    #[test]
    fn passes_functions_as_values() {
        let source = "fn twice(f, x) {\n  return f(f(x))\n}\ntwice(fn(n) { return n * 2 }, 3)";
        assert_eq!(number(source), 12.0);
    }

    #[test]
    fn functions_without_return_give_none() {
        assert!(matches!(
            run("fn nothing() {\n  let x = 1\n}\nnothing()").unwrap(),
            Some(Value::None)
        ));
        assert_eq!(string("\"${print}\""), "<fn print>");
    }

    #[test]
    fn reports_bad_calls() {
        assert_eq!(
            error("fn add(a, b) {\n  return a + b\n}\nadd(1)").message,
            "add expects 2 arguments, got 1"
        );
        assert_eq!(error("let x = 1\nx(2)").message, "number is not a function");
        assert_eq!(
            error("return 5").message,
            "'return' can only be used inside a function"
        );
        assert!(
            error("fn forever(n) {\n  return forever(n + 1)\n}\nforever(0)")
                .message
                .contains("too much recursion")
        );
    }

    #[test]
    fn reports_errors_inside_functions_on_their_own_line() {
        let error = error("fn broken() {\n  return nope\n}\nbroken()");
        assert_eq!(error.line, 2);
        assert_eq!(error.message, "'nope' is not defined");
    }

    #[test]
    fn builds_and_reads_lists() {
        assert_eq!(number("let xs = [10, 20, 30]\nxs[1]"), 20.0);
        assert_eq!(number("len([1, 2, 3]) + len(\"four\")"), 7.0);
        assert_eq!(
            string("\"${[1, \"two\", [true]]}\""),
            "[1, \"two\", [true]]"
        );
        assert_eq!(string("\"hello\"[1]"), "e");
        assert_eq!(
            number("let grid = [\n  [1, 2],\n  [3, 4],\n]\ngrid[1][0]"),
            3.0
        );
    }

    #[test]
    fn changes_lists_in_place() {
        assert_eq!(
            number("let xs = [1, 2]\npush(xs, 3)\nxs[0] = 9\nxs[0] + xs[2]"),
            12.0
        );
        assert_eq!(number("let a = [1]\nlet b = a\npush(b, 2)\nlen(a)"), 2.0);
    }

    #[test]
    fn compares_lists_by_contents() {
        assert!(boolean("[1, [2, \"x\"]] == [1, [2, \"x\"]]"));
        assert!(boolean("[1, 2] != [2, 1]"));
        assert!(boolean("range(3) == [0, 1, 2]"));
        assert!(boolean("range(2, 5) == [2, 3, 4]"));
    }

    #[test]
    fn loops_over_lists_and_strings() {
        assert_eq!(
            number("let total = 0\nfor n in [1, 2, 3, 4] {\n  total = total + n\n}\ntotal"),
            10.0
        );
        assert_eq!(
            string("let out = \"\"\nfor c in \"abc\" {\n  out = c + out\n}\nout"),
            "cba"
        );
    }

    #[test]
    fn breaks_and_continues() {
        let source = "let total = 0\nfor n in range(10) {\n  if n == 7 {\n    break\n  }\n  if n % 2 == 0 {\n    continue\n  }\n  total = total + n\n}\ntotal";
        assert_eq!(number(source), 1.0 + 3.0 + 5.0);
        let source = "let i = 0\nwhile true {\n  i = i + 1\n  if i == 5 {\n    break\n  }\n}\ni";
        assert_eq!(number(source), 5.0);
    }

    #[test]
    fn each_loop_round_gets_its_own_variable() {
        let source = "let getters = []\nfor i in range(3) {\n  push(getters, fn() { return i })\n}\ngetters[1]()";
        assert_eq!(number(source), 1.0);
    }

    #[test]
    fn reports_list_mistakes() {
        assert_eq!(
            error("[1, 2][5]").message,
            "index 5 is out of range, the length is 2"
        );
        assert_eq!(
            error("[1, 2][0.5]").message,
            "index must be a whole number, got 0.5"
        );
        assert_eq!(
            error("[1][\"a\"]").message,
            "index must be a number, got string"
        );
        assert_eq!(
            error("let s = \"hi\"\ns[0] = \"y\"").message,
            "strings can't be changed in place"
        );
        assert_eq!(
            error("for x in 5 {\n}").message,
            "for needs a list or a string, got number"
        );
        assert_eq!(
            error("break").message,
            "'break' can only be used inside a loop"
        );
        assert_eq!(
            error("while true {\n  fn f() {\n    continue\n  }\n}").message,
            "'continue' can only be used inside a loop"
        );
        assert_eq!(error("push(1, 2)").message, "push needs a list, got number");
        assert_eq!(error("len()").message, "len expects 1 argument, got 0");
        assert_eq!(
            error("[1] == 1").message,
            "cannot use '==' on list<number> and number"
        );
    }

    #[test]
    fn builds_and_reads_records() {
        let person = "let ada = { name: \"Ada\", born: 1815, langs: [\"math\"] }\n";
        assert_eq!(string(&format!("{person}ada.name")), "Ada");
        assert_eq!(
            number(&format!("{person}ada.born + len(ada.langs)")),
            1816.0
        );
        assert_eq!(number(&format!("{person}ada[\"born\"]")), 1815.0);
        assert_eq!(
            string(&format!("{person}\"${{ada}}\"")),
            "{name: \"Ada\", born: 1815, langs: [\"math\"]}"
        );
        assert_eq!(number("let p = {\n  x: 1,\n  y: { z: 2 },\n}\np.y.z"), 2.0);
        assert_eq!(number("len({ a: 1, b: 2 }) + len({})"), 2.0);
    }

    #[test]
    fn changes_existing_fields() {
        assert_eq!(
            number("let p = { age: 36 }\np.age = p.age + 1\np.age"),
            37.0
        );
        assert_eq!(number("let p = { age: 36 }\np[\"age\"] = 40\np.age"), 40.0);
        assert_eq!(number("let a = { n: 1 }\nlet b = a\nb.n = 2\na.n"), 2.0);
    }

    #[test]
    fn lists_keys_and_compares_records() {
        assert!(boolean("keys({ b: 1, a: 2 }) == [\"b\", \"a\"]"));
        assert!(boolean("{ a: 1, b: [2] } == { b: [2], a: 1 }"));
        assert!(boolean("{ a: 1 } != { a: 2 }"));
        assert!(boolean("{ a: 1 } != { a: 1, b: 2 }"));
    }

    #[test]
    fn records_can_hold_functions() {
        let source = "let math = { square: fn(n) { return n * n } }\nmath.square(7)";
        assert_eq!(number(source), 49.0);
    }

    #[test]
    fn reports_record_mistakes() {
        assert_eq!(
            error("let p = { age: 1 }\np.agee").message,
            "record has no field 'agee'"
        );
        assert!(
            error("let p = { age: 1 }\np.email = \"x\"")
                .message
                .starts_with("record has no field 'email', fields can only be added")
        );
        assert_eq!(error("{ a: 1, a: 2 }").message, "field 'a' is listed twice");
        assert_eq!(
            error("let n = 5\nn.size").message,
            "cannot read field 'size' of number"
        );
        assert!(
            error("for k in { a: 1 } {\n}")
                .message
                .contains("keys(record)")
        );
        assert_eq!(
            error("{ a: 1 }[0]").message,
            "record fields are named with strings, got number"
        );
    }

    #[test]
    fn none_is_the_one_empty_value() {
        assert!(boolean("none == none"));
        assert!(!boolean("5 == none"));
        assert!(boolean("\"a\" != none"));
        assert!(boolean("fn nothing() {}\nnothing() == none"));
        assert_eq!(string("\"${none}\""), "none");
    }

    #[test]
    fn falls_back_with_double_question_mark() {
        assert_eq!(number("none ?? 5"), 5.0);
        assert_eq!(number("3 ?? 5"), 3.0);
        assert_eq!(number("0 ?? 5"), 0.0);
        assert_eq!(number("1 ?? nope"), 1.0);
        assert_eq!(number("none ?? none ?? 7"), 7.0);
    }

    #[test]
    fn reads_fields_safely_with_question_dot() {
        let empty = "let user = { address: none }\n";
        let full = "let user = { address: { city: \"Paris\" } }\n";
        assert!(boolean(&format!("{empty}user.address?.city == none")));
        assert_eq!(string(&format!("{full}user.address?.city")), "Paris");
        assert_eq!(
            string(&format!("{empty}user.address?.city ?? \"unknown\"")),
            "unknown"
        );
    }

    #[test]
    fn none_is_not_falsy_or_a_number() {
        assert_eq!(
            error("if none {\n}").message,
            "if needs true or false, got none"
        );
        assert_eq!(
            error("none + 1").message,
            "cannot use '+' on none and number"
        );
        assert_eq!(
            error("let user = { address: none }\nuser.address.city").message,
            "cannot read field 'city' of none, use '?.city' if it might be none"
        );
        assert_eq!(error("5?.x").message, "cannot read field 'x' of number");
    }

    fn run_files(name: &str, files: &[(&str, &str)]) -> Result<Option<Value>, Error> {
        let dir = std::env::temp_dir().join(format!("khipu-test-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (file, source) in files {
            std::fs::write(dir.join(file), source).unwrap();
        }
        let main = std::fs::read_to_string(dir.join("main.kp")).unwrap();
        let program = Parser::new(lex(&main)?).parse_program()?;
        let result = Interpreter::in_dir(&dir).run(&program);
        std::fs::remove_dir_all(&dir).unwrap();
        result
    }

    const GEOMETRY: &str = "let pi = 3\nfn area(r) {\n  return pi * r * r\n}\n";

    #[test]
    fn imports_a_whole_module() {
        let result = run_files(
            "whole",
            &[
                ("geometry.kp", GEOMETRY),
                (
                    "main.kp",
                    "import geometry from \"geometry.kp\"\ngeometry.area(2) + geometry.pi",
                ),
            ],
        );
        assert!(matches!(result.unwrap(), Some(Value::Number(n)) if n == 15.0));
    }

    #[test]
    fn imports_chosen_names() {
        let result = run_files(
            "names",
            &[
                ("geometry.kp", GEOMETRY),
                (
                    "main.kp",
                    "import { area, pi } from \"geometry.kp\"\narea(1) + pi",
                ),
            ],
        );
        assert!(matches!(result.unwrap(), Some(Value::Number(n)) if n == 6.0));
    }

    #[test]
    fn runs_each_module_once() {
        let main =
            "import a from \"state.kp\"\nimport b from \"state.kp\"\na.box.count = 5\nb.box.count";
        let result = run_files(
            "once",
            &[("state.kp", "let box = { count: 0 }\n"), ("main.kp", main)],
        );
        assert!(matches!(result.unwrap(), Some(Value::Number(n)) if n == 5.0));
    }

    #[test]
    fn resolves_paths_from_the_importing_file() {
        let result = run_files(
            "nested",
            &[
                ("helper.kp", "let answer = 42\n"),
                (
                    "middle.kp",
                    "import { answer } from \"helper.kp\"\nlet doubled = answer * 2\n",
                ),
                ("main.kp", "import { doubled } from \"middle.kp\"\ndoubled"),
            ],
        );
        assert!(matches!(result.unwrap(), Some(Value::Number(n)) if n == 84.0));
    }

    #[test]
    fn reports_module_mistakes() {
        let missing_name = run_files(
            "missing-name",
            &[
                ("geometry.kp", GEOMETRY),
                ("main.kp", "import { volume } from \"geometry.kp\""),
            ],
        );
        assert_eq!(
            missing_name.unwrap_err().message,
            "geometry.kp has no 'volume'"
        );

        let missing_file = run_files(
            "missing-file",
            &[("main.kp", "\nimport nope from \"nope.kp\"")],
        );
        let error = missing_file.unwrap_err();
        assert_eq!(
            (error.line, error.message.as_str()),
            (2, "cannot find nope.kp")
        );

        let broken = run_files(
            "broken",
            &[
                ("broken.kp", "let a = 1\nlet b = nope\n"),
                ("main.kp", "import broken from \"broken.kp\""),
            ],
        );
        let error = broken.unwrap_err();
        assert_eq!(
            (error.line, error.message.as_str()),
            (2, "'nope' is not defined")
        );
        assert!(error.file.unwrap().ends_with("broken.kp"));

        let looped = run_files(
            "loop",
            &[
                ("a.kp", "import b from \"b.kp\"\n"),
                ("b.kp", "import a from \"a.kp\"\n"),
                ("main.kp", "import a from \"a.kp\""),
            ],
        );
        assert!(
            looped
                .unwrap_err()
                .message
                .contains("a.kp is part of an import loop")
        );
    }

    #[test]
    fn updates_variables_in_place_safely() {
        assert_eq!(
            number("let i = 0\ni = i + 1\ni = i - 3\ni = i + 10\ni"),
            8.0
        );
        assert_eq!(
            number("fn f() {\n  let n = 5\n  n = n + 2\n  n = n - 1\n  return n\n}\nf()"),
            6.0
        );
        assert_eq!(
            string("let s = \"a\"\nlet t = s\ns = s + \"b\"\nt + \"/\" + s"),
            "a/ab"
        );
        assert_eq!(
            string(
                "fn f() {\n  let s = \"x\"\n  let keep = s\n  s = s + \"y\"\n  return keep + s\n}\nf()"
            ),
            "xxy"
        );
        let source = "fn counter() {\n  let c = 0\n  return fn() {\n    c = c + 1\n    return c\n  }\n}\nlet next = counter()\nnext()\nnext()";
        assert_eq!(number(source), 2.0);
    }

    #[test]
    fn failed_updates_keep_the_old_value() {
        let mut interpreter = Interpreter::new();
        let setup = Parser::new(lex("let x = 1").unwrap())
            .parse_program()
            .unwrap();
        interpreter.run(&setup).unwrap();
        let bad = Parser::new(lex("x = x + true").unwrap())
            .parse_program()
            .unwrap();
        assert!(interpreter.run(&bad).is_err());
        let read = Parser::new(lex("x").unwrap()).parse_program().unwrap();
        assert!(matches!(interpreter.run(&read).unwrap(), Some(Value::Number(n)) if n == 1.0));
    }

    #[test]
    fn prints_numbers_like_before() {
        assert_eq!(
            string("\"${12} ${-7} ${0} ${2.5} ${1 / 3} ${-0.5}\""),
            "12 -7 0 2.5 0.3333333333333333 -0.5"
        );
        assert_eq!(
            string("\"${1000000000000000} ${0 - 0}\""),
            "1000000000000000 0"
        );
    }

    #[test]
    fn accepts_type_annotations() {
        assert_eq!(number("let age: number = 36\nage + 1"), 37.0);
        assert_eq!(string("let name: string = \"Ada\"\nname"), "Ada");
        let source = "fn add(a: number, b: number) -> number {\n  return a + b\n}\nadd(2, 3)";
        assert_eq!(number(source), 5.0);
    }

    #[test]
    fn rejects_mismatched_annotations() {
        assert_eq!(
            error("let age: number = \"old\"").message,
            "'age' should be number, got string"
        );
        assert_eq!(
            error("let ok: bool = 1").message,
            "'ok' should be bool, got number"
        );
    }

    #[test]
    fn infers_types_from_the_first_value() {
        let error = error("let count = 0\ncount = \"zero\"");
        assert_eq!(
            (error.line, error.message.as_str()),
            (2, "'count' should be number, got string")
        );
        assert_eq!(number("let n = 1\nn = n + 1\nn"), 2.0);
    }

    #[test]
    fn checks_types_before_anything_runs() {
        let error = error("let first = [1][5]\nlet second: number = \"x\"");
        assert_eq!(error.line, 2);
        assert_eq!(error.message, "'second' should be number, got string");
    }

    #[test]
    fn checks_function_signatures() {
        let add = "fn add(a: number, b: number) -> number {\n  return a + b\n}\n";
        assert_eq!(
            error(&format!("{add}add(1, \"2\")")).message,
            "argument 2 of 'add' should be number, got string"
        );
        assert_eq!(
            error("fn name() -> string {\n  return 5\n}").message,
            "'name' should return string, got number"
        );
        assert_eq!(
            error("fn sign(x: number) -> number {\n  if x > 0 {\n    return 1\n  }\n}").message,
            "'sign' should return number, but can reach the end without returning"
        );
        let both = "fn sign(x: number) -> number {\n  if x > 0 {\n    return 1\n  } else {\n    return -1\n  }\n}\nsign(-5)";
        assert_eq!(number(both), -1.0);
    }

    #[test]
    fn nullable_types_need_a_fallback() {
        let message = error("let nick: string? = none\nlet shown: string = nick").message;
        assert_eq!(
            message,
            "'shown' should be string, got string? (it might be none, use ?? to give a fallback)"
        );
        assert_eq!(
            string("let nick: string? = none\nlet shown: string = nick ?? \"anon\"\nshown"),
            "anon"
        );
        assert!(
            error("let n: number = none")
                .message
                .contains("'n' should be number, got none")
        );
        assert_eq!(
            error("fn find() -> record? {\n  return none\n}\nfind().name").message,
            "cannot read field 'name' of a value that might be none, use '?.name'"
        );
    }

    #[test]
    fn checks_function_types() {
        let twice =
            "fn twice(f: fn(number) -> number, x: number) -> number {\n  return f(f(x))\n}\n";
        assert_eq!(
            number(&format!(
                "{twice}twice(fn(n: number) -> number {{ return n * 2 }}, 3)"
            )),
            12.0
        );
        assert_eq!(
            error(&format!(
                "{twice}twice(fn(s: string) -> string {{ return s }}, 3)"
            ))
            .message,
            "argument 1 of 'twice' should be fn(number) -> number, got fn(string) -> string"
        );
    }

    #[test]
    fn untyped_code_stays_flexible() {
        assert_eq!(number("fn id(x) {\n  return x\n}\nid(1) + id(2)"), 3.0);
        assert_eq!(
            string("fn id(x) {\n  return x\n}\nid(\"a\") + id(\"b\")"),
            "ab"
        );
    }

    #[test]
    fn knows_functions_declared_later() {
        let source = "fn main() {\n  return helper(1)\n}\nfn helper(n: number) -> number {\n  return n\n}\nmain()";
        assert_eq!(number(source), 1.0);
        let bad = "fn main() {\n  return helper(\"x\")\n}\nfn helper(n: number) -> number {\n  return n\n}";
        assert_eq!(
            error(bad).message,
            "argument 1 of 'helper' should be number, got string"
        );
    }

    #[test]
    fn the_prompt_remembers_types_and_recovers_from_mistakes() {
        let mut interpreter = Interpreter::new();
        let mut run = |source: &str| {
            let program = Parser::new(lex(source).unwrap()).parse_program().unwrap();
            interpreter.run(&program)
        };
        run("let x: number = 1").unwrap();
        assert_eq!(
            run("x = \"s\"").unwrap_err().message,
            "'x' should be number, got string"
        );
        assert!(matches!(run("x").unwrap(), Some(Value::Number(n)) if n == 1.0));
    }

    const POINT: &str = "struct Point {\n  x: number,\n  y: number,\n\n  fn length_squared(self) -> number {\n    return self.x * self.x + self.y * self.y\n  }\n\n  fn moved(self, dx: number) -> Point {\n    return Point { x: self.x + dx, y: self.y }\n  }\n\n  fn origin() -> Point {\n    return Point { x: 0, y: 0 }\n  }\n}\n";

    #[test]
    fn builds_structs_and_calls_methods() {
        assert_eq!(
            number(&format!(
                "{POINT}let p = Point {{ x: 3, y: 4 }}\np.length_squared()"
            )),
            25.0
        );
        assert_eq!(
            number(&format!("{POINT}Point {{ y: 2, x: 1 }}.moved(10).x")),
            11.0
        );
        assert_eq!(
            number(&format!("{POINT}Point.origin().length_squared()")),
            0.0
        );
        assert_eq!(
            string(&format!("{POINT}\"${{Point {{ x: 1, y: 2 }}}}\"")),
            "Point { x: 1, y: 2 }"
        );
        assert!(boolean(&format!(
            "{POINT}Point {{ x: 1, y: 2 }} == Point {{ x: 1, y: 2 }}"
        )));
        assert_eq!(
            number(&format!(
                "{POINT}let p = Point {{ x: 1, y: 2 }}\np.x = 9\np.x"
            )),
            9.0
        );
    }

    #[test]
    fn checks_struct_usage() {
        assert_eq!(
            error(&format!("{POINT}Point {{ x: 1 }}")).message,
            "Point is missing field 'y'"
        );
        assert_eq!(
            error(&format!("{POINT}Point {{ x: 1, y: 2, z: 3 }}")).message,
            "Point has no field 'z'"
        );
        assert_eq!(
            error(&format!("{POINT}Point {{ x: \"1\", y: 2 }}")).message,
            "'Point.x' should be number, got string"
        );
        assert_eq!(
            error(&format!("{POINT}Point.origin().z")).message,
            "Point has no field 'z'"
        );
        assert_eq!(
            error(&format!("{POINT}Point.origin().moved(\"far\")")).message,
            "argument 1 of 'moved' should be number, got string"
        );
        assert_eq!(
            error(&format!("{POINT}let p: Point = Point.origin()\np = 5")).message,
            "'p' should be Point, got number"
        );
        assert_eq!(
            error("struct Box {\n  size: number,\n  fn get(self) -> string {\n    return self.size\n  }\n}").message,
            "'Box.get' should return string, got number"
        );
    }

    const SHAPE: &str = "enum Shape {\n  Circle(number),\n  Rect(number, number),\n  Empty,\n}\n";

    #[test]
    fn matches_enums() {
        let area = "fn area(s: Shape) -> number {\n  return match s {\n    Shape.Circle(r) => 3 * r * r,\n    Shape.Rect(w, h) => w * h,\n    Shape.Empty => 0,\n  }\n}\n";
        assert_eq!(
            number(&format!(
                "{SHAPE}{area}area(Shape.Rect(2, 5)) + area(Shape.Circle(1)) + area(Shape.Empty)"
            )),
            13.0
        );
        assert_eq!(
            string(&format!(
                "{SHAPE}\"${{Shape.Rect(2, 5)}} ${{Shape.Empty}}\""
            )),
            "Shape.Rect(2, 5) Shape.Empty"
        );
        assert!(boolean(&format!(
            "{SHAPE}Shape.Circle(2) == Shape.Circle(2)"
        )));
        assert!(!boolean(&format!("{SHAPE}Shape.Circle(2) == Shape.Empty")));
    }

    #[test]
    fn match_needs_every_case() {
        let partial = "let s = Shape.Empty\nmatch s {\n  Shape.Circle(r) => print(r),\n  Shape.Empty => print(0),\n}";
        assert_eq!(
            error(&format!("{SHAPE}{partial}")).message,
            "match on Shape is missing Shape.Rect"
        );
        let wildcard = "let s = Shape.Rect(1, 2)\nlet label = match s {\n  Shape.Circle(r) => \"round\",\n  _ => \"other\",\n}\nlabel";
        assert_eq!(string(&format!("{SHAPE}{wildcard}")), "other");
        assert_eq!(
            error("let n = 5\nmatch n {\n  1 => print(1),\n}").message,
            "match on number needs a '_' arm for the other values"
        );
        assert_eq!(
            error(&format!("{SHAPE}let s = Shape.Empty\nmatch s {{\n  Shape.Rect(w) => print(w),\n  _ => print(0),\n}}")).message,
            "Shape.Rect holds 2 values, but the pattern names 1"
        );
    }

    #[test]
    fn matches_plain_values() {
        let source = "fn describe(n: number) -> string {\n  return match n {\n    0 => \"zero\",\n    1 => \"one\",\n    other => \"many: ${other}\",\n  }\n}\ndescribe(0) + \" \" + describe(1) + \" \" + describe(7)";
        assert_eq!(string(source), "zero one many: 7");
        let blocks = "let total = 0\nfor n in [1, 2, 3] {\n  match n % 2 == 0 {\n    true => {\n      total = total + 10\n    }\n    false => {\n      total = total + 1\n    }\n  }\n}\ntotal";
        assert_eq!(number(blocks), 12.0);
    }

    #[test]
    fn checks_generic_lists_and_functions() {
        let first = "fn first<T>(items: list<T>) -> T {\n  return items[0]\n}\n";
        assert_eq!(number(&format!("{first}first([4, 5]) + 1")), 5.0);
        assert_eq!(
            error(&format!("{first}first([\"a\"]) + 1")).message,
            "cannot use '+' on string and number, use \"${...}\" to put values inside a string"
        );
        assert_eq!(
            error("let names: list<string> = [\"a\"]\npush(names, 5)").message,
            "cannot push number into list<string>"
        );
        assert_eq!(
            error("let xs: list<number> = [\"a\", \"b\"]").message,
            "'xs' should be list<number>, got list<string>"
        );
        let typed = "let total = 0\nlet xs: list<number> = [1, 2, 3]\nfor x in xs {\n  total = total + x\n}\ntotal";
        assert_eq!(number(typed), 6.0);
        assert_eq!(
            error("for x in [\"a\"] {\n  let n: number = x\n}").message,
            "'n' should be number, got string"
        );
    }

    #[test]
    fn struct_literals_do_not_confuse_conditions() {
        let source = "let Ready = true\nlet count = 0\nif Ready {\n  count = 1\n}\nwhile Ready {\n  Ready = false\n}\ncount";
        assert_eq!(number(source), 1.0);
    }
}
