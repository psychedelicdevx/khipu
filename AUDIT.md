# Phase 8 audit

The last step of Phase 8: every roadmap item checked against the code and the tests that cover it, the benchmark results after the performance work, and the findings of an independent review of the repository. Dated 2026-11-26, commit at the head of `main` when this file was last touched.

## How this was checked

- `cargo test` runs 111 unit tests, the fuzz smoke run (1 500 mutated programs) and the WebAssembly check (the emitted module runs in `node` and is compared with the interpreter). A 60 000-program fuzz run was done once for the record.
- `scripts/check-docs.py` runs every Khipu snippet in `README.md` and every program in `examples/` as written. The pre-commit hook runs it on every commit.
- `python3 scripts/bench.py` reran the whole suite; the tables in `README.md` and `BENCHMARKS.md` are its output.
- The review in the last section was done by a separate agent with no part in writing the code, reading the repository cold.

## Phase 0: Foundations

| Item | Where | Checked by |
|---|---|---|
| Lexer: numbers and math operators | `src/lexer.rs` | `lexer::tests::lexes_simple_math` |
| Parser with correct precedence | `src/parser.rs` | `interpreter::tests::respects_precedence_and_parentheses` |
| Calculator REPL | `src/main.rs` `repl` | `the_prompt_remembers_types_and_recovers_from_mistakes` (the same `Interpreter::run` the prompt uses) |
| Variables and `print` | `src/compiler.rs`, `src/value.rs` `print` | `runs_a_multi_line_program`, `check-docs.py` |
| Running `.kp` files | `src/main.rs` `run_file` | every example in `check-docs.py` |
| `//` comments | `src/lexer.rs` | `lexer::tests::skips_comments_and_counts_lines` |
| Errors with line numbers | `src/error.rs` | `error::tests`, `reports_every_error_with_its_line` |
| Booleans, comparisons, `&&` `\|\|` `!`, `%` | `src/value.rs` `binary` | `compares_and_combines` |
| `if` / `else if` / `else` | `src/compiler.rs` | `picks_the_right_branch` |
| `while` loops and reassignment | `src/compiler.rs` | `loops_with_while` |
| Block scope | `src/compiler.rs` `begin_scope` | `keeps_block_variables_inside_the_block` |
| No truthiness, no coercion | `src/checker.rs`, `src/vm.rs` `CheckBool` | `has_no_truthiness_or_coercion` |

## Phase 1: Core language

| Item | Where | Checked by |
|---|---|---|
| Strings, `+`, escapes | `src/lexer.rs`, `src/value.rs` | `joins_and_compares_strings`, `lexer::tests::lexes_strings_with_escapes` |
| String interpolation | `src/lexer.rs` templates, `Code::Concat`, `Code::Stringify` | `interpolates_expressions_into_strings`, `lists_of_one_kind_can_still_take_anything` |
| Functions | `src/compiler.rs` `function` | `calls_functions_and_returns_values` |
| Recursion and closures | `src/vm.rs` frames, cells | `supports_recursion`, `closures_remember_their_variables` |
| `print` as a built-in | `src/value.rs` `NATIVES` | `functions_without_return_give_none` |
| Lists | `src/value.rs` `List` | `builds_and_reads_lists`, `changes_lists_in_place` |
| Records | `src/value.rs` `Fields` | `builds_and_reads_records`, `changes_existing_fields` |
| `for`, `break`, `continue` | `Code::IterInit`, `Code::ForNext` | `loops_over_lists_and_strings`, `breaks_and_continues` |
| `none` instead of `null` and `undefined` | `src/value.rs`, `src/checker.rs` `Optional` | `none_is_the_one_empty_value`, `nullable_types_need_a_fallback` |
| Modules | `src/vm.rs` `load_module` | `imports_a_whole_module` and the other `imports_*` tests |

## Phase 2: Speed

