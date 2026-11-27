# Contributing

Khipu is a laboratory as much as a language, so there are two kinds of contribution and both are welcome.

**Make the language better.** Bugs, error messages, standard library gaps, editor support, documentation. Open an issue first if the change is bigger than a fix, so nobody builds the same thing twice.

**Run an experiment.** The [Experimental section of the roadmap](ROADMAP.md#experimental) lists bets with the measurement that confirms or kills each one. Pick one, run its experiment, and report the numbers in an issue whether they are good or bad. A dead idea with a clear number is as valuable as a live one. To propose a new bet, write it the same way: the hypothesis, why Khipu is the place to try it, the experiment that decides it in days rather than months, the number that kills it, and the closest prior work you know of.

## Development

You need [Rust](https://rustup.rs). Build and run the tests:

```bash
cargo build --release
cargo test
```

The same checks run as a pre-commit hook locally and as GitHub Actions on every push and pull request. Turn the hook on once after cloning, so a commit fails before it reaches CI:

```bash
git config core.hooksPath .githooks
```

Each commit then syncs the roadmap progress and runs `cargo fmt --check`, `cargo clippy`, `cargo test`, `khipu fmt --check` on every `.kp` file and `khipu test`. If any of them fail, the commit is stopped.

`cargo test` checks the WebAssembly backend by running the emitted module in `node`, when it is installed, and includes a short fuzz run: `tests/fuzz.rs` mutates the examples and benchmarks into thousands of broken programs and checks that lexing, parsing, formatting, checking and running them never crashes or hangs. For a longer run:

```bash
KHIPU_FUZZ=100000 cargo test --release --test fuzz -- --ignored
```

Every user-facing error message has a test that checks its wording and the line it points at. `unsafe` appears in four places, each with a comment saying why it holds: turning the pointer to a function the JIT just compiled into a callable with the signature we declared (`src/jit.rs`), calling it (`src/native.rs`), in the browser build only, calling the two functions the page provides (`src/native.rs`), and the system calls of the HTTP server's event loop, `kqueue` or `epoll` and `poll` (`src/poll.rs`). The `#[unsafe(no_mangle)]` attributes in `src/wasm.rs` name the exports the browser calls. The JIT and WebAssembly tests compare every compiled function with the interpreter, and the playground build exercises the exports.

## Code

- The code explains itself. Comments are for `unsafe` blocks and for the rare place where the reason cannot be seen from the code.
- Add a test for every new user-facing error message, checking its wording and the line it points at.
- Every snippet in the README, `docs/` and `book/` is run by `scripts/check-docs.py`, so an example that stops working fails the commit. Write examples as complete programs.
- Benchmarks are rerun with `python3 scripts/bench.py`, which rewrites the tables in the README and BENCHMARKS.md. Do not edit those tables by hand.

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

## Pull requests

Small and focused beats large. Open an issue first for anything beyond a fix. The pull request template asks for a test, a check that the examples still run, and the experiment numbers when the change is an experimental item. Pull requests need CI green on Linux and macOS before they merge.

## Licence

By contributing you agree that your contribution is licensed under the [MIT Licence](LICENSE), like the rest of the project.
