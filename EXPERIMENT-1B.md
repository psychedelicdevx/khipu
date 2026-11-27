# Experiment 1B — flat structs across calls, and lists of numbers

## Context

Second step of "Experimental phase 1: Near C on typed code" in ROADMAP.md. The two bets, measured separately:

1. Flat structs cross the call boundary. Inside a kernel group a struct is passed and returned as its separate field values; conversion to and from the interpreter's struct instance happens once at the kernel's outer boundary.
2. Lists whose elements the checker proves are numbers can be indexed, assigned by index and measured with `len` inside a kernel, backed by the existing list representation. A failed bounds check fails the kernel call the way a division trap does, so the interpreter reruns it and raises the normal error.

Rejection stays correct: a struct that is stored in a list, captured, or handed to a non-kernel function, a list that is created or appended to inside a kernel, and a list of anything but numbers all stay rejected.

## Machine and command

| | |
|---|---|
| date | 2026-11-27 |
| branch | `exp-1b-structs-and-lists` from `main` (1102d03) |
| cpu | Apple M5 |
| os | macOS-27.2-arm64-arm-64bit-Mach-O |
| python | Python 3.15.2 |
| node | v25.9.0 |
| c | Apple clang version 21.0.0 (clang-2100.3.34.2) |
| rust | rustc 1.100.0 (0ef1bc386 2026-11-09) |
| command | `python3 scripts/bench.py --no-write` |

Every run below is full-suite, with cold and sustained medians for all 18 benchmarks. The benchmark sources were changed to use the same function shape in all five languages before the first run, so each measurement compares like with like.

## Before

`python3 scripts/bench.py --no-write` on commit 8679f22, after the benchmark restructure and before any feature change. Full output (every benchmark, cold and sustained):

```
fib             cold      Khipu 5.5 ms  Python 26 ms  Node 24 ms  C 2.7 ms  Rust 2.9 ms
fib             sustained Khipu 21 ms  Python 143 ms  Node 35 ms  C 6.2 ms  Rust 6.3 ms
fib_iterative   cold      Khipu 12 ms  Python 59 ms  Node 26 ms  C 3.3 ms  Rust 3.6 ms
fib_iterative   sustained Khipu 86 ms  Python 498 ms  Node 47 ms  C 10 ms  Rust 12 ms
loop            cold      Khipu 6.1 ms  Python 238 ms  Node 36 ms  C 2.4 ms  Rust 2.7 ms
loop            sustained Khipu 27 ms  Python 2249 ms  Node 159 ms  C 3.0 ms  Rust 2.9 ms
nested_loops    cold      Khipu 4.4 ms  Python 20 ms  Node 24 ms  C 2.5 ms  Rust 2.7 ms
nested_loops    sustained Khipu 10.0 ms  Python 261 ms  Node 32 ms  C 4.5 ms  Rust 5.1 ms
sieve           cold      Khipu 9.7 ms  Python 29 ms  Node 27 ms  C 2.6 ms  Rust 2.8 ms
sieve           sustained Khipu 187 ms  Python 594 ms  Node 55 ms  C 5.2 ms  Rust 5.6 ms
matrix          cold      Khipu 6.2 ms  Python 20 ms  Node 24 ms  C 2.4 ms  Rust 2.7 ms
matrix          sustained Khipu 120 ms  Python 273 ms  Node 30 ms  C 3.4 ms  Rust 5.1 ms
mandelbrot      cold      Khipu 4.3 ms  Python 29 ms  Node 25 ms  C 2.5 ms  Rust 2.7 ms
mandelbrot      sustained Khipu 6.7 ms  Python 269 ms  Node 29 ms  C 4.2 ms  Rust 4.4 ms
lists           cold      Khipu 13 ms  Python 43 ms  Node 29 ms  C 3.0 ms  Rust 3.3 ms
lists           sustained Khipu 94 ms  Python 327 ms  Node 90 ms  C 5.8 ms  Rust 6.2 ms
records         cold      Khipu 22 ms  Python 52 ms  Node 28 ms  C 2.6 ms  Rust 2.9 ms
records         sustained Khipu 189 ms  Python 368 ms  Node 28 ms  C 2.6 ms  Rust 2.9 ms
strings         cold      Khipu 24 ms  Python 44 ms  Node 28 ms  C 11 ms  Rust 11 ms
strings         sustained Khipu 214 ms  Python 307 ms  Node 47 ms  C 90 ms  Rust 114 ms
calls           cold      Khipu 10 ms  Python 35 ms  Node 26 ms  C 2.6 ms  Rust 2.8 ms
calls           sustained Khipu 65 ms  Python 205 ms  Node 37 ms  C 2.7 ms  Rust 2.6 ms
closures        cold      Khipu 16 ms  Python 39 ms  Node 25 ms  C 2.7 ms  Rust 2.9 ms
closures        sustained Khipu 127 ms  Python 249 ms  Node 30 ms  C 5.3 ms  Rust 5.1 ms
locals          cold      Khipu 5.0 ms  Python 45 ms  Node 29 ms  C 2.6 ms  Rust 2.6 ms
locals          sustained Khipu 12 ms  Python 301 ms  Node 69 ms  C 2.5 ms  Rust 3.2 ms
globals         cold      Khipu 24 ms  Python 83 ms  Node 25 ms  C 3.0 ms  Rust 4.5 ms
globals         sustained Khipu 101 ms  Python 341 ms  Node 29 ms  C 5.2 ms  Rust 12 ms
record_access   cold      Khipu 29 ms  Python 100 ms  Node 24 ms  C 2.7 ms  Rust 3.4 ms
record_access   sustained Khipu 247 ms  Python 873 ms  Node 28 ms  C 4.9 ms  Rust 11 ms
struct_access   cold      Khipu 4.5 ms  Python 32 ms  Node 24 ms  C 2.5 ms  Rust 3.4 ms
struct_access   sustained Khipu 11 ms  Python 204 ms  Node 28 ms  C 5.0 ms  Rust 11 ms
parallel        cold      Khipu 9.9 ms  Python 122 ms  Node 44 ms  C 2.9 ms  Rust 3.2 ms
parallel        sustained Khipu 233 ms  Python 368 ms  Node 70 ms  C 21 ms  Rust 21 ms
empty           cold      Khipu 3.3 ms  Python 13 ms  Node 22 ms  C 2.2 ms  Rust 2.4 ms
```

Kill-number baseline for step 1 is the same program with a struct passed through a call in the hot loop, run on `main`; the control is recorded below with the step-1 result. Step 2's baseline is `lists` sustained = **94 ms** here, so the branch needs **≤ 47 ms** to pass.