| Item | Where | Checked by |
|---|---|---|
| Benchmark suite | `benchmarks/`, `scripts/bench.py` | the tables below |
| Variables resolved to slots | `src/compiler.rs` `resolve` | all interpreter tests |
| Scopes skipped for blocks that declare nothing | `src/compiler.rs` `end_scope` emits `PopN` only when needed | `keeps_block_variables_inside_the_block` |
| Cheap-to-share strings | `Value::Str(Rc<String>)` | `updates_variables_in_place_safely` |
| Bytecode compiler | `src/compiler.rs` | everything |
| Virtual machine | `src/vm.rs` | everything |
| Faster than Python on every benchmark | see the sustained table below | `scripts/bench.py` |

## Phase 3: Types

| Item | Where | Checked by |
|---|---|---|
| Type annotations | `src/parser.rs` types, `src/checker.rs` | `accepts_type_annotations`, `rejects_mismatched_annotations` |
| Type checker before running | `Interpreter::execute` calls `Checker::check` first | `checks_types_before_anything_runs` |
| Inference for `let` | `src/checker.rs` | `infers_types_from_the_first_value` |
| Typed signatures | `src/checker.rs` `check_args` | `checks_function_signatures`, `checks_function_types` |
| Structs with methods | `Code::MakeStructDef`, `Code::Invoke` | `builds_structs_and_calls_methods`, `checks_struct_usage` |
| Enums and `match` | `Code::Test`, `src/checker.rs` match arms | `matches_enums`, `match_needs_every_case`, `matches_plain_values` |
| Generics | `src/checker.rs` `Type::Param`, `substitute` | `checks_generic_lists_and_functions` |

## Phase 4: Benchmarks

| Item | Where | Checked by |
|---|---|---|
| Median of 20+ runs with spread | `scripts/bench.py` (21 cold runs, 5 sustained) | `BENCHMARKS.md` |
| Cold-start and sustained | `benchmarks/suite.json` two sizes | `BENCHMARKS.md` |
| Peak memory | `scripts/bench.py` `run_once` reads `ru_maxrss` | memory table |
| CPU set | `benchmarks/cpu/` | tables |
| Microbenchmarks | `benchmarks/functions/` | tables |
| C and Rust versions | `benchmarks/**/*.c`, `*.rs` | `bench.py` checks all five print the same answer |
| Interleaved runs with raw results | `bench.py` shuffles; `benchmarks/results/` | files |
| BENCHMARKS.md with the losses | `BENCHMARKS.md` | reading it |

## Phase 5: Tooling

| Item | Where | Checked by |
|---|---|---|
| VS Code highlighting | `editors/vscode/syntaxes` | opening a `.kp` file (manual) |
| `.kp` icon | `editors/vscode/icons` | manual |
| Errors that point at the problem | `src/error.rs` `underline` | `error::tests` |
| `khipu fmt` | `src/format.rs` | `format::tests`, fuzz checks formatting is stable, pre-commit checks every `.kp` |
| `khipu test` | `src/main.rs` `test_files`, `Stmt::Test` | `skips_tests_in_a_normal_run_and_asserts`, `examples/testing.kp` in the hook |
| Language server | `src/lsp.rs` | `lsp::tests` |
| Web playground | `playground/`, `src/wasm.rs` | opened in a browser during this audit; `scripts/build-playground.sh` |
| Tree-sitter grammar | `editors/tree-sitter-khipu` | its corpus tests (`npx tree-sitter-cli test`); not rerun in this audit, the grammar did not change in Phase 8 |

## Phase 6: Concurrency

| Item | Where | Checked by |
|---|---|---|
| `spawn` | `src/task.rs` `spawn` | `runs_tasks_that_talk_over_channels` |
| Channels | `src/task.rs` `send`, `receive` | same |
| Every core | `src/task.rs` `workers` starts one thread per core | `parallel` benchmark |
| `select` | `src/parser.rs` `select_statement`, `task::select` | same test, `reports_every_error_with_its_line` |
| No shared mutable state | `task::pack` copies every value sent | the `copies` case in `runs_tasks_that_talk_over_channels` |

