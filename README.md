<p align="center">
  <img src="editors/vscode/icons/khipu.svg" width="120" alt="Khipu logo">
</p>

<h1 align="center">Khipu</h1>

<p align="center">
  <strong>JavaScript, minus the bad parts, plus real types and real multi-core.</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/written%20in-Rust-B7410E?logo=rust&logoColor=white" alt="Written in Rust">
  <img src="https://img.shields.io/badge/status-early%20development-FB923C" alt="Status: early development">
  <img src="https://img.shields.io/badge/files-.kp-E8A23A" alt="File extension: .kp">
</p>

---

Khipu is a programming language that keeps the feel of JavaScript and drops the parts people complain about: no silent type coercion, no truthiness, no `null` *and* `undefined`. Next come C#-style static types, and its signature feature, safe parallelism across every CPU core with lightweight tasks and channels.

The name comes from the **khipu**, the knotted cords the Inca used to record numbers and data: many strings working side by side, which is the idea behind Khipu's concurrency.

## A taste

```js
let n = 2

while n <= 30 {
    let d = 2
    let prime = true

    while d * d <= n {
        if n % d == 0 {
            prime = false
        }
        d = d + 1
    }

    if prime {
        print(n)
    }
    n = n + 1
}
```

```
$ khipu examples/primes.kp
2
3
5
7
...
29
```

## Strict where JavaScript is loose

| JavaScript | Khipu |
|---|---|
| `if (1) { ... }` runs, because `1` is "truthy" | `if 1 { ... }` is an error: *if needs true or false, got number* |
| `1 + true` is `2` | `1 + true` is an error: *cannot use '+' on number and bool* |
| `1 == true` is `true` | `1 == true` is an error: you can't compare different types |
| A typo'd variable is `undefined` | `print(totl)` is an error: *'totl' is not defined* |
| `x = 5` without declaring creates a global | `x = 5` is an error until you write `let x = ...` |
| `let count = 0` then `count = "zero"` is fine | A type error, *'count' should be number, got string*, caught before the program runs |
| `user.name` crashes at runtime when `user` is `null` | `find_user()` returning `record?` must be handled with `?.` or `??` first |

## Getting started

You need [Rust](https://rustup.rs).

```bash
git clone git@github.com:psychedelicdevx/khipu.git
cd khipu
cargo install --path .
```

Run a file:

```bash
khipu examples/primes.kp
```

Or start the interactive prompt by running `khipu` with no file. Blocks can span several lines; it waits until your `{ }` are closed.

## Roadmap

| Phase | Progress |
|---|---|
| 0. Foundations | `██████████` 12 / 12 |
| 1. Core language | `██████████` 10 / 10 |
| 2. Speed | `██████████` 7 / 7 |
| 3. Types | `██████████` 7 / 7 |
| 4. Benchmarks | `░░░░░░░░░░` 0 / 8 |
| 5. Tooling | `██░░░░░░░░` 2 / 9 |
| 6. Concurrency | `░░░░░░░░░░` 0 / 5 |
| 7. Ecosystem | `░░░░░░░░░░` 0 / 6 |
| 8. Native speed | `░░░░░░░░░░` 0 / 3 |

**Next up:** Median of 20+ runs with spread, instead of best of 5

The full list is in [ROADMAP.md](ROADMAP.md). The progress bars update themselves on every commit.

## Performance

| Benchmark | Khipu | Python | Node | vs Python | vs Node |
|---|---|---|---|---|---|
| closures | 15 ms | 34 ms | 24 ms | **2.2× faster** | **1.6× faster** |
| fib | 16 ms | 24 ms | 23 ms | **1.5× faster** | **1.4× faster** |
| lists | 12 ms | 37 ms | 26 ms | **3.0× faster** | **2.1× faster** |
| loop | 33 ms | 192 ms | 33 ms | **5.8× faster** | 1.0× slower |
| records | 22 ms | 42 ms | 23 ms | **1.9× faster** | **1.1× faster** |
| strings | 23 ms | 39 ms | 25 ms | **1.7× faster** | **1.1× faster** |

Best of 5 runs, including start-up, on Apple M5. Run `python3 scripts/bench.py` to measure again.

Every benchmark in `benchmarks/` exists in Khipu, Python and JavaScript, and the script checks that all three print the same answer before it reports a time.

## Editor support

`editors/vscode` is a VS Code extension with syntax highlighting, the `.kp` file icon, comment toggling and bracket handling. If you use the Symbols icon theme, run `editors/vscode/patch-symbols.sh` to give `.kp` files the KP icon there too.

## Project layout

| Path | What's there |
|---|---|
| `src/lexer.rs` | Turns source text into tokens |
| `src/parser.rs` | Turns tokens into a syntax tree |
| `src/checker.rs` | The static type checker, run before anything executes |
| `src/compiler.rs` | Turns the tree into bytecode, resolving every variable to a slot |
| `src/vm.rs` | Runs the bytecode: one tight loop, calls without native recursion |
| `src/value.rs` | Runtime values, operators and built-in functions |
| `src/interpreter.rs` | Ties it together, plus the language test suite |
| `src/main.rs` | The `khipu` command: run a file or open the prompt |
| `benchmarks/` | The same programs in Khipu, Python and JavaScript |
| `examples/` | Example Khipu programs |
| `editors/vscode/` | VS Code extension |
| `scripts/` | Repository tooling, like the roadmap sync |

## Development

Checks run locally on every commit instead of in paid CI. Turn them on once after cloning:

```bash
git config core.hooksPath .githooks
```

Each commit then syncs the roadmap progress and runs `cargo fmt --check`, `cargo clippy` and `cargo test`. If any of them fail, the commit is stopped.
