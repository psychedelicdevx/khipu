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

Mistakes are shown where they happen:

```
error: 'total' should be number, got string
 --> shop.kp:2
  |
2 | let total: number = "free"
  |     ^^^^^
```

Format files in place, or check them without changing anything:

```bash
khipu fmt examples/*.kp
khipu fmt --check examples/*.kp
```

Write tests next to your code and run them with `khipu test`. A normal run skips them.

```
fn add(a: number, b: number) -> number {
    return a + b
}

test "adds numbers" {
    assert_eq(add(2, 3), 5)
    assert(add(-1, 1) == 0, "should cancel out")
}
```

```bash
khipu test examples/testing.kp
```

## Roadmap

| Phase | Progress |
|---|---|
| 0. Foundations | `██████████` 12 / 12 |
| 1. Core language | `██████████` 10 / 10 |
| 2. Speed | `██████████` 7 / 7 |
| 3. Types | `██████████` 7 / 7 |
| 4. Benchmarks | `██████████` 8 / 8 |
| 5. Tooling | `██████████` 8 / 8 |
| 6. Concurrency | `░░░░░░░░░░` 0 / 5 |
| 7. Ecosystem | `░░░░░░░░░░` 0 / 6 |
| 8. Performance and final audit | `░░░░░░░░░░` 0 / 15 |
| 9. Documentation | `░░░░░░░░░░` 0 / 5 |

**Next up:** `spawn` lightweight tasks

The full list is in [ROADMAP.md](ROADMAP.md). The progress bars update themselves on every commit.

## Performance

Sustained runs use inputs large enough that start-up is a small share of the time. Median of 5 runs on Apple M5; lower is better.

| Benchmark | Khipu | Python | Node | C | Rust | vs Python | vs Node |
|---|---|---|---|---|---|---|---|
| fib | 148 ms | 136 ms | 35 ms | 6.1 ms | 6.1 ms | 1.1× slower | 4.3× slower |
| fib_iterative | 621 ms | 467 ms | 45 ms | 9.7 ms | 10 ms | 1.3× slower | 13.7× slower |
| loop | 310 ms | 2294 ms | 156 ms | 2.7 ms | 2.9 ms | **7.4× faster** | 2.0× slower |
| nested_loops | 178 ms | 234 ms | 28 ms | 4.4 ms | 4.7 ms | **1.3× faster** | 6.3× slower |
| sieve | 322 ms | 527 ms | 51 ms | 4.9 ms | 5.4 ms | **1.6× faster** | 6.4× slower |
| matrix | 150 ms | 272 ms | 30 ms | 3.3 ms | 4.8 ms | **1.8× faster** | 5.0× slower |
| mandelbrot | 76 ms | 225 ms | 26 ms | 3.8 ms | 4.4 ms | **3.0× faster** | 2.9× slower |
| lists | 114 ms | 286 ms | 78 ms | 5.1 ms | 6.1 ms | **2.5× faster** | 1.5× slower |
| records | 218 ms | 346 ms | 27 ms | 2.1 ms | 2.7 ms | **1.6× faster** | 8.1× slower |
| strings | 216 ms | 287 ms | 40 ms | 83 ms | 105 ms | **1.3× faster** | 5.4× slower |
| calls | 75 ms | 191 ms | 34 ms | 2.4 ms | 2.2 ms | **2.6× faster** | 2.2× slower |
| closures | 125 ms | 234 ms | 27 ms | 4.6 ms | 4.2 ms | **1.9× faster** | 4.6× slower |
| locals | 134 ms | 275 ms | 65 ms | 2.3 ms | 2.6 ms | **2.1× faster** | 2.1× slower |
| globals | 102 ms | 342 ms | 27 ms | 4.7 ms | 11 ms | **3.4× faster** | 3.7× slower |
| record_access | 287 ms | 819 ms | 25 ms | 4.9 ms | 6.5 ms | **2.9× faster** | 11.4× slower |
| struct_access | 270 ms | 719 ms | 26 ms | 5.1 ms | 6.6 ms | **2.7× faster** | 10.5× slower |

Start-up, an empty program (median of 21 runs): Khipu 2.6 ms · Python 13 ms · Node 22 ms · C 2.1 ms · Rust 2.4 ms.

On sustained runs Khipu beats Python on 14 of 16 benchmarks and Node on 0 of 16. Cold-start results, spreads, memory use and the method are in [BENCHMARKS.md](BENCHMARKS.md).

## Editor support

`khipu lsp` is a language server. Any editor that speaks LSP gets:

- errors as you type, underlined where the problem is
- hover to see the type of any name, field or method
- autocomplete for names, keywords, struct fields and methods, and enum variants, which keeps working while the line is half typed
- formatting, the same as `khipu fmt`

`editors/vscode` is a VS Code extension that uses it, plus syntax highlighting, the `.kp` file icon, comment toggling and bracket handling. It runs `khipu` from your `PATH`; set `khipu.path` if it lives somewhere else. To build it:

```bash
cargo install --path .
cd editors/vscode && npm install && npx @vscode/vsce package
code --install-extension khipu-*.vsix
```

If you use the Symbols icon theme, run `editors/vscode/patch-symbols.sh` to give `.kp` files the KP icon there too.

`editors/tree-sitter-khipu` is a tree-sitter grammar with highlight queries, for Zed, Neovim, Helix and anything else built on tree-sitter. It parses every example and benchmark in this repository; run its tests with `npx tree-sitter-cli test` in that folder.

## Playground

`playground/` runs Khipu in the browser. The interpreter is compiled to WebAssembly and runs in a web worker, so a long loop never freezes the page and Stop ends it. Errors are underlined as you type, Format tidies the code, and Copy link shares a program.

```bash
scripts/build-playground.sh
python3 -m http.server -d playground 8000
```

Then open http://localhost:8000. You need the WebAssembly target once: `rustup target add wasm32-unknown-unknown`.

## Project layout

| Path | What's there |
|---|---|
| `src/lexer.rs` | Turns source text into tokens |
| `src/error.rs` | Errors that show the line and point at the problem |
| `src/format.rs` | `khipu fmt`: one canonical layout, comments kept |
| `src/lsp.rs` | `khipu lsp`: the language server behind editor support |
| `src/parser.rs` | Turns tokens into a syntax tree |
| `src/checker.rs` | The static type checker, run before anything executes |
| `src/compiler.rs` | Turns the tree into bytecode, resolving every variable to a slot |
| `src/vm.rs` | Runs the bytecode: one tight loop, calls without native recursion |
| `src/value.rs` | Runtime values, operators and built-in functions |
| `src/interpreter.rs` | Ties it together, plus the language test suite |
| `src/lib.rs` | The library the `khipu` command and the playground share |
| `src/wasm.rs` | Browser entry points: run, check and format |
| `src/main.rs` | The `khipu` command: run a file, `fmt`, `test`, `lsp`, or open the prompt |
| `benchmarks/` | The same programs in Khipu, Python, JavaScript, C and Rust |
| `examples/` | Example Khipu programs |
| `editors/vscode/` | VS Code extension |
| `editors/tree-sitter-khipu/` | Tree-sitter grammar and highlight queries |
| `playground/` | The browser playground |
| `scripts/` | Repository tooling, like the roadmap sync |

## Development

Checks run locally on every commit instead of in paid CI. Turn them on once after cloning:

```bash
git config core.hooksPath .githooks
```

Each commit then syncs the roadmap progress and runs `cargo fmt --check`, `cargo clippy`, `cargo test`, `khipu fmt --check` on every `.kp` file and `khipu test`. If any of them fail, the commit is stopped.