## Phase 7: Ecosystem

| Item | Where | Checked by |
|---|---|---|
| Standard library | `src/stdlib.rs`, `src/prelude.kp` | `has_a_standard_library` |
| Files and the terminal | `src/stdlib.rs` file natives | same test (writes, appends, lists, deletes a file) |
| HTTP | `src/http.rs` | `speaks_json_and_http` |
| Contracts | `src/parser.rs` `requires`/`ensures`/`example` desugaring | `checks_contracts`, `examples/contracts.kp` |
| Package manager | `src/package.rs` | `package::tests` |
| Compile to WASM for the web | `src/web.rs`, `src/wasm.rs`, `build.rs` | `web::tests`; a generated page was opened in a browser during this audit |

## Phase 8: Performance and final audit

| Item | Where | Checked by |
|---|---|---|
| Lazy `range()` | `List::Range` in `src/value.rs` | `ranges_count_without_building_a_list` (`len(range(100000000000))` in no time) |
| Garbage collector that frees cycles | `src/gc.rs` | `gc::tests`, including a program leaving 1 000 cycles behind |
| Peak memory below Python and Node | memory table below | `bench.py` |
| Faster function calls | `src/compiler.rs` `fuse`, `src/native.rs` | `fib`, `fib_iterative` rows below |
| Beat Python on every sustained benchmark | sustained table below | `bench.py` |
| Native compilation (Cranelift) | `src/kernel.rs`, `src/jit.rs`, `src/native.rs` | `jit::tests`, `hot_numeric_functions_run_natively_and_fall_back_when_needed`, `hot_loops_run_natively_with_the_same_results` |
| Compile straight to WebAssembly instructions | `src/wasmgen.rs`, `khipu web` | `tests/wasm_kernels.rs` runs the module in `node`; a page was timed in a browser (`fib(32)` 25 ms) |
| Competitive with Node | sustained table below, honestly read | `bench.py` |
| Start-up under 5 ms | `bench.py` start-up line | 3.9 ms median |
| Work stealing | `src/task.rs` `POOL`, idle workers | `parallel` benchmark, `runs_tasks_that_talk_over_channels` |
| Tasks take turns in deep recursion | `src/vm.rs` budget in `enter!` | `tasks_take_turns_inside_deep_recursion` |
| Fuzzing finds no crash | `tests/fuzz.rs` | 60 000 programs clean; 1 500 on every `cargo test` |
| Every error message tested | `reports_every_error_with_its_line`, `package::tests::explains_every_package_mistake`, `web::tests` | the remaining untested strings are `internal error` invariants and playground-only messages, listed below |
| No `unsafe`, or every use justified | three uses, each with a comment | `jit::tests` compare native with interpreted results |
| Every roadmap item re-checked | this file | |
| Full benchmark rerun | `README.md`, `BENCHMARKS.md` | `bench.py` on 2026-11-26 |
| Every example and doc snippet runs | `scripts/check-docs.py` | 26 snippets and examples, in the pre-commit hook |
| Independent review | last section | |

### Error messages without a test

These cannot be reached from a program, so they have no test:

- The `internal error: ...` messages in `src/vm.rs` and `src/compiler.rs` guard invariants the compiler establishes (a field name constant is always a string, a captured variable is always a cell, and so on).
- `only a struct can be built with { ... }` in `src/vm.rs`: the parser only accepts a struct name before `{`.
- The playground-only messages (`... is not available in the playground`) are compiled only for `wasm32`.
- The `needs a string` / `needs a number` checks in `src/stdlib.rs` and `src/http.rs` natives: every native is reached through a typed prelude wrapper, so a wrong type is reported by the method lookup first (`number has no method 'upper'`), which is tested.
- `the server stopped: ...` in `src/http.rs`: needs the listening socket to fail after it was opened.

