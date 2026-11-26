# Khipu Roadmap

JavaScript, minus the bad parts, plus real types and real multi-core.

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

---

## Phase 0: Foundations ✅

- [x] Lexer: numbers and math operators
- [x] Parser with correct precedence (`2 + 3 * 4` is 14)
- [x] Calculator REPL
- [x] Variables (`let`) and `print`
- [x] Running `.kp` files (`khipu main.kp`)
- [x] `//` comments
- [x] Errors with line numbers
- [x] Booleans, comparisons, `&&` `||` `!`, `%`
- [x] `if` / `else if` / `else`
- [x] `while` loops and reassignment
- [x] Block scope (`let` inside `{ }` stays inside)
- [x] No truthiness, no coercion (`if 1` and `1 + true` are errors)

## Phase 1: Core language ✅

- [x] Strings, joining with `+`, escapes like `\n`
- [x] String interpolation: `"total: ${price + tax}"`
- [x] Functions: `fn`, parameters, `return`
- [x] Recursion and closures
- [x] `print` becomes a real built-in function
- [x] Lists: `[1, 2, 3]`, indexing, `len`
- [x] Records (objects): `{ name: "Ada", age: 36 }`
- [x] `for item in list` loops, `break` and `continue`
- [x] `none` instead of `null` and `undefined`
- [x] Modules: `import` other `.kp` files

## Phase 2: Speed ✅

- [x] Benchmark suite, with timings shown in the README
- [x] Resolve variables to slots before running
- [x] Skip scopes for blocks that declare nothing
- [x] Strings that are cheap to share
- [x] Bytecode compiler
- [x] Virtual machine that runs the bytecode
- [x] Faster than Python on every benchmark

## Phase 3: Types (the C# feel) ✅

- [x] Type annotations: `let age: number = 36`
- [x] A type checker that runs before the program does
- [x] Type inference for `let`
- [x] Typed function signatures
- [x] Structs / classes with methods
- [x] Enums with values, and `match`
- [x] Generics: `List<T>`

## Phase 4: Benchmarks ✅

- [x] Median of 20+ runs with spread, instead of best of 5
- [x] Separate cold-start and sustained (long-running) results
- [x] Peak memory for every benchmark
- [x] CPU set: iterative Fibonacci, nested loops, prime sieve, matrix multiply, Mandelbrot
- [x] Microbenchmarks: calls, local and global variables, record and struct access
- [x] C and Rust reference versions of every benchmark
- [x] Interleaved runs with raw results saved in `benchmarks/results`
- [x] BENCHMARKS.md: methodology, and every result, including where Khipu loses

## Phase 5: Tooling ✅

- [x] VS Code syntax highlighting
- [x] `.kp` file icon (KP, Go style)
- [x] Error messages that show the line and point at the problem
- [x] `khipu fmt`: one canonical formatting
- [x] `khipu test`: built-in test blocks
- [x] Language server: errors as you type, autocomplete, hover
- [x] Web playground (Khipu running in the browser via WASM)
- [x] Tree-sitter grammar (Zed, Neovim, GitHub)

## Phase 6: Concurrency (Khipu's signature)

- [ ] `spawn` lightweight tasks
- [ ] Channels: `send` and `receive`
- [ ] Tasks running on all CPU cores
- [ ] `select` across several channels
- [ ] No shared mutable state, so no data races

## Phase 7: Ecosystem

- [ ] Standard library: math, strings, lists
- [ ] Files and the terminal
- [ ] HTTP: fetch and a small server
- [ ] Contracts: `requires`, `ensures`, `example` (LLM-proof code)
- [ ] Package manager
- [ ] Compile Khipu to WASM for the web

## Phase 8: Performance and final audit

Everything the benchmarks found, fixed and proven, then a full independent review of the project.

Memory
- [ ] Lazy `range()` that never builds a list
- [ ] Garbage collector that frees cycles
- [ ] Peak memory below Python and Node on every benchmark

Speed
- [ ] Faster function calls, so `fib` and `fib_iterative` beat Python
- [ ] Beat Python on every sustained benchmark
- [ ] Native compilation (Cranelift)
- [ ] Competitive with Node on the benchmark suite
- [ ] Start-up stays under 5 ms

Robustness
- [ ] Fuzzing the lexer, parser, checker and VM finds no crash
- [ ] Every error message tested, with line numbers
- [ ] No `unsafe` code, or every use justified and tested

Final audit
- [ ] Every roadmap item re-checked against the code and tests
- [ ] Full benchmark rerun, with README and BENCHMARKS.md matching the new results
- [ ] Every example and doc snippet runs as written
- [ ] Independent review of the whole repo, with every finding fixed or written down

## Phase 9: Documentation

The last phase, written once the language has settled.

- [ ] The Khipu Book: teaches the language
- [ ] Khipu Language Reference: exactly how everything behaves
- [ ] Standard Library Reference: every API, with examples
- [ ] Khipu Internals: the Rust implementation, compiler, VM, garbage collector and scheduler
- [ ] Documentation site that hosts all four
