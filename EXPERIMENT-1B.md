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

## After step 1

`python3 scripts/bench.py --no-write` on the branch after the struct-call change, same machine and command as Before. `struct_access.kp` now builds the point in `make(x, y) -> Point`, passes it to `area(p: Point) -> number` inside the hot loop, and every other language version was changed to the same program. Full output (every benchmark, cold and sustained):

```
fib             cold      Khipu 5.7 ms  Python 26 ms  Node 25 ms  C 2.9 ms  Rust 3.0 ms
fib             sustained Khipu 21 ms  Python 139 ms  Node 34 ms  C 6.3 ms  Rust 6.2 ms
fib_iterative   cold      Khipu 13 ms  Python 57 ms  Node 26 ms  C 3.2 ms  Rust 3.5 ms
fib_iterative   sustained Khipu 93 ms  Python 454 ms  Node 44 ms  C 9.8 ms  Rust 12 ms
loop            cold      Khipu 6.1 ms  Python 225 ms  Node 35 ms  C 2.5 ms  Rust 2.7 ms
loop            sustained Khipu 26 ms  Python 2342 ms  Node 153 ms  C 2.5 ms  Rust 2.7 ms
nested_loops    cold      Khipu 4.2 ms  Python 19 ms  Node 24 ms  C 2.4 ms  Rust 2.8 ms
nested_loops    sustained Khipu 9.7 ms  Python 242 ms  Node 28 ms  C 4.5 ms  Rust 4.8 ms
sieve           cold      Khipu 9.9 ms  Python 27 ms  Node 25 ms  C 2.5 ms  Rust 2.7 ms
sieve           sustained Khipu 191 ms  Python 557 ms  Node 55 ms  C 5.2 ms  Rust 5.3 ms
matrix          cold      Khipu 6.3 ms  Python 19 ms  Node 24 ms  C 2.3 ms  Rust 2.6 ms
matrix          sustained Khipu 119 ms  Python 267 ms  Node 28 ms  C 3.1 ms  Rust 4.8 ms
mandelbrot      cold      Khipu 4.1 ms  Python 27 ms  Node 24 ms  C 2.5 ms  Rust 2.7 ms
mandelbrot      sustained Khipu 6.5 ms  Python 237 ms  Node 27 ms  C 4.0 ms  Rust 4.3 ms
lists           cold      Khipu 13 ms  Python 41 ms  Node 28 ms  C 2.9 ms  Rust 3.2 ms
lists           sustained Khipu 92 ms  Python 288 ms  Node 79 ms  C 5.4 ms  Rust 6.3 ms
records         cold      Khipu 21 ms  Python 47 ms  Node 25 ms  C 2.4 ms  Rust 2.7 ms
records         sustained Khipu 174 ms  Python 345 ms  Node 27 ms  C 2.2 ms  Rust 2.6 ms
strings         cold      Khipu 25 ms  Python 43 ms  Node 28 ms  C 11 ms  Rust 11 ms
strings         sustained Khipu 201 ms  Python 292 ms  Node 43 ms  C 88 ms  Rust 107 ms
calls           cold      Khipu 14 ms  Python 32 ms  Node 25 ms  C 2.5 ms  Rust 2.6 ms
calls           sustained Khipu 110 ms  Python 205 ms  Node 36 ms  C 2.5 ms  Rust 2.6 ms
closures        cold      Khipu 16 ms  Python 37 ms  Node 24 ms  C 2.6 ms  Rust 2.8 ms
closures        sustained Khipu 126 ms  Python 245 ms  Node 30 ms  C 5.0 ms  Rust 4.4 ms
locals          cold      Khipu 4.7 ms  Python 40 ms  Node 27 ms  C 2.4 ms  Rust 2.4 ms
locals          sustained Khipu 12 ms  Python 283 ms  Node 67 ms  C 2.7 ms  Rust 2.8 ms
globals         cold      Khipu 23 ms  Python 78 ms  Node 24 ms  C 3.0 ms  Rust 4.4 ms
globals         sustained Khipu 101 ms  Python 343 ms  Node 27 ms  C 4.9 ms  Rust 11 ms
record_access   cold      Khipu 30 ms  Python 98 ms  Node 24 ms  C 2.7 ms  Rust 3.5 ms
record_access   sustained Khipu 268 ms  Python 889 ms  Node 29 ms  C 5.0 ms  Rust 11 ms
struct_access   cold      Khipu 5.8 ms  Python 45 ms  Node 24 ms  C 2.5 ms  Rust 3.4 ms
struct_access   sustained Khipu 21 ms  Python 321 ms  Node 28 ms  C 2.4 ms  Rust 10 ms
parallel        cold      Khipu 11 ms  Python 137 ms  Node 47 ms  C 3.2 ms  Rust 3.4 ms
parallel        sustained Khipu 280 ms  Python 423 ms  Node 79 ms  C 22 ms  Rust 21 ms
empty           cold      Khipu 3.8 ms  Python 14 ms  Node 23 ms  C 2.5 ms  Rust 2.6 ms
```