## Benchmark results

`python3 scripts/bench.py` ran on 2026-11-26 after all the Phase 8 work; `README.md` holds the sustained table and `BENCHMARKS.md` the cold starts, spreads and memory, as it wrote them.

- Python: Khipu is faster on 17 of 17 sustained benchmarks, from 1.5× (`strings`) to 83× (`loop`).
- Node: Khipu is faster on 5 of 17 (`fib`, `loop`, `nested_loops`, `mandelbrot`, `locals`), within 1.6× on `lists` and `calls`, and 3 to 8× slower on the rest. The slow ones are the benchmarks the native tier does not reach: records, structs, strings, closures and globals, where the interpreter still does the work. Records got a shape (a shared list of field names plus a vector of values) and the field instructions got inline caches keyed by that shape, which took `records` from 197 to 160 ms and `record_access` from 365 to 209 ms; the rest of the gap is the interpreter's dispatch itself, so "Competitive with Node" is left open in the roadmap, with this result written next to it.
- Peak memory is below both Python and Node on every benchmark, including start-up.
- Start-up: 3.8 ms, under the 5 ms target. It was 2.6 ms before Cranelift was linked in; the difference is the larger binary.
- Measurements drift by a few percent between runs; `fib_iterative` measured 75 ms in a quick check and 119 ms in the full run on a warm machine.

## Independent review

A separate agent read the repository cold, without having written any of it, and reported fifteen findings. Each one is listed here with what happened to it. Fixed items have a regression test in `interpreter::tests::native_code_keeps_every_rule_of_the_interpreter`, `jit::tests` or `tests/wasm_kernels.rs`.

| # | Finding | Outcome |
|---|---|---|
| 1 | A native `for ... in range(x)` ran with a fractional, NaN or infinite bound where the interpreter errors with `range needs whole numbers` | Fixed: the kernel checks both bounds are finite whole numbers and otherwise fails back to the bytecode, which reports the error on the right line |
| 2 | A callee was looked up in the caller's globals, so a module's private helper could resolve to a same-named function in the importing program | Fixed: a callee is only accepted when it lives in the same globals store as the root function |
| 3 | A function called through another name (`let sq = square`) made the compiler index a map by the wrong key and panic | Fixed: the group keeps signatures by the name used at the call site |
| 4 | A loop that called a parameter or local with the same name as a global function called the global | Fixed: a loop with a call to anything but a global stays interpreted |
| 5 | Printing a value that holds itself more than once took exponential time and output | Fixed: printing tracks which containers it is inside and shows `...` at the first repeat. The nesting limit for equality and JSON went from 32 to 200 levels, and `assert_eq` reports the nesting error instead of a misleading mismatch |
| 6 | Redefining the built-in `range` after a kernel was compiled was not seen by that kernel | Fixed: replacing a global that holds a built-in invalidates compiled code, like replacing a function |
| 7 | `sorted` and `sort_by` could panic on a long mixed list because the comparator was not a total order | Fixed: the element types are checked before sorting |
| 8 | `khipu web` keyed loop kernels by owner and index, so two same-named functions with a numeric loop could swap kernels | Narrowed: the key now includes the loop's line; two same-named functions with a loop on the same line and the same variable count would still collide. Written down here |
| 9a | The README performance table predated the native tier | Fixed by the rerun below |
| 9b | The README miscounted the `unsafe` uses | Fixed: the sentence now lists them |
| 9c | The README said imports only look inside the project, but a `.kp` import can use `../` | Fixed: the sentence now says this about package imports, which is what the check covers |
| 9d | Native recursion depth was counted from zero instead of from the interpreter's current depth | Fixed: the interpreter passes its frame count into the native context (and into the WebAssembly module's `depth` global) |
| 10a | `-0 % 3` gave `+0` in WebAssembly | Fixed with `f64.copysign`, tested |
| 10b | Reassigning a global function in a loop recompiled kernels without bound, each compile leaking its code | Fixed in part: after 2 000 compiled groups the native tier stops compiling. Compiled code is still never freed; the bound keeps it finite |
| 10c | `let old = f` then `f = ...` then `old(n)`: a compiled `old` calls itself where the interpreter calls the new `f` | Fixed: a self call resolves through the globals again at compile time, and the generation bump recompiles after the reassignment |
| 10d | `range(-9e18, 9e18)` overflowed `len`, `repeat("x", 1e18)` aborted | Fixed: saturating length; `repeat` refuses results over 1 GB with an error |
| 10e | `spawn` wakes one idle worker; if that worker was already waking for a parked task, the new job waits until that worker is free, while other idle workers sleep | Written down: a latency loss with no deadlock or wrong result. The README describes the mechanism as a shared queue, not stealing of started tasks, which is accurate |

