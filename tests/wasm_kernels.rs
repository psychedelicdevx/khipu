use std::process::Command;

use khipu::interpreter::{Interpreter, Value};
use khipu::kernel::{group_all, top_level};
use khipu::lexer::lex;
use khipu::parser::Parser;
use khipu::wasmgen::module;

const PROGRAM: &str = "fn fib(n) {\n    if n < 2 {\n        return n\n    }\n    return fib(n - 1) + fib(n - 2)\n}\nfn work(n) {\n    let a = 0\n    let b = 1\n    let i = 0\n    while i < n {\n        a = a + b\n        b = b + 1\n        i = i + 1\n    }\n    return a % 1000\n}\nfn rem(a, b) {\n    return a % b\n}\nfn is_even(n: number) -> bool {\n    if n == 0 {\n        return true\n    }\n    return is_odd(n - 1)\n}\nfn is_odd(n: number) -> bool {\n    if n == 0 {\n        return false\n    }\n    return is_even(n - 1)\n}\nfn pick(flag: bool, a: number, b: number) -> number {\n    if flag && a > b || !flag && a < b {\n        return a\n    }\n    return b\n}\nfn count(limit: number) -> number {\n    let total = 0\n    let i = 0\n    while true {\n        i = i + 1\n        if i > limit {\n            break\n        }\n        if i % 2 == 0 {\n            continue\n        }\n        total = total + i\n    }\n    return total\n}\nfn forever(n) {\n    return forever(n + 1)\n}\nfn shout(s) {\n    return s + \"!\"\n}";

const CALLS: &[(&str, &[f64])] = &[
    ("fib", &[20.0]),
    ("work", &[1000.0]),
    ("rem", &[7.0, 3.0]),
    ("rem", &[-7.0, 3.0]),
    ("rem", &[7.5, 2.0]),
    ("rem", &[1e300, 7.0]),
    ("rem", &[5.0, 0.0]),
    ("rem", &[-4.0, 2.0]),
    ("rem", &[-0.0, 3.0]),
    ("rem", &[4.0, f64::INFINITY]),
    ("rem", &[1e300, 1e-300]),
    ("rem", &[123456.789, 0.001]),
    ("rem", &[-1e300, 7.0]),
    ("rem", &[f64::INFINITY, 7.0]),
    ("is_even", &[11.0]),
    ("is_odd", &[11.0]),
    ("pick", &[1.0, 3.0, 2.0]),
    ("pick", &[0.0, 3.0, 2.0]),
    ("count", &[10.0]),
];

fn interpreted(call: &str) -> String {
    let source = format!("{PROGRAM}\n{call}");
    let block = Parser::new(lex(&source).unwrap()).parse_program().unwrap();
    match Interpreter::new().run_main(&block).unwrap() {
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => f64::from(b).to_string(),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn emitted_webassembly_matches_the_interpreter() {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("node is not installed, skipping the WebAssembly check");
        return;
    }
    let block = Parser::new(lex(PROGRAM).unwrap()).parse_program().unwrap();
    let sources = top_level(&block);
    let kernels = group_all(&sources, &[], &[], true);
    let names: Vec<&str> = kernels.iter().map(|k| k.name.as_str()).collect();
    assert!(names.contains(&"fib") && names.contains(&"forever"));
    assert!(
        !names.contains(&"shout"),
        "a string function must not qualify"
    );
    let (bytes, exports) = module(&kernels).unwrap();

    let dir = std::env::temp_dir().join(format!("khipu-wasm-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("kernels.wasm");
    std::fs::write(&path, &bytes).unwrap();

    let index_of = |name: &str| exports.iter().position(|e| e.name == name).unwrap();
    let mut requests = Vec::new();
    for (name, args) in CALLS {
        let shown: Vec<String> = args
            .iter()
            .map(|a| {
                if a.is_infinite() {
                    "Infinity".to_string()
                } else {
                    format!("{a:?}")
                }
            })
            .collect();
        requests.push(format!("[{}, [{}]]", index_of(name), shown.join(",")));
    }
    requests.push(format!("[{}, [0]]", index_of("forever")));
    let script = format!(
        "const bytes = require('fs').readFileSync(process.argv[1]);\
         const m = new WebAssembly.Module(bytes);\
         const i = new WebAssembly.Instance(m, {{}});\
         const out = [{}].map(([k, args]) => {{\
             const r = i.exports['k' + k](...args);\
             const e = i.exports.error.value; i.exports.error.value = 0;\
             return (Object.is(r, -0) ? '-0' : String(r)) + '|' + e;\
         }});\
         console.log(out.join('\\n'));",
        requests.join(",")
    );
    let output = Command::new("node")
        .arg("-e")
        .arg(&script)
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "node failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let lines: Vec<&str> = std::str::from_utf8(&output.stdout)
        .unwrap()
        .lines()
        .collect();
    assert_eq!(lines.len(), CALLS.len() + 1);
    for ((name, args), line) in CALLS.iter().zip(&lines) {
        let shown: Vec<String> = args
            .iter()
            .zip(
                sources
                    .iter()
                    .find(|s| s.name == *name)
                    .unwrap()
                    .param_types
                    .iter(),
            )
            .map(|(a, ty)| match ty {
                Some(khipu::parser::Type::Bool) => (*a != 0.0).to_string(),
                _ if a.is_infinite() => "1 / 0".to_string(),
                _ if *a == 0.0 && a.is_sign_negative() => "-0".to_string(),
                _ => a.to_string(),
            })
            .collect();
        let call = format!("{name}({})", shown.join(", "));
        let want = interpreted(&call);
        let (got, error) = line.split_once('|').unwrap();
        let same = got.parse::<f64>().unwrap().to_bits() == want.parse::<f64>().unwrap().to_bits();
        assert!(
            same && error == "0",
            "{call}: wasm {got} (error {error}), interpreter {want}"
        );
    }
    assert_eq!(
        lines[CALLS.len()],
        "NaN|1",
        "runaway recursion must set the error flag"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
