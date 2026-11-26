use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use khipu::format::format;
use khipu::interpreter::Interpreter;
use khipu::lexer::lex;
use khipu::parser::Parser;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len())]
    }
}

const UNSAFE_WORDS: [&str; 14] = [
    "spawn", "channel", "receive", "select", "Http", "input", "File", "Process", "import",
    "listen", "accept", "fetch", "exit", "respond",
];

const SNIPPETS: [&str; 8] = [
    "struct Point {\n    x: number,\n    y: number,\n\n    fn len(self) -> number {\n        return self.x * self.x + self.y * self.y\n    }\n}\nlet p = Point { x: 3, y: 4 }\nprint(p.len(), p == Point { x: 3, y: 4 }, \"${p}\")\n",
    "enum Shape {\n    Circle(number),\n    Rect(number, number),\n    Empty,\n}\nfn area(s: Shape) -> number {\n    return match s {\n        Shape.Circle(r) => 3 * r * r,\n        Shape.Rect(w, h) => w * h,\n        Shape.Empty => 0,\n    }\n}\nprint(area(Shape.Rect(2, 5)), area(Shape.Empty))\n",
    "fn counter() {\n    let count = 0\n    return fn() {\n        count = count + 1\n        return count\n    }\n}\nlet next = counter()\nnext()\nprint(next())\n",
    "fn divide(a: number, b: number) -> number\n    requires b != 0\n    ensures result * b == a\n    example divide(10, 2) == 5\n{\n    return a / b\n}\nprint(divide(9, 3))\n",
    "let xs: list = [1, true, \"s\", none, [2], { a: 1 }]\npush(xs, xs)\nprint(len(xs), xs[0] ?? 0, keys({ a: 1 }))\nlet r = { me: none }\nr.me = r\nprint(r == r)\n",
    "let words = \"  the quick brown fox  \".trim().split(\" \")\nlet long = words.filter(fn(w: string) -> bool { return w.len() > 3 })\nprint(long.map(fn(w: string) -> string { return w.upper() }).join(\", \"))\nprint([5, 3, 9].sorted(), [1, 2, 3].sum(), (2).pow(10), Math.sqrt(16))\n",
    "let user: record? = none\nprint(user?.name ?? \"nobody\")\nlet n = \"12\".to_number() ?? 0\nmatch n {\n    12 => print(\"twelve\"),\n    _ => print(\"other\"),\n}\ntest \"adds\" {\n    assert_eq(1 + 1, 2)\n}\n",
    "fn fib<T>(n: number) -> number {\n    if n < 2 {\n        return n\n    }\n    return fib(n - 1) + fib(n - 2)\n}\nlet total = 0\nfor i in range(10) {\n    total = total + fib(i)\n}\nwhile total > 0 {\n    total = total - 7\n    if total % 2 == 0 {\n        continue\n    }\n    break\n}\nprint(\"total: ${total}\")\n",
];

const ATOMS: [&str; 40] = [
    "(", ")", "{", "}", "[", "]", "\"", "${", "}", ",", ".", "?.", "??", "=", "==", "!=", "<",
    "<=", "+", "-", "*", "/", "%", "!", "&&", "||", ":", "->", "\n", " ", "let ", "fn ", "if ",
    "else ", "while ", "for ", "in ", "return ", "match ", "struct ",
];

fn seeds() -> Vec<String> {
    let mut seeds: Vec<String> = SNIPPETS.iter().map(|s| s.to_string()).collect();
    for folder in ["examples", "benchmarks"] {
        let mut stack = vec![Path::new(folder).to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "kp")
                    && let Ok(source) = fs::read_to_string(&path)
                    && !UNSAFE_WORDS.iter().any(|word| source.contains(word))
                {
                    seeds.push(source);
                }
            }
        }
    }
    seeds
}

fn mutate(rng: &mut Rng, seeds: &[String]) -> String {
    let mut source = seeds[rng.below(seeds.len())].clone();
    for _ in 0..1 + rng.below(4) {
        let len = source.len();
        let mut at = rng.below(len + 1);
        while !source.is_char_boundary(at) {
            at -= 1;
        }
        let mut end = (at + rng.below(24)).min(len);
        while !source.is_char_boundary(end) {
            end -= 1;
        }
        match rng.below(7) {
            0 => source.replace_range(at..end, ""),
            1 => {
                let piece = source[at..end].to_string();
                source.insert_str(at, &piece);
            }
            2 => source.insert_str(at, rng.pick(&ATOMS)),
            3 => {
                let other = &seeds[rng.below(seeds.len())];
                let lines: Vec<&str> = other.lines().collect();
                if !lines.is_empty() {
                    let line = format!("\n{}\n", rng.pick(&lines));
                    source.insert_str(at, &line);
                }
            }
            4 => {
                let number = ["0", "-1", "1e308", "0.5", "99999999999999999999", "2"];
                source.insert_str(at, rng.pick(&number));
            }
            5 => source.truncate(at),
            _ => {
                let mut lines: Vec<String> = source.lines().map(String::from).collect();
                if lines.len() > 1 {
                    let (a, b) = (rng.below(lines.len()), rng.below(lines.len()));
                    lines.swap(a, b);
                    source = lines.join("\n");
                }
            }
        }
    }
    source
}

fn exercise(source: &str) {
    let tokens = match lex(source) {
        Ok(tokens) => tokens,
        Err(_) => return,
    };
    let Ok(program) = Parser::new(tokens).parse_program() else {
        return;
    };
    if let Ok(once) = format(source) {
        let twice = format(&once).expect("formatted code must format again");
        assert_eq!(once, twice, "formatting must be stable");
    }
    let _ = Interpreter::new().run_with_budget(&program, 20);
}

fn fuzz(iterations: usize, seed: u64) {
    let seeds = seeds();
    let mut rng = Rng(seed | 1);
    for round in 0..iterations {
        let source = mutate(&mut rng, &seeds);
        if UNSAFE_WORDS.iter().any(|word| source.contains(word)) {
            continue;
        }
        let outcome = catch_unwind(AssertUnwindSafe(|| exercise(&source)));
        if outcome.is_err() {
            let path = std::env::temp_dir().join(format!("khipu-fuzz-crash-{round}.kp"));
            fs::write(&path, &source).unwrap();
            panic!(
                "round {round} crashed, the program is saved at {}",
                path.display()
            );
        }
    }
}

fn on_big_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn mutated_programs_never_crash() {
    on_big_stack(|| fuzz(1500, 20261126));
}

#[test]
#[ignore]
fn mutated_programs_never_crash_long() {
    let iterations = std::env::var("KHIPU_FUZZ")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(100_000);
    let seed = std::env::var("KHIPU_FUZZ_SEED")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(1);
    on_big_stack(move || fuzz(iterations, seed));
}