The review also checked the three `unsafe` sites, the collector's conservativeness, the scheduler's lock order and wake-ups, the fused instructions, both code generators and the contract desugaring, and found nothing wrong there.

## Phase 9 review

The HTTP server was rebuilt on a branch by one agent and reviewed cold by another. The suite at the end was 148 tests plus the HTTP fuzzer in `tests/http_fuzz.rs`, all green, and the `wrk` comparison against Node, Go and Rust is in BENCHMARKS.md under HTTP.

**Soak.** `scripts/soak-http.sh` ran twice. The first run, before the worker pool grew, was `wrk -t4 -c100` against `benchmarks/http/server.kp` for 43 minutes with one worker per core: resident memory 77 MB on average over the first fifteen minutes, 75 MB over the last thirteen, a sawtooth between 29 MB and 192 MB, and the `wrk` output lost with the process. The second run, on 2026-11-27 with `DURATION=5h` and the pool at eight workers per core (80 on this machine), served 2,572,768,676 requests in 300 minutes at 142,930 requests per second, average latency 551 µs, two timeouts and no other socket errors. Resident memory, sampled every minute for 300 samples, climbed from 68 MB to an average of 788 MB in the third hour and then held: 737 MB, 724 MB, 702 MB, 755 MB and 738 MB over the last five half hours, peak 994 MB. No unbounded growth over five hours, but the plateau is ten times the small pool's, consistent with eighty workers each holding their own buffers and garbage under 100 keep-alive connections; it has not been profiled. The 24 hour run the roadmap asks for has not happened; the command is the script with `DURATION=24h`. Non-blocking IO, on the roadmap, is what brings the memory back down: one loop instead of eighty workers.

**Non-blocking IO.** Each server now runs one event loop on the thread that called `Http.serve`, over `kqueue` on macOS and `epoll` on Linux through the `libc` crate (`src/poll.rs`, the only new `unsafe`, one block per system call). Connections are non-blocking from `accept` on and registered one-shot, so a connection is in exactly one place at a time: waiting in the loop, or being answered by a worker. The loop reads what has arrived, parses the request head and body in its own buffer, and hands a complete request to the pool; a worker runs the handler, writes the reply, and, when no other request is queued, waits up to a millisecond for the same connection's next request, which saves a trip through the loop under keep-alive load; otherwise it gives the connection back. Slow clients are timed by the loop, not by a blocked read: `read_timeout` for silence before the first request and in the middle of one, `idle_timeout` between requests, `request_timeout` from the first byte of a request, checked every 50 ms. The pool is now two workers per core between 4 and 64 (20 on this machine) and only holds requests being answered. WebSockets moved off the pool entirely: after the 101 the loop reads frames and puts each message on the socket's channel, and `chat(socket)` runs as a task on the scheduler, so a `receive` that waits parks the task instead of a thread. A silent socket gets a ping after `idle_timeout` times 12 and is closed with 1001 if another such period passes with nothing; a peer that answers pings stays open. Writes to a socket take a per-socket write lock and wait for the kernel buffer with `poll` without holding the stream, so the loop never waits behind a slow writer. On SIGINT or SIGTERM the loop stops accepting, closes open sockets with 1001 and the drain waits for requests and socket handlers still running. A test holds 2,000 idle connections and 200 open WebSockets, more than the pool's 20 workers, and checks a new request is answered in under two seconds and every socket still echoes.

