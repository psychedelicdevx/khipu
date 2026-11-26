# Khipu Roadmap

JavaScript, minus the bad parts, plus real types and real multi-core.

| Phase | Progress |
|---|---|
| 0. Foundations | `██████████` 12 / 12 |
| 1. Core language | `██████████` 10 / 10 |
| 2. Speed | `░░░░░░░░░░` 0 / 7 |
| 3. Types | `░░░░░░░░░░` 0 / 7 |
| 4. Tooling | `██░░░░░░░░` 2 / 9 |
| 5. Concurrency | `░░░░░░░░░░` 0 / 5 |
| 6. Ecosystem | `░░░░░░░░░░` 0 / 6 |
| 7. Native speed | `░░░░░░░░░░` 0 / 3 |

**Next up:** Benchmark suite, with timings shown in the README

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

## Phase 2: Speed

- [ ] Benchmark suite, with timings shown in the README
- [ ] Resolve variables to slots before running
- [ ] Skip scopes for blocks that declare nothing
- [ ] Strings that are cheap to share
- [ ] Bytecode compiler
- [ ] Virtual machine that runs the bytecode
- [ ] Faster than Python on every benchmark

## Phase 3: Types (the C# feel)

- [ ] Type annotations: `let age: number = 36`
- [ ] A type checker that runs before the program does
- [ ] Type inference for `let`
- [ ] Typed function signatures
- [ ] Structs / classes with methods
- [ ] Enums with values, and `match`
- [ ] Generics: `List<T>`

## Phase 4: Tooling

- [x] VS Code syntax highlighting
- [x] `.kp` file icon (KP, Go style)
- [ ] Error messages that show the line and point at the problem
- [ ] `khipu fmt`: one canonical formatting
- [ ] `khipu test`: built-in test blocks
- [ ] Language server: errors as you type, autocomplete, hover
- [ ] Web playground (Khipu running in the browser via WASM)
- [ ] Tree-sitter grammar (Zed, Neovim, GitHub)
- [ ] Documentation site

## Phase 5: Concurrency (Khipu's signature)

- [ ] `spawn` lightweight tasks
- [ ] Channels: `send` and `receive`
- [ ] Tasks running on all CPU cores
- [ ] `select` across several channels
- [ ] No shared mutable state, so no data races

## Phase 6: Ecosystem

- [ ] Standard library: math, strings, lists
- [ ] Files and the terminal
- [ ] HTTP: fetch and a small server
- [ ] Contracts: `requires`, `ensures`, `example` (LLM-proof code)
- [ ] Package manager
- [ ] Compile Khipu to WASM for the web

## Phase 7: Native speed

- [ ] Garbage collector
- [ ] Native compilation (Cranelift)
- [ ] Competitive with Node on the benchmark suite
