<p align="center">
  <img src="editors/vscode/icons/khipu.svg" width="120" alt="Khipu logo">
</p>

<h1 align="center">Khipu</h1>

<p align="center">
  <strong>The smart adapt to the world. The fool tries to change it.</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/written%20in-Rust-B7410E?logo=rust&logoColor=white" alt="Written in Rust">
  <img src="https://img.shields.io/badge/status-early%20development-FB923C" alt="Status: early development">
  <img src="https://img.shields.io/badge/files-.kp-E8A23A" alt="File extension: .kp">
</p>

---

Every tool you trust was a fool's idea first. The smart see the world as it is and fit themselves to it. The fool decides the world is wrong, tries to fix it, fails in front of everyone, and tries again. That is the only way anything new has ever been made, and it is what this repository is for.

Khipu is a programming language and a laboratory. The language is typed, runs on every core, compiles to native code and WebAssembly, and is written in Rust. The laboratory is the [Experimental section of the roadmap](ROADMAP.md#experimental): ideas that should not work, each written down with the experiment that will prove it or kill it. Shaw wrote that all progress depends on the unreasonable man. Here we try to be unreasonable on purpose, and to measure it.

In daily use, Khipu keeps the feel of JavaScript and drops the parts people complain about: no silent type coercion, no truthiness, no `null` *and* `undefined`. Static types catch mistakes before anything runs, and lightweight tasks with channels give safe parallelism across every CPU core.

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

## Contracts

A function can say what it needs, what it promises, and show examples. Khipu checks `requires` on every call and `ensures` on every return, and `khipu test` runs every `example`:

```js
fn divide(a: number, b: number) -> number
    requires b != 0
    ensures result * b == a
    example divide(10, 2) == 5
{
    return a / b
}
```

A broken promise names the rule, the values and the line that broke it:

```
error: divide requires b != 0 (a = 3, b = 0)
 --> shop.kp:12
   |
12 | print(divide(total, count))
   |       ^^^^^^^^^^^^^^^^^^^^
```

Contracts make intent part of the code, so a person or an AI changing a function finds out at once when the change breaks what callers rely on.

## Standard library

Strings, lists and numbers have methods, and every one is type-checked, including the ones that take a function:

```js
let words = "  the quick brown fox  ".trim().split(" ")
let long = words.filter(fn(w: string) -> bool { return w.len() > 3 })
print(long.map(fn(w: string) -> string { return w.upper() }).join(", "))
print([5, 3, 9].sorted(), [1, 2, 3].sum(), (2).pow(10), Math.sqrt(16))
```

| On | Methods |
|---|---|
| strings | `len` `is_empty` `upper` `lower` `trim` `split` `lines` `chars` `contains` `starts_with` `ends_with` `replace` `index_of` `slice` `repeat` `to_number` |
| lists | `len` `is_empty` `push` `pop` `insert` `remove` `first` `last` `contains` `index_of` `slice` `concat` `reversed` `sorted` `sort_by` `join` `map` `filter` `reduce` `find` `any` `all` `count`, and `sum` `min` `max` on numbers |
| numbers | `sqrt` `floor` `ceil` `round` `abs` `pow` `min` `max` `clamp` `to_string` |
| `Math` | `pi` `e` `sqrt` `pow` `abs` `floor` `ceil` `round` `min` `max` `sin` `cos` `tan` `atan2` `log` `exp` `random` `random_int` |

Files and the terminal:

```js
File.write("notes.txt", "first line\n")
File.append("notes.txt", "second line\n")
for line in File.lines("notes.txt") {
    print(line.upper())
}
let name = input("Your name? ") ?? "stranger"
print("hello", name, "you passed", Process.args())
```

The web, with JSON built in:

```js
fn handle(request: Request) -> Response {
    if request.path == "/hello" {
        return Response.from_json({ message: "hello", path: request.path })
    }
    return Response.error(404, "nothing here")
}

Http.serve(8080, handle)
```

```js
let reply = Http.get("http://localhost:8080/hello")
print(reply.status, reply.json().message)
```

`Http` also has `post`, `post_json` and `request` for any method and headers, over `http://` or `https://`. `Http.serve` answers one request at a time on `localhost`; see [`examples/web`](examples/web). `Json.parse` and `Json.stringify` turn text into records, lists, numbers and back.

`File` also has `read`, `exists`, `list` and `delete`; `Process` has `args`, `env` and `exit`. A file that cannot be read stops the program with an error that says why, on the line that tried.

`range(n)` counts on demand instead of building a list, and a list of nothing but numbers or bools is stored compactly. Lookups that can miss give back `none` instead of a magic `-1`: `"abc".index_of("z")`, `[].first()` and `"x".to_number()` all return `none`, so the type checker makes you handle it with `??`. Methods that take a callback are written in Khipu itself, in [`src/prelude.kp`](src/prelude.kp).

## Tasks on every core

`spawn` starts a task. Tasks are cheap: fifty thousand can wait at once in about 100 MB. They run on every core and talk only through channels: whatever you `send` is copied, so two tasks never share a value and data races can't happen. A task that is waiting to start goes to whichever core is free, and a long-running task gives the others a turn, in loops and in deep recursion alike.

```js
let results = channel()

fn square(n: number) {
    send(results, n * n)
}

for n in range(1, 11) {
    spawn(square, n)
}

let total = 0
for _ in range(10) {
    total = total + receive(results)
}
print(total)
```

`select` waits on several channels and runs the arm for whichever gets a value first:

```js
let jobs = channel()
let quit = channel()

fn worker() {
    while true {
        select {
            job = receive(jobs) => print("working on", job)
            receive(quit) => return
        }
    }
}

spawn(worker)
send(jobs, "the report")
send(quit, true)
```

If every task ends up waiting on a channel that nothing will ever send to, Khipu stops with a deadlock error that points at the line, instead of hanging.

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
scripts/build-playground.sh   # optional: the web runtime that khipu web embeds
cargo install --path .
```

Run a file:

```bash
khipu examples/primes.kp
```

New to the language? [The Khipu Book](book/README.md) teaches it chapter by chapter, with examples that run.

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

## Packages

Packages come straight from git, with no central registry. A project is a folder with a `khipu.json`:

```bash
khipu init                                                  # creates khipu.json and main.kp
khipu add colors https://github.com/someone/colors.git#v1.0 # a tag, branch or commit after #
khipu add util path:../util                                 # or a folder next to yours
khipu run                                                   # runs the main file
```

```js
import colors from "colors"
print(colors.red("hello"))
```

An import without `.kp` is a package; with `.kp` it is a file next to yours. `khipu add` records the exact commit in `khipu.lock`, so `khipu install` gives everyone the same code; packages' own dependencies are installed too, but only from `https://`, `ssh://` or `git@` sources, so a package can never pull in a folder from your machine. Installing never runs a package's code, and package imports only look inside your own project.

## Roadmap

| Phase | Progress |
|---|---|
| 0. Foundations | `██████████` 12 / 12 |
| 1. Core language | `██████████` 10 / 10 |
| 2. Speed | `██████████` 7 / 7 |
| 3. Types | `██████████` 7 / 7 |
| 4. Benchmarks | `██████████` 8 / 8 |
| 5. Tooling | `██████████` 8 / 8 |
| 6. Concurrency | `██████████` 5 / 5 |
| 7. Ecosystem | `██████████` 6 / 6 |
| 8. Performance and final audit | `█████████░` 17 / 18 |
| 9. Documentation | `██░░░░░░░░` 1 / 5 |

**Next up:** Competitive with Node on the benchmark suite (as of 2026-11-26: faster on 5 of 17, within 1.6× on 2 more, 3–8× slower on records, structs, strings and closures; see AUDIT.md)

The full list is in [ROADMAP.md](ROADMAP.md). The progress bars update themselves on every commit.

## Performance

Sustained runs use inputs large enough that start-up is a small share of the time. Median of 5 runs on Apple M5; lower is better.

| Benchmark | Khipu | Python | Node | C | Rust | vs Python | vs Node |
|---|---|---|---|---|---|---|---|
| fib | 21 ms | 145 ms | 34 ms | 6.0 ms | 6.2 ms | **7.0× faster** | **1.6× faster** |
| fib_iterative | 123 ms | 425 ms | 42 ms | 9.7 ms | 9.8 ms | **3.5× faster** | 2.9× slower |
| loop | 24 ms | 2039 ms | 147 ms | 2.4 ms | 2.6 ms | **84.4× faster** | **6.1× faster** |
| nested_loops | 9.5 ms | 233 ms | 26 ms | 4.2 ms | 4.5 ms | **24.5× faster** | **2.8× faster** |
| sieve | 164 ms | 506 ms | 50 ms | 4.9 ms | 4.9 ms | **3.1× faster** | 3.2× slower |
| matrix | 110 ms | 248 ms | 28 ms | 3.0 ms | 4.5 ms | **2.3× faster** | 4.0× slower |
| mandelbrot | 6.3 ms | 223 ms | 24 ms | 3.7 ms | 3.9 ms | **35.5× faster** | **3.8× faster** |
| lists | 88 ms | 275 ms | 80 ms | 4.6 ms | 6.3 ms | **3.1× faster** | 1.1× slower |
| records | 160 ms | 319 ms | 25 ms | 2.3 ms | 2.5 ms | **2.0× faster** | 6.3× slower |
| strings | 200 ms | 270 ms | 39 ms | 82 ms | 105 ms | **1.4× faster** | 5.1× slower |
| calls | 54 ms | 187 ms | 34 ms | 2.3 ms | 2.2 ms | **3.5× faster** | 1.6× slower |
| closures | 100 ms | 224 ms | 28 ms | 4.7 ms | 4.1 ms | **2.2× faster** | 3.6× slower |
| locals | 11 ms | 262 ms | 62 ms | 2.4 ms | 2.4 ms | **24.7× faster** | **5.8× faster** |
| globals | 94 ms | 318 ms | 26 ms | 4.8 ms | 11 ms | **3.4× faster** | 3.5× slower |
| record_access | 209 ms | 801 ms | 26 ms | 4.9 ms | 6.4 ms | **3.8× faster** | 8.2× slower |
| struct_access | 220 ms | 746 ms | 27 ms | 4.9 ms | 6.3 ms | **3.4× faster** | 8.2× slower |
| parallel | 219 ms | 398 ms | 69 ms | 21 ms | 22 ms | **1.8× faster** | 3.2× slower |

Start-up, an empty program (median of 21 runs): Khipu 3.3 ms · Python 13 ms · Node 21 ms · C 2.1 ms · Rust 2.3 ms.

On sustained runs Khipu beats Python on 17 of 17 benchmarks and Node on 5 of 17. Cold-start results, spreads, memory use and the method are in [BENCHMARKS.md](BENCHMARKS.md).

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

## Native code

Functions that work only with numbers and bools are compiled to machine code while the program runs. A function qualifies when its body uses only number and bool values, arithmetic, comparisons, `&&` `||` `!`, `let`, assignment, `if`, `while`, `break`, `continue`, `return`, and calls to other functions that qualify, itself included. The compiler ([Cranelift](https://cranelift.dev)) runs the 64th time such a function is called, or on the first call if the function contains a loop, and from then on the call runs natively: `fib(32)` goes from 165 ms to 24 ms. The same goes for a `while` or `for ... in range(...)` loop whose variables hold numbers or bools, wherever it appears, even at the top level of a program: the loop is compiled when it is first reached, its variables go in, and their final values come back out. `loop` goes from 341 ms to 26 ms and `mandelbrot` from 78 ms to 7 ms. Nothing changes for the program: arguments or variables of another type fall back to the bytecode, too much recursion is still an error on the right line, and reassigning a global function throws the native code for it away. Spawned tasks keep running bytecode, so they keep taking turns.

## Web pages

`khipu web app.kp` turns a program into `app.html`: one file with the program and the Khipu runtime (compiled to WebAssembly) inside. It opens with a double-click, works offline, runs the program in the background so the page stays responsive, and shows output and errors just like the terminal. The program is type-checked first, so mistakes are caught before the page is written. The functions that qualify for native code (see above) are compiled straight to WebAssembly instructions when the page is built, so `fib(32)` takes 25 ms in the browser instead of over a second in the interpreter. Web pages cannot import other files or use files, HTTP or tasks yet.

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
| `src/task.rs` | Tasks, channels and `select`: a scheduler with one worker per core |
| `src/stdlib.rs`, `src/prelude.kp` | The standard library: fast built-ins in Rust, the rest in Khipu |
| `src/http.rs` | HTTP requests (with TLS), the small web server, and JSON |
| `src/package.rs` | The package manager: `khipu.json`, `khipu.lock` and git sources |
| `src/web.rs`, `build.rs` | `khipu web`: one self-contained HTML page per program |
| `src/vm.rs` | Runs the bytecode: one tight loop, calls without native recursion |
| `src/kernel.rs` | Finds functions that work only with numbers and bools, and lowers them to a small typed IR |
| `src/jit.rs`, `src/native.rs` | Compiles that IR to machine code with Cranelift, and switches hot functions over to it |
| `src/wasmgen.rs` | Compiles the same IR to WebAssembly instructions for `khipu web` pages |
| `src/value.rs` | Runtime values, operators and built-in functions; records are a shape plus values, and field reads cache the shape they saw last |
| `src/gc.rs` | The cycle collector: reference counting frees most values, this frees the cycles |
| `src/interpreter.rs` | Ties it together, plus the language test suite |
| `src/lib.rs` | The library the `khipu` command and the playground share |
| `src/wasm.rs` | Browser entry points: run, check and format |
| `src/main.rs` | The `khipu` command: run a file, `fmt`, `test`, `lsp`, or open the prompt |
| `benchmarks/` | The same programs in Khipu, Python, JavaScript, C and Rust |
| `book/` | The Khipu Book, a chapter-by-chapter tutorial whose examples `scripts/check-docs.py` runs |
| `examples/` | Example Khipu programs |
| `editors/vscode/` | VS Code extension |
| `editors/tree-sitter-khipu/` | Tree-sitter grammar and highlight queries |
| `playground/` | The browser playground |
| `scripts/` | Repository tooling: the roadmap sync, the benchmark runner, the doc checker |
| `AUDIT.md` | The Phase 8 audit: every roadmap item, the benchmark rerun and the independent review |

## Development

Checks run locally on every commit instead of in paid CI. Turn them on once after cloning:

```bash
git config core.hooksPath .githooks
```

Each commit then syncs the roadmap progress and runs `cargo fmt --check`, `cargo clippy`, `cargo test`, `khipu fmt --check` on every `.kp` file and `khipu test`. If any of them fail, the commit is stopped.

`cargo test` checks the WebAssembly backend by running the emitted module in `node`, when it is installed, and includes a short fuzz run: `tests/fuzz.rs` mutates the examples and benchmarks into thousands of broken programs and checks that lexing, parsing, formatting, checking and running them never crashes or hangs. For a longer run:

```bash
KHIPU_FUZZ=100000 cargo test --release --test fuzz -- --ignored
```

Every user-facing error message has a test that checks its wording and the line it points at. `unsafe` appears in three places, each with a comment saying why it holds: turning the pointer to a function the JIT just compiled into a callable with the signature we declared (`src/jit.rs`), calling it (`src/native.rs`), and, in the browser build only, calling the two functions the page provides (`src/native.rs`). The `#[unsafe(no_mangle)]` attributes in `src/wasm.rs` name the exports the browser calls. The JIT and WebAssembly tests compare every compiled function with the interpreter, and the playground build exercises the exports.