**Measured after non-blocking IO**, on the same machine (Apple M5, 10 cores) on 2026-11-27. `scripts/bench-http.sh` with `wrk -t4 -c100 -d30s`, in BENCHMARKS.md: Khipu 133,514 / 136,417 / 91,218 requests per second on `/hello`, `/json` and `/file`, Node 124,162 / 121,900 / 124,314, Go 129,302 / 126,369 / 129,383, Rust-std 30,241 / 29,718 / 29,109. Against `main` built from f4af4fc, `wrk -t4 -c100 -d20s` on `/hello`: 137,815 requests per second at 0.70 ms average latency on the branch, 141,222 at 0.55 ms on `main`, no socket errors on either. Getting there took two corrections, both found by measuring: the first loop switched each socket between blocking and non-blocking twice a request, read a second time to see `WouldBlock` and sent every keep-alive request through the loop, about 10% slower than `main`; and a worker that waited for its own connection's next request without checking the queue kept 20 connections busy while the other 80 starved, which a 30 minute soak showed as 77 ms average latency and 16,239 timeouts. Both are fixed. With 10,000 idle connections held open by a script the server used 28 MB and 22 threads (20 workers, the loop and the main thread) and answered a new request in 27 ms; with 1,000 open WebSockets on top, 50 MB and 32 threads, every socket echoing. The same script against `main` starts at 66 MB and 82 threads and cannot open the 10,000: once its 80 workers each hold an idle connection, new connections are reset. The 30 minute soak (`PORT=18080 DURATION=30m scripts/soak-http.sh`) served 238,750,399 requests at 0.88 ms average latency with 661 timeouts and no other socket errors, and resident memory over 30 one-minute samples stayed between 24 MB and 138 MB, mostly 63 to 116 MB, against the 740 MB plateau of the eighty-worker pool. That run is not clean: the laptop was on battery and went to sleep from 16:48 to 17:09 in the middle of it, so wrk counted 73.4 minutes and its 54,213 requests per second is not a throughput figure; the timeouts may come from the sleep too. A clean rerun with sleep prevented is in the last section of this review.

**Three holes in the loop, found by an automated review of the commit and fixed.** A peer could send a WebSocket messages faster than its handler received them, and the queue grew without bound: a socket with 64 messages waiting is now held out of the loop, unread, until its handler is down to 32, so the kernel's buffers and TCP push back on the peer. The loop could block for up to `request_timeout` writing a 400 or a TLS handshake record to a client that does not read, stalling every connection: a connection now waits for buffer space only while a worker owns it, and the loop's own writes are best effort. And nothing bounded the number of connections, each of which may buffer a request head and body: past `max_connections`, 10,000 by default and changeable with the other limits, the loop stops accepting until one closes, and new connections wait in the kernel's backlog. Tested: a socket fed 70 messages is held and picked up again once 40 are received, and with `max_connections: 5` a sixth connection gets no answer until one of the five closes. What is left: the memory a full server can hold is still `max_connections` times `max_request_head` plus `max_body`, 100 GB with the defaults in the worst case, so a server open to the internet should lower `max_body` or `max_connections`; no total budget across connections exists.

**Starvation, found and narrowed.** The pool had one worker per core and each worker blocks reading its connection, so ten idle connections made the server unreachable to everyone until the 10 second read timeout. Verified by holding one idle connection per core and watching a fresh `curl` time out. The pool is now 8 workers per core, between 16 and 128, and a kept-alive connection waiting for its next request is cut off after an `idle_timeout` of 5 seconds, changeable with the other limits. A client now needs as many connections as there are workers to block the server, and holds it for one timeout per round. That was a documented limit, not a fix; non-blocking IO, above, is the fix. A test holds 200 idle connections and checks a new request is still answered.

