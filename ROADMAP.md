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
| 6. Concurrency | `██████████` 5 / 5 |
| 7. Ecosystem | `██████████` 6 / 6 |
| 8. Performance and final audit | `█████████░` 17 / 18 |
| 9. Documentation | `██░░░░░░░░` 1 / 5 |

**Next up:** Competitive with Node on the benchmark suite (as of 2026-11-26: faster on 5 of 17, within 1.6× on 2 more, 3–8× slower on records, structs, strings and closures; see AUDIT.md)

The bets that may never land are in the [Experimental](#experimental) section at the end. They are not counted above.

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

## Phase 6: Concurrency (Khipu's signature) ✅

- [x] `spawn` lightweight tasks
- [x] Channels: `send` and `receive`
- [x] Tasks running on all CPU cores
- [x] `select` across several channels
- [x] No shared mutable state, so no data races

## Phase 7: Ecosystem ✅

- [x] Standard library: math, strings, lists
- [x] Files and the terminal
- [x] HTTP: fetch and a small server
- [x] Contracts: `requires`, `ensures`, `example` (LLM-proof code)
- [x] Package manager
- [x] Compile Khipu to WASM for the web

## Phase 8: Performance and final audit

Everything the benchmarks found, fixed and proven, then a full independent review of the project.

Memory
- [x] Lazy `range()` that never builds a list
- [x] Garbage collector that frees cycles
- [x] Peak memory below Python and Node on every benchmark

Speed
- [x] Faster function calls, so `fib` and `fib_iterative` beat Python
- [x] Beat Python on every sustained benchmark
- [x] Native compilation (Cranelift)
- [x] Compile Khipu straight to WebAssembly instructions, not only bundle the interpreter
- [ ] Competitive with Node on the benchmark suite (as of 2026-11-26: faster on 5 of 17, within 1.6× on 2 more, 3–8× slower on records, structs, strings and closures; see AUDIT.md)
- [x] Start-up stays under 5 ms
- [x] Work stealing, so a busy core hands tasks to idle ones
- [x] Tasks also take turns inside deep recursion, not only in loops

Robustness
- [x] Fuzzing the lexer, parser, checker and VM finds no crash
- [x] Every error message tested, with line numbers
- [x] No `unsafe` code, or every use justified and tested

Final audit
- [x] Every roadmap item re-checked against the code and tests
- [x] Full benchmark rerun, with README and BENCHMARKS.md matching the new results
- [x] Every example and doc snippet runs as written
- [x] Independent review of the whole repo, with every finding fixed or written down

## Phase 9: Documentation

The last regular phase, written once the language has settled.

- [x] The Khipu Book: teaches the language
- [ ] Khipu Language Reference: exactly how everything behaves
- [ ] Standard Library Reference: every API, with examples
- [ ] Khipu Internals: the Rust implementation, compiler, VM, garbage collector and scheduler
- [ ] Documentation site that hosts all four

---

## Experimental

Always the last section. Nothing here is promised. Each phase builds on the one before it, and each item is a bet with the measurement that confirms or kills it. The hypothesis is written first; it becomes a thesis only once the experiment says so.

### Experimental phase 1: Near C on typed code

The stable path. Khipu already checks types; the bet is that the interpreter should rarely run.

- [ ] Whole-program typed AOT: extend the kernel IR from numbers and bools to records, structs, lists, strings and closures, so every function whose types the checker proves compiles to native code, with bytecode only for what stays dynamic
- [ ] Flat typed structs: a struct instance is plain memory with field offsets known at compile time, one load per field, no shape check; lists of structs can be laid out column-wise
- [ ] 8-byte values: NaN-boxing or a tagged word instead of the 16-byte `Rc`/`RefCell` enum, with `UnsafeCell` where the compiler proves there is no aliasing
- [ ] Ownership instead of refcounting: escape analysis in the kernel lowering puts values that never leave a function on the stack or a per-call arena; only escaping values pay for `Rc` and the cycle collector
- [ ] LLVM backend for `khipu build --release`: inlining, loop-invariant motion and SIMD that Cranelift does not do; Cranelift stays for the JIT and the web
- [ ] Interpreter polish last: register VM, unchecked indexing, direct tag dispatch (capped at about 1.5× and only worth it once the items above make the interpreter the cold path)

### Experimental phase 2: Hypotheses

Could work, could fail. Each starts with a two-to-three-day experiment; the thesis is only claimed if the numbers hold.

- [ ] Memory is a compile-time problem: infer regions (frame, loop, task) for every allocation and free them with a pointer reset, so the hot path has no refcounts and no collector; escaping values are promoted at the `pack` boundary. Experiment: count `Rc` traffic and allocations per benchmark tagged by whether the value escaped its frame; the thesis stands if over 80% never escape. Known failure: a long loop allocating into a region that frees only at exit
- [ ] The test suite is the warm-up: record every observed type, shape, arity and loop kind while `khipu test` runs, then compile the whole program ahead of time specialized to that profile, with guards and bytecode fallback, so there is no JIT and no warm-up. Experiment: dump the profile from the examples' tests, run the examples, count the sites that see a type the profile never saw; under 5% misses is a thesis. Known failure: tests that exercise other types than production, so every guard misses
- [ ] The loop decides the layout: when the kernel IR sees a loop touching two fields of a struct list with no cross-iteration dependency, choose struct-of-arrays for that list and split the iterations across workers, without the programmer writing SIMD or tasks. Experiment: hand-convert `record_access` and `struct_access` to struct-of-arrays and a four-worker split in Khipu itself; under 3× means the analysis is not worth building. Known failures: lists that escape to row-wise code and transpose on every crossing, short loops that lose to commit cost, print order

### Experimental phase 3: Only a mad engineer would try

Each is less insane in Khipu than anywhere else, and each has been tried in research and never shipped.

- [ ] The compiler that searches for its own output: a stochastic superoptimizer (STOKE-style) as the backend for hot kernel blocks, verified against the interpreter on thousands of inputs and proven with an SMT solver, with results cached by block hash so the compiler gets faster the longer you own it. Experiment: random search of 10 million candidates per block over the five numeric benchmark kernels; a thesis only if any block beats the Cranelift output. Known failures: floating-point edge cases that agree on a million inputs and disagree on one, compile times in minutes before the cache warms
- [ ] The program runs before it is written: execute everything up to the first I/O call at compile time and ship only the residue, every constant folded and every struct already built; then the Futamura projections, a Khipu interpreter written in Khipu fed to the same evaluator yields a compiler for free. Experiment: stop the interpreter at the first `Http`, `input` or file call, snapshot the globals, emit a program that starts from the snapshot, and measure how much of the HTTP server examples' start-up collapses; stage two is worth a month only if start-up drops to microseconds and the residue is smaller than the source. Known failures: deciding what to unfold is the whole field of partial evaluation; input-dependent loop bounds
- [ ] Speculative parallelism: run the iterations of a plain `for` loop on every core with private copies via `pack`, commit in order, roll back on conflict, with `print` buffered so output stays in iteration order. Experiment: write the records and struct benchmark loops as chunked tasks by hand to measure the ceiling with zero speculation cost; under 2× on eight cores kills it, 5× or more earns a conflict detector for list indices only. Known failures: loops that append to one list conflict on every iteration, memory doubles while copies are alive, and a missed aliasing path is a silent wrong answer, the worst failure a language can have

### Experimental phase 4: Insanity

Never tried, because the failure margin is far higher than anything above. Each one sounds insane, is insane, and would work insane.

- [ ] The GPU is the scheduler: run the bytecode VM itself on the GPU, one GPU thread per Khipu task, with the `pack` graph as the flat memory a GPU wants and channels as atomic queues in device memory. Ten thousand `spawn`s become ten thousand hardware threads instead of eight workers taking turns. Why Khipu: tasks share no mutable state, so the only thing the GPU memory model cannot do is the one thing the language already forbids. Experiment: a cut-down VM in WGSL that runs only numbers, bools, lists and channels; `spawn` 100 000 tasks that each do 1 000 iterations and compare against the thread pool. Under 10× kills it, since the transfer and divergence costs only grow from there. Known failures: divergent control flow serializes a warp to one task at a time, double precision runs at a fraction of single, strings and records have no home on the device, and any host call from a task stalls everything
- [ ] Start every core in the future: a sequential loop runs on all cores at once, each core starting at an iteration the runtime has not reached yet, with its starting variables predicted from the first iterations (counters are linear, sums are linear, most induction variables are a line). When the real execution arrives at that iteration, the prediction is checked; if it was right the whole chunk is already done, if it was wrong the chunk is thrown away. Why Khipu: the kernel IR already knows every variable a loop reads and writes, so the predictor knows exactly which values it must guess and the check is a comparison of a few f64s. Experiment: log the loop variables at every iteration of the benchmark loops and measure, offline, how many iterations ahead a linear predictor is still right; `loop` and `sieve` should be predictable forever, `mandelbrot` for zero iterations. A thesis only if over half of the loop time in the suite sits in predictable loops. Known failures: anything that accumulates floating-point error breaks the equality check, a loop that writes to a list is predictable only if the writes are too, and the wasted energy of wrong guesses on a laptop
- [ ] Data lives inside the instructions: globals, struct field offsets and constants are not loaded from memory but baked into the machine code as immediates, and assigning to a global patches every instruction that used it, in place, while the program runs. A read of a global is zero loads; the price moves to writes, which become code patches with an instruction-cache flush. Why Khipu: globals are already tracked by a generation counter that invalidates native code on assignment, so the list of code sites that depend on each global exists, and the language has no pointers to globals that could bypass the patch. Experiment: patch only numeric globals in the Cranelift output and run `globals` and `record_access`; the thesis needs a read-heavy win over 2× with the write-heavy case no more than 20% slower. Known failures: W^X memory policies on macOS and most Linux hardening make writable code a fight with the operating system, a write-heavy loop flushes the instruction cache on every assignment and runs 100× slower, patching while another thread executes the same instruction is undefined in every architecture manual, and a patch that lands one byte off is a crash with no useful message
