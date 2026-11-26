# Experiment 1 — flat typed structs inside kernels

## Context

First step of the first item of "Experimental phase 1: Near C on typed code" in ROADMAP.md:

> Whole-program typed AOT: extend the kernel IR from numbers and bools to records, structs, lists, strings and closures, so every function whose types the checker proves compiles to native code, with bytecode only for what stays dynamic

This experiment extends the kernel IR with **flat typed structs**: a struct whose fields are all number or bool, created, read and written entirely inside one kernel function, becomes plain memory with field offsets known at compile time. One load or store per field, no shape check. A struct that is passed in, returned, stored in a list, or captured makes the kernel reject the function, which stays on the bytecode path.

## How a number-only function gets from bytecode to Cranelift today

This is my own description of the current pipeline, file by file.

### 1. The program is compiled to bytecode

`src/compiler.rs` turns the checked AST into a `Proto`: a vector of stack-machine `Code` instructions, a constant pool, and nested child protos for functions. Every variable is resolved to a slot (local, cell for captures, or global). `src/vm.rs` runs this bytecode in a tight loop with a frame stack and a global store.

### 2. A hot function is a candidate for native code

Each `Proto` carries a `tier: Cell<Tier>`. It starts as `Tier::Counting(0)`. On every call, `src/native.rs` `entry()` bumps the count. After `CALLS_BEFORE_COMPILING` (64) calls — or immediately if `source.looping` is set — it tries to compile. Two things reject a function outright here:

- it has captures (closures over locals stay interpreted),
- `attempt()` fails to build a kernel group.

### 3. The function and its callees become a kernel group

`attempt()` looks up the root function's `Source` (captured at compile time in `src/compiler.rs` `function()`), then calls `kernel::group()`. That walks the function and every function it calls, resolving callees by name through a `Lookup` that only accepts top-level functions in the same globals store with no captures. The fixed-point loop in `resolve_group()` keeps pulling in callees until the whole mutually-recursive group is known, capped at 64 functions.

Each function is lowered by `kernel::lower()` to a `Kernel`: a tiny typed IR (`Stmt`/`Expr`) where every value is `Kind::Num` or `Kind::Bool`. The lowering rejects anything it cannot represent:

- reading or writing a global variable,
- anything but a number or bool (strings, lists, records, structs, enums, `none`),
- `match`, nested functions, `??`,
- loops other than `for x in range(...)`,
- calling anything but a known global function in the group,
- more than 16 parameters,
- any code path that ends without returning a value.

`kernel::group_all()` (used by `src/wasmgen.rs` and `src/web.rs`) does the same for every top-level function plus every hot loop, discovered by walking the compiled `Proto` for `Code::Loop` sites.

### 4. Cranelift compiles the kernel group

`src/jit.rs` `compile()` builds one Cranelift `JITModule`. For each kernel it emits two functions:

- `k{index}` — the kernel itself. Signature: `(context ptr, arg ptr…) -> f64` (plus the loop-variable array for loops). It bumps a depth counter at entry, fails with `error=1` past `MAX_DEPTH` (10,000), runs the lowered IR with Cranelift variables as locals, and returns `f64` (bools are i8 internally, converted at the boundary).
- `e{index}` — the entry wrapper. Signature: `(context ptr, *mut f64 args) -> f64`. It loads each argument from the pointer array (converting bools from f64), calls `k{index}`, converts the result back (`f64` or `bool != 0`), and returns.

`remainder()` (modulo) is special-cased: fast integer path when both operands are whole and small, otherwise an imported C helper `khipu_remainder`.

The module is finalized and **leaked** (`Box::leak`) so the machine code stays mapped for the rest of the process.

### 5. The VM switches to the native code

`install()` wraps each compiled entry in a `Native` (arity, a `bool_params` bitmask, `returns_bool`, and a `generation`) and stores `Tier::Native(native)` on the root proto. Later calls go through `native::call()`: arguments are packed into a raw `[f64; 16]`, the target is called, and the result is unpacked. If the context's error flag is set (stack overflow, division trap), the call returns `None` and the VM falls back to the interpreter bytecode for that invocation.

Loops take the same path: `vm.rs` `Code::Loop` calls `native::loop_entry()`, which compiles the loop body as a kernel whose parameters are the loop's live variables, runs it, and writes the updated values back to the slots.

### Summary of today's boundary

| Compiles to native | Stays on bytecode |
|---|---|
| Top-level functions over numbers/bools only | Functions with captures |
| `for x in range(...)` loops over numbers/bools | Globals read or written |
| Calls to other functions in the same group | Strings, lists, records, structs, enums |
| No `match`, no `??`, no nested functions | `match`, `??`, nested functions |
| All paths return a number or bool | Missing return on some path |