**WebSockets.** A handler that sees `request.is_websocket()` returns `Response.websocket(chat)`; the server answers 101 and calls `chat(socket)` on the worker that owns the connection, where `socket` is a channel whose `send` writes a text frame and whose `receive` waits for the next message and gives `none` once the peer has closed. Returning from `chat` closes with 1000, throwing prints the error and closes with 1011. The frame reader follows RFC 6455 and refuses what it does not understand: unmasked client frames, reserved opcodes and RSV bits close with 1002, invalid UTF-8 with 1007, a message over `max_body` with 1009; pings get pongs and fragments are reassembled. A silent socket is pinged after `idle_timeout` times 12 (a minute by default) and closed with 1001 after a second such period. Tested with Node's own `WebSocket`, a raw client, and a fuzzer over random frames after a real handshake. Not done: compression (permessage-deflate is declined by not answering it), subprotocols, a client side, and binary messages, because Khipu has no bytes value; a binary frame that is valid UTF-8 arrives as a string and any other closes with 1003. Since non-blocking IO, above, a socket no longer holds a worker, a `send` no longer waits behind a `receive`, and shutdown closes open sockets with 1001 before the drain.

**Bytes.** `bytes` is a value of its own, `Value::Bytes(Rc<Vec<u8>>)`, with a `bytes` type in the checker and four methods in the prelude: `len`, `slice` (by byte, with the same negative and clamped bounds as strings and lists), `to_string`, which gives `none` for bytes that are not UTF-8, and `string.to_bytes()`. `==` compares contents, `print` shows `<6 bytes>`, and bytes cross `send` and `spawn` like strings. A `Request` keeps its body twice, `body` as text with invalid UTF-8 replaced and `bytes` exactly as sent; `Response.body` is now typed `any` and takes a string or bytes, and `Response.file` reads the file as bytes, so any file is served untouched. On a WebSocket, `send` of bytes writes a binary frame, a binary frame arrives as bytes whatever it contains, and a text frame that is not UTF-8 still closes with 1007; nothing closes with 1003 any more. Tested: the language-level methods and their errors, a POST of all 256 byte values echoed back from `request.bytes`, a file of all 256 values through `Response.file`, binary frames both ways with a raw client and with Node's `WebSocket` set to `arraybuffer`. Not done: indexing a single byte, building bytes from a list of numbers, `File.read_bytes` and `File.write_bytes` in the prelude (the native `read_bytes#` exists for `Response.file`), and binary bodies on the client side, where `Http.get` still gives a string.

**Close and select.** `close(socket)` sends a close frame with 1000, `close(socket, code)` and `close(socket, code, reason)` with the program's own; the code must be 1000 to 1003, 1007 to 1011 or 3000 to 4999 and the reason at most 123 bytes, and each wrong argument has its own error. After a close the server stops writing but keeps reading, so the peer's echoing close frame is read before the connection goes and Node reports the close as clean. A second close, or a close after the peer has gone, does nothing. `select` now takes sockets beside channels: a socket's messages wait in its channel, put there by the event loop, so the waiting is the same as for any channel. A wait that involves a socket is not counted by the deadlock check, since the network can always answer. Tested with a raw client (an echo that answers through `select`, then `close(socket, 4000, "see you")` and the exact frame on the wire), Node's `WebSocket` (code, reason and `wasClean`), and the error test in `tests/http_fuzz.rs` for a bad code and for `close` on a plain channel, each with its line. Not done: `close` on a plain channel, which is refused rather than given a meaning.

**Not done, on purpose.** Zero-copy parsing cannot reach Khipu values as the value model stands, written under the item. The soak is five hours, not 24.

