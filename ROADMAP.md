# Khipu Roadmap

The smart adapt to the world. The fool tries to change it. This is the fool's notebook.

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
| 9. HTTP without limits | `████████░░` 20 / 24 |
| 10. Cleanup | `█░░░░░░░░░` 1 / 10 |
| 11. Documentation | `██░░░░░░░░` 1 / 5 |

**Next up:** Competitive with Node on the benchmark suite (as of 2026-11-27: faster on 7 of 18, within 1.2× on lists, 2–9× slower on records, strings, closures, globals and calls; see docs/audit.md)

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
- [x] docs/benchmarks.md: methodology, and every result, including where Khipu loses

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
- [ ] Competitive with Node on the benchmark suite (as of 2026-11-27: faster on 7 of 18, within 1.2× on lists, 2–9× slower on records, strings, closures, globals and calls; see docs/audit.md)
- [x] Start-up stays under 5 ms
- [x] Work stealing, so a busy core hands tasks to idle ones
- [x] Tasks also take turns inside deep recursion, not only in loops

Robustness
- [x] Fuzzing the lexer, parser, checker and VM finds no crash
- [x] Every error message tested, with line numbers
- [x] No `unsafe` code, or every use justified and tested

Final audit
- [x] Every roadmap item re-checked against the code and tests
- [x] Full benchmark rerun, with README and docs/benchmarks.md matching the new results
- [x] Every example and doc snippet runs as written
- [x] Independent review of the whole repo, with every finding fixed or written down

## Phase 9: HTTP without limits

Today `Http.serve` answers one request at a time on one thread. A backend should be bounded by the machine, not by the language.

Throughput
- [x] Every request is a task: accepted connections are handed to the scheduler and handled on every core, with the handler's globals copied once per worker, not per request
- [x] Keep-alive and pipelining, so a connection serves many requests without reconnecting
- [ ] Zero-copy request parsing: headers and body are slices of the read buffer until the program asks for a string
  (reassessed after `bytes`, still not reachable: `bytes` is an owned `Rc<Vec<u8>>` and `Value` is not `Send`, so the body could at best become one slice of a buffer moved to the worker, while method, path, query and every header stay `string` fields that are copied into `Rc<String>` when the `Request` is built; reaching the item needs lazy string values that point into a shared buffer, a change to the value model, not to the server)
- [x] Responses written in one syscall, with bodies that stream (`Response.stream`) when they do not fit in memory
- [x] Throughput benchmark with `wrk` against Node, Go and Rust on the same box, in docs/benchmarks.md, for a hello route, a JSON route and a route that reads a file

