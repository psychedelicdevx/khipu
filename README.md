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

Khipu is a programming language that keeps the feel of JavaScript and drops the parts people complain about: no silent type coercion, no truthiness, no `null` *and* `undefined`. It has static types that catch mistakes before anything runs, and its signature feature: safe parallelism across every CPU core with lightweight tasks and channels.

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

Lookups that can miss give back `none` instead of a magic `-1`: `"abc".index_of("z")`, `[].first()` and `"x".to_number()` all return `none`, so the type checker makes you handle it with `??`. Methods that take a callback are written in Khipu itself, in [`src/prelude.kp`](src/prelude.kp).

## Tasks on every core

`spawn` starts a task. Tasks are cheap: fifty thousand can wait at once in about 100 MB. They run on every core and talk only through channels: whatever you `send` is copied, so two tasks never share a value and data races can't happen.

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
select {
    job = receive(jobs) => handle(job)
    receive(quit) => return
}
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

An import without `.kp` is a package; with `.kp` it is a file next to yours. `khipu add` records the exact commit in `khipu.lock`, so `khipu install` gives everyone the same code; packages' own dependencies are installed too, but only from `https://`, `ssh://` or `git@` sources, so a package can never pull in a folder from your machine. Installing never runs a package's code, and imports only look inside your own project.

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
| 8. Performance and final audit | `░░░░░░░░░░` 0 / 18 |
| 9. Documentation | `░░░░░░░░░░` 0 / 5 |

**Next up:** Lazy `range()` that never builds a list

The full list is in [ROADMAP.md](ROADMAP.md). The progress bars update themselves on every commit.

## Performance

Sustained runs use inputs large enough that start-up is a small share of the time. Median of 5 runs on Apple M5; lower is better.

| Benchmark | Khipu | Python | Node | C | Rust | vs Python | vs Node |
|---|---|---|---|---|---|---|---|
| fib | 161 ms | 149 ms | 35 ms | 6.2 ms | 6.2 ms | 1.1× slower | 4.6× slower |
| fib_iterative | 613 ms | 431 ms | 44 ms | 9.8 ms | 10 ms | 1.4× slower | 13.9× slower |
| loop | 356 ms | 2206 ms | 148 ms | 2.5 ms | 2.7 ms | **6.2× faster** | 2.4× slower |
| nested_loops | 171 ms | 227 ms | 28 ms | 4.2 ms | 4.3 ms | **1.3× faster** | 6.1× slower |
| sieve | 334 ms | 549 ms | 51 ms | 5.0 ms | 5.3 ms | **1.6× faster** | 6.5× slower |
| matrix | 136 ms | 259 ms | 28 ms | 3.3 ms | 4.8 ms | **1.9× faster** | 4.8× slower |
| mandelbrot | 79 ms | 234 ms | 27 ms | 4.4 ms | 4.3 ms | **2.9× faster** | 2.9× slower |
| lists | 111 ms | 283 ms | 77 ms | 5.2 ms | 6.0 ms | **2.5× faster** | 1.4× slower |
| records | 215 ms | 340 ms | 27 ms | 2.4 ms | 2.9 ms | **1.6× faster** | 8.0× slower |
| strings | 231 ms | 298 ms | 44 ms | 90 ms | 110 ms | **1.3× faster** | 5.3× slower |
| calls | 71 ms | 198 ms | 35 ms | 2.6 ms | 2.5 ms | **2.8× faster** | 2.0× slower |
| closures | 130 ms | 237 ms | 29 ms | 5.6 ms | 4.2 ms | **1.8× faster** | 4.5× slower |
| locals | 147 ms | 279 ms | 65 ms | 2.7 ms | 2.7 ms | **1.9× faster** | 2.3× slower |
| globals | 94 ms | 332 ms | 27 ms | 5.0 ms | 11 ms | **3.5× faster** | 3.5× slower |
| record_access | 275 ms | 844 ms | 27 ms | 5.0 ms | 6.4 ms | **3.1× faster** | 10.3× slower |
| struct_access | 275 ms | 739 ms | 27 ms | 5.3 ms | 6.5 ms | **2.7× faster** | 10.2× slower |
| parallel | 264 ms | 421 ms | 72 ms | 21 ms | 22 ms | **1.6× faster** | 3.7× slower |

Start-up, an empty program (median of 21 runs): Khipu 2.6 ms · Python 13 ms · Node 23 ms · C 2.3 ms · Rust 2.5 ms.

On sustained runs Khipu beats Python on 15 of 17 benchmarks and Node on 0 of 17. Cold-start results, spreads, memory use and the method are in [BENCHMARKS.md](BENCHMARKS.md).

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

## Web pages

`khipu web app.kp` turns a program into `app.html`: one file with the program and the Khipu runtime (compiled to WebAssembly) inside. It opens with a double-click, works offline, runs the program in the background so the page stays responsive, and shows output and errors just like the terminal. The program is type-checked first, so mistakes are caught before the page is written. Web pages cannot import other files or use files, HTTP or tasks yet.

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