The experiment adds **flat typed structs** to the left column — but only when the whole struct life (create, field read, field write) stays inside a single kernel function.

## Plan

1. Write this document (done — this section).
2. Create branch `exp-1-typed-aot` from `main`, pre-commit hook on.
3. Run `python3 scripts/bench.py` on main → "Before" in this doc.
4. Extend `src/kernel.rs` IR: a struct type whose fields are `Num`/`Bool`, with `Stmt::MakeStruct`, `Stmt::GetField`, `Stmt::SetField` over fixed offsets. Reject structs that escape (parameters, returns, lists, captures).
5. Extend `src/jit.rs` to lower the new IR to Cranelift (plain loads/stores at fixed offsets).
6. Make `src/wasmgen.rs` reject (not crash on) the new IR and keep its tests passing.
7. Add JIT tests (interpreter vs native comparison), including a wrong-field/wrong-type rejection case.
8. Run `python3 scripts/bench.py` on the branch → "After" in this doc.
9. Kill number check: `struct_access` sustained must be ≥ 2× faster than main. If not, stop and write why.
10. `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `python3 scripts/check-docs.py`.
11. Report what landed, before/after table, which functions newly compile, which were rejected and why, open questions.

## Before (main)

`python3 scripts/bench.py --no-write` on commit 947c38e (main), working tree clean.

| Benchmark | Khipu sustained | Node sustained | C sustained | Rust sustained |
|---|---|---|---|---|
| fib | 20 ms | 33 ms | 6.0 ms | 5.9 ms |
| fib_iterative | 125 ms | 42 ms | 9.4 ms | 11 ms |
| loop | 26 ms | 145 ms | 2.4 ms | 2.8 ms |
| nested_loops | 9.1 ms | 26 ms | 4.2 ms | 4.3 ms |
| sieve | 183 ms | 57 ms | 5.1 ms | 5.7 ms |
| matrix | 120 ms | 30 ms | 3.5 ms | 5.1 ms |
| mandelbrot | 6.2 ms | 26 ms | 3.9 ms | 4.0 ms |
| lists | 92 ms | 74 ms | 5.4 ms | 6.4 ms |
| records | 174 ms | 28 ms | 2.6 ms | 2.7 ms |
| strings | 207 ms | 47 ms | 88 ms | 109 ms |
| calls | 60 ms | 36 ms | 2.5 ms | 2.6 ms |
| closures | 116 ms | 28 ms | 4.9 ms | 4.3 ms |
| locals | 12 ms | 67 ms | 2.6 ms | 2.8 ms |
| globals | 94 ms | 26 ms | 4.8 ms | 11 ms |
| record_access | 261 ms | 30 ms | 5.0 ms | 11 ms |
| struct_access | 248 ms | 27 ms | 5.0 ms | 11 ms |
| parallel | 283 ms | 69 ms | 21 ms | 22 ms |

Kill-number baseline: `struct_access` sustained = **248 ms** on main. The branch must reach **≤ 124 ms** sustained to pass.

Full raw output is in `/tmp/opencode/bench-before.txt`.

## After (exp-1-typed-aot)

<details>

</details>

## Machine and command

| | |
|---|---|
| date | 2026-11-26 |
| khipu | 947c38e (main) |
| cpu | Apple M5 |
| os | macOS-27.2-arm64-arm-64bit-Mach-O |
| python | Python 3.15.2 |
| node | v25.9.0 |
| c | Apple clang version 21.0.0 (clang-2100.3.34.2) |
| rust | rustc 1.100.0 |
| command | `python3 scripts/bench.py --no-write` |

## Kill number

`sustained struct_access` on the branch must be at least 2× faster than main: ≤ 124 ms. If it is not, I stop, write why here, and leave the branch for review.

## Open questions

- Should field reads/writes on a struct value that crosses a call boundary be allowed if the callee is also a kernel? (Rejected for this step: any escape rejects the function.)
- How does `wasmgen.rs` handle a kernel that contains a struct? It must not panic; falling back to bytecode for the whole program is acceptable for this step.


## Kill number

`sustained struct_access` on the branch must be at least 2× faster than `sustained struct_access` on main. If it is not, I stop, write why here, and leave the branch for review.

## Open questions

- Should field reads/writes on a struct value that crosses a call boundary be allowed if the callee is also a kernel? (Rejected for this step: any escape rejects the function.)
- How does `wasmgen.rs` handle a kernel that contains a struct? It must not panic; falling back to bytecode for the whole program is acceptable for this step.
