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
| 1. Core language | `██░░░░░░░░` 2 / 10 |
| 2. Types | `░░░░░░░░░░` 0 / 7 |
| 3. Tooling | `██░░░░░░░░` 2 / 9 |
| 4. Speed | `░░░░░░░░░░` 0 / 5 |
| 5. Concurrency | `░░░░░░░░░░` 0 / 5 |
| 6. Ecosystem | `░░░░░░░░░░` 0 / 6 |

**Next up:** Functions: `fn`, parameters, `return`

The full list is in [ROADMAP.md](ROADMAP.md). The progress bars update themselves on every commit.

## Editor support

`editors/vscode` is a VS Code extension with syntax highlighting, the `.kp` file icon, comment toggling and bracket handling. If you use the Symbols icon theme, run `editors/vscode/patch-symbols.sh` to give `.kp` files the KP icon there too.

## Project layout

| Path | What's there |
|---|---|
| `src/lexer.rs` | Turns source text into tokens |
| `src/parser.rs` | Turns tokens into a syntax tree |
| `src/interpreter.rs` | Runs the tree: values, scopes, errors |
| `src/main.rs` | The `khipu` command: run a file or open the prompt |
| `examples/` | Example Khipu programs |
| `editors/vscode/` | VS Code extension |
| `scripts/` | Repository tooling, like the roadmap sync |

## Development

Checks run locally on every commit instead of in paid CI. Turn them on once after cloning:

```bash
git config core.hooksPath .githooks
```

Each commit then syncs the roadmap progress and runs `cargo fmt --check`, `cargo clippy` and `cargo test`. If any of them fail, the commit is stopped.