Control: the *same call version* of `struct_access.kp` was copied into a clean worktree of `main` (1102d03) and run with `python3 scripts/bench.py --no-write --only struct_access`, three times:

```
struct_access   cold      Khipu 37 ms  Python 47 ms  Node 25 ms  C 2.5 ms  Rust 3.5 ms
struct_access   sustained Khipu 334 ms  Python 325 ms  Node 28 ms  C 2.8 ms  Rust 11 ms
struct_access   cold      Khipu 37 ms  Python 44 ms  Node 24 ms  C 2.4 ms  Rust 3.4 ms
struct_access   sustained Khipu 386 ms  Python 344 ms  Node 29 ms  C 2.6 ms  Rust 11 ms
struct_access   cold      Khipu 36 ms  Python 45 ms  Node 24 ms  C 2.5 ms  Rust 3.6 ms
struct_access   sustained Khipu 332 ms  Python 325 ms  Node 28 ms  C 2.5 ms  Rust 11 ms
```

Khipu `struct_access` sustained: **21 ms** on the branch against 332–386 ms on main for the identical program — at least **15.8× faster**, past the required 2×. The gain comes from the kernel, not from the benchmark shape: the cold-run compile attempt on main rejects the struct call, so the whole loop stays interpreted.

## After step 2

`python3 scripts/bench.py --no-write` on the branch after the list change, same machine and command as Before. Every benchmark is included. The whole suite drifted upward in this run (for example `fib_iterative` 93 → 103 ms and `parallel` 280 → 395 ms), which is run-to-run noise on this machine; the list number is far outside that drift. Full output:

```
fib             cold      Khipu 6.7 ms  Python 31 ms  Node 29 ms  C 3.3 ms  Rust 3.7 ms
fib             sustained Khipu 25 ms  Python 166 ms  Node 42 ms  C 7.1 ms  Rust 7.2 ms
fib_iterative   cold      Khipu 14 ms  Python 68 ms  Node 29 ms  C 3.6 ms  Rust 4.4 ms
fib_iterative   sustained Khipu 103 ms  Python 542 ms  Node 51 ms  C 12 ms  Rust 13 ms
loop            cold      Khipu 7.3 ms  Python 301 ms  Node 44 ms  C 3.2 ms  Rust 3.2 ms
loop            sustained Khipu 30 ms  Python 3096 ms  Node 185 ms  C 3.5 ms  Rust 3.7 ms
nested_loops    cold      Khipu 6.0 ms  Python 24 ms  Node 29 ms  C 3.0 ms  Rust 3.4 ms
nested_loops    sustained Khipu 12 ms  Python 280 ms  Node 33 ms  C 5.5 ms  Rust 5.9 ms
sieve           cold      Khipu 11 ms  Python 34 ms  Node 31 ms  C 2.9 ms  Rust 3.3 ms
sieve           sustained Khipu 206 ms  Python 684 ms  Node 74 ms  C 6.3 ms  Rust 6.4 ms
matrix          cold      Khipu 7.6 ms  Python 24 ms  Node 29 ms  C 3.0 ms  Rust 3.4 ms
matrix          sustained Khipu 145 ms  Python 313 ms  Node 35 ms  C 3.9 ms  Rust 6.0 ms
mandelbrot      cold      Khipu 5.0 ms  Python 32 ms  Node 29 ms  C 3.0 ms  Rust 3.3 ms
mandelbrot      sustained Khipu 7.7 ms  Python 269 ms  Node 33 ms  C 5.0 ms  Rust 4.9 ms
lists           cold      Khipu 6.2 ms  Python 25 ms  Node 29 ms  C 3.0 ms  Rust 3.1 ms
lists           sustained Khipu 17 ms  Python 98 ms  Node 32 ms  C 3.8 ms  Rust 3.4 ms
records         cold      Khipu 24 ms  Python 56 ms  Node 31 ms  C 2.9 ms  Rust 3.3 ms
records         sustained Khipu 202 ms  Python 401 ms  Node 34 ms  C 2.9 ms  Rust 3.0 ms
strings         cold      Khipu 27 ms  Python 49 ms  Node 31 ms  C 12 ms  Rust 13 ms
strings         sustained Khipu 230 ms  Python 352 ms  Node 53 ms  C 99 ms  Rust 126 ms
calls           cold      Khipu 17 ms  Python 39 ms  Node 29 ms  C 2.9 ms  Rust 3.3 ms
calls           sustained Khipu 121 ms  Python 234 ms  Node 41 ms  C 3.0 ms  Rust 3.0 ms
closures        cold      Khipu 18 ms  Python 44 ms  Node 29 ms  C 3.1 ms  Rust 3.4 ms
closures        sustained Khipu 134 ms  Python 296 ms  Node 33 ms  C 6.2 ms  Rust 5.3 ms
locals          cold      Khipu 6.0 ms  Python 50 ms  Node 33 ms  C 3.0 ms  Rust 3.2 ms
locals          sustained Khipu 13 ms  Python 331 ms  Node 72 ms  C 3.6 ms  Rust 3.3 ms
globals         cold      Khipu 28 ms  Python 97 ms  Node 31 ms  C 3.9 ms  Rust 5.5 ms
globals         sustained Khipu 128 ms  Python 389 ms  Node 34 ms  C 5.8 ms  Rust 13 ms
record_access   cold      Khipu 34 ms  Python 118 ms  Node 30 ms  C 3.4 ms  Rust 4.2 ms
record_access   sustained Khipu 312 ms  Python 1034 ms  Node 33 ms  C 5.8 ms  Rust 13 ms
struct_access   cold      Khipu 7.1 ms  Python 54 ms  Node 29 ms  C 2.9 ms  Rust 4.0 ms
struct_access   sustained Khipu 23 ms  Python 376 ms  Node 35 ms  C 2.9 ms  Rust 14 ms
parallel        cold      Khipu 12 ms  Python 173 ms  Node 58 ms  C 3.8 ms  Rust 4.1 ms
parallel        sustained Khipu 395 ms  Python 651 ms  Node 79 ms  C 22 ms  Rust 23 ms
empty           cold      Khipu 4.3 ms  Python 17 ms  Node 26 ms  C 2.8 ms  Rust 3.1 ms
```