Robustness
- [x] Limits on by default: header size, body size, request count per connection, and a slow-client timeout, each changeable from the program
- [x] Backpressure: when every worker is busy, new connections wait in the kernel's queue instead of in memory, and the server never grows without bound
  (connections and sockets wait in the event loop; when every handler worker is busy and the queue of parsed requests, twice the workers, is full, the loop stops reading and accepting until one frees; past `max_connections` open connections, 10,000 by default, it stops accepting; a WebSocket whose handler has 64 messages waiting is not read until it catches up; in each case the wait is in the kernel's buffers)
- [x] A handler that throws answers 500 and the server keeps running; the error is printed with its line like any other
- [x] Graceful shutdown: in-flight requests finish, new ones are refused, `Process.exit` waits for both
- [x] HTTPS serving with a certificate and key from files, using the TLS the client already has
- [x] Fuzzed: malformed requests, truncated bodies, invalid UTF-8 in headers and a client that sends one byte a second never crash or hang the server
- [x] Non-blocking IO: connections and open sockets wait in one event loop (`kqueue` on macOS, `epoll` on Linux) and only a request that is being handled holds a worker, so ten thousand idle connections cost memory, not workers, and the starvation limit above goes away
- [x] A native kernel that fails after writing to a list raises the error from the recorded site instead of rerunning the function in the interpreter, so a handler that survives the error never sees the write applied twice

Interface
- [x] `Request` gives method, path, query parameters, headers and body; `Response` sets status, headers and body, with `from_json`, `text`, `html` and `file` helpers
- [x] Routing in the standard library: match on method and path with parameters, written in Khipu in `src/prelude.kp`
- [x] WebSockets: upgrade a request, then `send` and `receive` on it like a channel
- [x] A `bytes` value: binary request bodies, `Response.file` for non-text files, and binary WebSocket frames in both directions, without going through a string
- [x] WebSocket subprotocols and compression: `Sec-WebSocket-Protocol` negotiated from a list the program gives, and `permessage-deflate` when the client offers it
- [x] A WebSocket client: `Http.websocket(url)` returns a socket that works with `send` and `receive` like the server side, over TLS when the URL is `wss`
- [x] `close(socket)` closes a WebSocket with a code and reason from the program, and `select` can wait on sockets alongside channels
- [ ] A 24-hour soak test under load with memory flat at the end, written down in docs/audit.md
- [ ] The WebSocket client at parity with the server: pings and a timeout on a silent server so a socket to a dead peer does not wait forever, `permessage-deflate` when the server accepts it, and the protocol the server chose readable from the program
- [ ] `bytes` as a full value: `data[i]` for one byte, `bytes([..])` from a list of numbers, `File.read_bytes` and `File.write_bytes`, and binary bodies from `Http.get` and `Http.request`
- [x] `close(channel)` on a plain channel: `receive` drains what is queued and then gives `none`, `select` sees it like a closed socket, and `send` after `close` is an error with its line

## Phase 10: Cleanup

Before the documentation is written, the code it describes is made small. Every item here removes something or merges two things into one; none adds a feature.

- [x] One home per kind of file: experiments under `experiments/`, the audit and the benchmark report under `docs/`, the community files under `.github/`, and nothing at the root but the README, the roadmap, the contributing guide, the licence and the Cargo files
- [ ] Dead code: every function, variant and `pub` item in `src/` has a caller or a test, and the rest is gone
- [ ] One of each: duplicated helpers merged so each job has one function (encoding in `src/web.rs` and `src/http.rs`, number formatting, error construction, request parsing)
- [ ] Files by concern: `src/interpreter.rs` and `src/http.rs` split so a file is one thing, with tests beside the code they test
- [ ] Dependencies: every crate in `Cargo.toml` justified in a line in CONTRIBUTING.md, `cargo audit` clean, and nothing kept for one call that a few lines could do
- [ ] Error messages in one voice and shape across the parser, checker, VM and HTTP, each one tested
- [ ] Every `unsafe` block in `src/jit.rs`, `src/native.rs` and `src/wasm.rs` states the invariant it relies on, and the ones that can go are gone
- [ ] Clippy at `pedantic` reviewed: fixed, or allowed in one place with the reason next to it
- [ ] `src/prelude.kp` without dead or duplicated functions, every function with an `example`
- [ ] The pre-commit hook and the test suite timed and written down in CONTRIBUTING.md, with the slow tests named

---

## Phase 11: Documentation

The last regular phase, written once the language has settled and the code is clean.

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
- [ ] Adaptive thresholds instead of magic constants: the 64 calls before a function compiles, the size of a task chunk and when an idle worker is woken are today fixed numbers chosen by hand. Replace them with a feedback controller that measures energy × delay (joules times seconds, from `powermetrics` on macOS and RAPL on Linux) and moves each knob toward the minimum while the program runs. Experiment: sweep each constant by hand across `parallel`, `sieve` and the HTTP examples and record energy × delay; if the best value differs by more than 2× between workloads, a controller has something to find, and the thesis needs it to beat every fixed value on the mixed run. Known failures: a controller that oscillates, one that tunes itself to the benchmark and not the user's program, and measurement noise bigger than the gains

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

### Experimental phase 5: New era

Ideas with no shipped or published equivalent that we know of. The closest prior work is named so the claim can be checked, and each would be new if it works at all.

- [ ] The program is what it did: compile the grammar of the execution trace. Run the program once in the interpreter, record the dynamic instruction stream, and compress it with a grammar inference algorithm (Sequitur, Re-Pair). Every repeated structure becomes a grammar rule, whether it is a loop, a function, or a sequence that crosses three function boundaries and no static compiler would ever see as one unit. Emit native code per rule, with a guard where the recorded trace diverged. The compiled program's shape comes from what the program actually did, not from how it was written. Closest prior work: Larus's whole-program paths (1999) compressed traces with Sequitur for profiling only; trace JITs compile one loop at a time. Experiment: record the trace of the five numeric benchmarks, run Sequitur, and count how much of the trace the top twenty rules cover; over 90% means the grammar is a compilable program. Known failures: traces are gigabytes for a second of execution, a different input gives a different grammar, and a rule that only repeats because of the input's accident guards and bails everywhere
- [ ] Entropy-budgeted compilation: measure the information content of every site. Observe each call site, field access, loop and parameter while the tests run and compute the Shannon entropy of what passed through it, in bits. Zero bits is a constant and gets folded, a struct field with zero bits is removed from the struct, one bit gets two specialized versions, three bits get eight, and above a budget the site stays generic. Specialization is spent exactly where information is scarce, which is where it pays, and the same number decides memory layout and code layout. Closest prior work: inline caches with a fixed megamorphic threshold, constant propagation; nobody uses entropy as the budget or applies it to layout. Experiment: instrument the VM to record a histogram per site over the examples' tests and print the entropy table; the thesis needs most sites under two bits and the hot ones under one. Known failures: tests with low entropy and production with high, so every specialization misses; entropy estimated from few samples is wrong in the direction that specializes too much
- [ ] Complexity is a contract: `costs n * log(n)` next to `requires` and `ensures`. The function declares its complexity class in terms of its arguments; `khipu test` runs the `example` blocks at several sizes, fits the measured times, and fails the test when the fit is a worse class than the contract, so an accidental quadratic is a failing test, not a production incident. The compiler uses the declared class too: a function that costs `1` can be called inside a loop kernel, one that costs `n` cannot be hoisted. Closest prior work: Goldsmith's empirical computational complexity (2007) as a standalone profiler, resource-aware ML as a static analysis; no language has it as a contract the test runner checks. Experiment: add the fit to `khipu test` for the standard library's sort, map and filter and see whether it distinguishes `n`, `n * log(n)` and `n * n` reliably on a laptop with other programs running; the thesis needs under 1% false failures across 100 runs. Known failures: timing noise at small sizes, cache effects that look like a class change, constants that hide a class for every size the tests can afford
- [ ] Laws the compiler finds for you: for every function, test candidate laws over random inputs, commutativity `f(a, b) == f(b, a)`, idempotence `f(f(x)) == f(x)`, involution `f(f(x)) == x`, linearity `f(k * x) == k * f(x)`, order independence `f(reverse(xs)) == f(xs)`, and propose the ones that hold as `ensures` contracts the fuzzer then guards, so an edit that breaks a law fails a test. Closest prior work: QuickSpec discovers equational laws for Haskell functions; no language turns the laws into contracts in the source. One hard line: a law found by testing is a conjecture, safe as a contract or a test, never on its own a licence to optimize; a fold is parallelized on a discovered associativity only when the programmer confirms it or the checker proves it, because one counterexample at the millionth input is a silently wrong program. Experiment: run the discovery over `src/prelude.kp` and `examples/`, count the real laws it finds and the false ones, then break one law per standard-library function on purpose and count how many the generated contracts catch. Known failures: laws that hold for every random input and fail on the structured one, numeric laws broken by floating-point rounding, and a flood of trivial laws nobody wants in their source