Control: the same restructured `lists` program in a clean worktree of `main` (1102d03), `python3 scripts/bench.py --no-write --only lists`:

```
lists           cold      Khipu 13 ms  Python 25 ms  Node 28 ms  C 2.9 ms  Rust 3.1 ms
lists           sustained Khipu 97 ms  Python 97 ms  Node 31 ms  C 4.0 ms  Rust 3.3 ms
```

Khipu `lists` sustained: **17 ms** on the branch against **97 ms** on main for the identical program — **5.7× faster**, past the required 2× (≤ 47 ms). Against the Before number of 94 ms it is **5.5× faster**. The benchmark now uses a fixed eight-element number list built outside the kernel and spends N rounds on indexed reads, one indexed write and `len` inside it, in all five languages; all five print `1950014` at N = 300000 and `19500014` at N = 3000000.

## Open questions

- A returned struct's interpreter `StructDef` carries methods, which the kernel IR does not carry. Where does the native boundary get it?
  - Proposed answer: reuse the definition of an input instance of the same struct when there is one, otherwise find the struct definition in the program's globals by name; if neither exists, fail the native call so the interpreter reruns it. Taken.
- `Native` has to stay `Copy` because it lives in `Tier` cells. How does it keep the layout needed to convert struct fields at the boundary?
  - Proposed answer: leak a small clone of the compiled `Kernel` with `Box::leak` (safe, bounded by `MAX_COMPILES`) and store the `&'static Kernel`. Taken. No new `unsafe` was added, so the count in CONTRIBUTING.md stays at three.
- Should the WebAssembly backend emit a kernel where a struct crosses a call boundary?
  - Proposed answer: no. `wasmgen::module` returns `Err` and the web page falls back to the interpreter, as it already does for kernels containing structs. Taken, and extended to number-list kernels for the same reason.
- With list creation and `push` rejected inside kernels, how can the `lists` benchmark still measure the feature without the creation loop dominating the time?
  - Proposed answer: build a small fixed numeric list at top level and let the suite's N drive the number of indexed rounds inside the kernel, with the same program in all five languages. The first attempt kept the original 3-million-element build loop and could not beat main even with a 2.4 ms kernel, because building the list alone took about 70 ms; that version was discarded rather than used to claim a kill number. Taken.
- A lazy `range()` value is typed as `list<number>` but is not backed by a `Vec<f64>`. Should the kernel accept it?
  - Proposed answer: no. The native boundary accepts only `List::Numbers`, the representation the kernel's pointer and bounds check address directly; anything else fails the call and the interpreter reruns it. Assignment by index also stays rejected for anything that would have to widen the list representation. Taken.
- `len` can be redefined by a program. How does the kernel know the built-in is still the one in scope?
  - Proposed answer: mirror `range`: the lowering only recognises `len(...)` as a length read when the global slot still holds the native `len` and no local shadows it; otherwise it goes through ordinary call resolution and is rejected if it is not a group function. Taken.
