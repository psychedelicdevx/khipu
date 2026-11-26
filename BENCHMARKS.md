# Khipu Benchmarks

How fast Khipu is, measured honestly against Python, Node, C and Rust, including where it loses.

## What is measured

Every benchmark in `benchmarks/` is written five times: Khipu, Python, JavaScript (Node), C and Rust. Each version uses the same algorithm, the same inputs and prints the same answer. `benchmarks/suite.json` lists them with two input sizes:

- **Cold start** uses a small input, where start-up time matters. This is what a short script feels like.
- **Sustained** uses an input large enough that start-up is a small share of the time, and gives Node's JIT time to warm up. This is what a long-running program feels like.

| Group | Benchmarks |
|---|---|
| CPU | recursive and iterative Fibonacci, a plain loop, nested loops, prime sieve, matrix multiplication, Mandelbrot |
| Functions | call overhead, closures, local variables, global variables, record field access, struct field access |
| Collections | building and summing a list, creating records |
| Strings | interpolation, joining and measuring strings |
| Start-up | an empty program |

## Method

- **Median, not best.** Cold runs take the median of 21 runs, sustained runs the median of 5. The fastest and slowest runs are reported next to each median as the spread.
- **Start-up is included** in every time, because that is what a user waits for. The empty-program benchmark shows how much of each time is start-up alone.
- **Peak memory** is the highest resident memory of the process, read from the operating system for every run.
- **Interleaved order.** Each round runs the languages in a shuffled order, so heat or background activity drifting during a run does not favour one language.
- **One warm-up run per program is discarded.** macOS checks every newly built executable the first time it launches, which would otherwise add tens of milliseconds to one run.
- **Absolute times drift.** Long runs heat the machine and it may slow down; interleaving keeps the comparison fair, but expect a few percent of variation between full runs.
- **Answers are checked.** A benchmark only reports times if all five languages print the same answer on every run.
- **Compilation is not timed.** C is built with `clang -O2 -ffp-contract=off` (no fused multiply-add, so floating-point results match the other languages exactly) and Rust with `rustc -C opt-level=3`. Khipu runs its release build, which includes compiling to bytecode at start-up.
- **Fair, not identical.** C and Rust use native integers where the answer is a whole number; Khipu, Python and Node use their normal number types. C and Rust stand in for the hardware's limit rather than as a like-for-like competitor.

Run it yourself:

```bash
python3 scripts/bench.py
```

`--quick` runs fewer samples, `--only fib,loop` picks benchmarks, `--cold-only` skips the long runs and `--no-write` leaves the files untouched.

## Results

### Environment

| | |
|---|---|
| date | 2026-11-26T03:55:54 |
| khipu | 06a52dd (uncommitted changes) |
| cpu | Apple M5 |
| os | macOS-27.2-arm64-arm-64bit-Mach-O |
| python | Python 3.15.2 |
| node | v25.9.0 |
| c | Apple clang version 21.0.0 (clang-2100.3.34.2) |
| rust | rustc 1.95.0 (59807616e 2026-04-14) |

### Start-up

An empty program, median of 21 runs: Khipu 2.6 ms · Python 13 ms · Node 22 ms · C 2.1 ms · Rust 2.4 ms.

### Sustained

Median of 5 runs, with the fastest and slowest run in brackets.

| Benchmark | Khipu | Python | Node | C | Rust | vs Python | vs Node |
|---|---|---|---|---|---|---|---|
| fib | 148 ms (145–153) | 136 ms (134–142) | 35 ms (33–37) | 6.1 ms (6–6) | 6.1 ms (6–7) | 1.1× slower | 4.3× slower |
| fib_iterative | 621 ms (597–682) | 467 ms (452–489) | 45 ms (44–53) | 9.7 ms (10–11) | 10 ms (10–11) | 1.3× slower | 13.7× slower |
| loop | 310 ms (295–464) | 2294 ms (2090–2396) | 156 ms (143–165) | 2.7 ms (3–3) | 2.9 ms (3–5) | **7.4× faster** | 2.0× slower |
| nested_loops | 178 ms (176–179) | 234 ms (229–242) | 28 ms (27–34) | 4.4 ms (4–5) | 4.7 ms (5–5) | **1.3× faster** | 6.3× slower |
| sieve | 322 ms (305–331) | 527 ms (508–571) | 51 ms (50–58) | 4.9 ms (5–5) | 5.4 ms (5–5) | **1.6× faster** | 6.4× slower |
| matrix | 150 ms (134–181) | 272 ms (252–641) | 30 ms (28–41) | 3.3 ms (3–4) | 4.8 ms (5–6) | **1.8× faster** | 5.0× slower |
| mandelbrot | 76 ms (74–90) | 225 ms (212–255) | 26 ms (25–30) | 3.8 ms (4–7) | 4.4 ms (4–5) | **3.0× faster** | 2.9× slower |
| lists | 114 ms (110–115) | 286 ms (267–301) | 78 ms (76–105) | 5.1 ms (5–6) | 6.1 ms (6–9) | **2.5× faster** | 1.5× slower |
| records | 218 ms (214–235) | 346 ms (330–363) | 27 ms (26–28) | 2.1 ms (2–3) | 2.7 ms (2–5) | **1.6× faster** | 8.1× slower |
| strings | 216 ms (213–218) | 287 ms (270–313) | 40 ms (39–43) | 83 ms (82–83) | 105 ms (104–105) | **1.3× faster** | 5.4× slower |
| calls | 75 ms (71–89) | 191 ms (186–192) | 34 ms (34–35) | 2.4 ms (2–3) | 2.2 ms (2–3) | **2.6× faster** | 2.2× slower |
| closures | 125 ms (122–128) | 234 ms (225–250) | 27 ms (25–36) | 4.6 ms (4–5) | 4.2 ms (4–4) | **1.9× faster** | 4.6× slower |
| locals | 134 ms (120–184) | 275 ms (269–282) | 65 ms (62–67) | 2.3 ms (2–3) | 2.6 ms (2–3) | **2.1× faster** | 2.1× slower |
| globals | 102 ms (93–116) | 342 ms (325–355) | 27 ms (26–31) | 4.7 ms (5–5) | 11 ms (11–12) | **3.4× faster** | 3.7× slower |
| record_access | 287 ms (270–289) | 819 ms (804–854) | 25 ms (25–33) | 4.9 ms (4–5) | 6.5 ms (6–7) | **2.9× faster** | 11.4× slower |
| struct_access | 270 ms (255–271) | 719 ms (698–727) | 26 ms (25–31) | 5.1 ms (5–5) | 6.6 ms (6–7) | **2.7× faster** | 10.5× slower |

### Cold start

Small inputs where start-up time matters. Median of 21 runs.

| Benchmark | Khipu | Python | Node | C | Rust | vs Python | vs Node |
|---|---|---|---|---|---|---|---|
| fib | 16 ms (15–18) | 26 ms (24–31) | 24 ms (23–28) | 2.9 ms (2–3) | 3.0 ms (2–3) | **1.6× faster** | **1.5× faster** |
| fib_iterative | 64 ms (61–82) | 60 ms (56–80) | 28 ms (25–46) | 3.5 ms (3–16) | 3.5 ms (3–14) | 1.1× slower | 2.3× slower |
| loop | 38 ms (31–51) | 247 ms (236–291) | 39 ms (34–51) | 2.7 ms (2–4) | 2.8 ms (3–12) | **6.5× faster** | **1.0× faster** |
| nested_loops | 6.9 ms (6–10) | 20 ms (18–25) | 26 ms (21–32) | 2.7 ms (2–4) | 2.8 ms (2–6) | **2.9× faster** | **3.7× faster** |
| sieve | 11 ms (10–12) | 27 ms (26–30) | 26 ms (23–35) | 2.5 ms (2–3) | 2.6 ms (2–6) | **2.4× faster** | **2.3× faster** |
| matrix | 5.5 ms (5–8) | 19 ms (18–33) | 24 ms (21–29) | 2.3 ms (2–4) | 2.5 ms (2–3) | **3.4× faster** | **4.4× faster** |
| mandelbrot | 7.1 ms (6–8) | 27 ms (25–29) | 23 ms (22–31) | 2.3 ms (2–4) | 2.7 ms (2–3) | **3.7× faster** | **3.3× faster** |
| lists | 14 ms (13–15) | 42 ms (38–120) | 29 ms (26–34) | 2.8 ms (2–9) | 3.3 ms (3–4) | **3.0× faster** | **2.1× faster** |
| records | 25 ms (24–29) | 47 ms (43–115) | 26 ms (23–49) | 2.4 ms (2–3) | 2.6 ms (2–3) | **1.9× faster** | **1.0× faster** |
| strings | 24 ms (23–25) | 40 ms (39–42) | 25 ms (24–28) | 10 ms (10–11) | 11 ms (10–14) | **1.6× faster** | **1.0× faster** |
| calls | 9.4 ms (9–10) | 30 ms (29–32) | 23 ms (21–24) | 2.2 ms (2–2) | 2.4 ms (2–3) | **3.2× faster** | **2.4× faster** |
| closures | 15 ms (14–16) | 35 ms (34–39) | 23 ms (21–25) | 2.6 ms (2–3) | 2.6 ms (2–4) | **2.4× faster** | **1.6× faster** |
| locals | 17 ms (14–19) | 39 ms (38–49) | 26 ms (25–28) | 2.3 ms (2–3) | 2.5 ms (2–3) | **2.3× faster** | **1.5× faster** |
| globals | 26 ms (20–34) | 75 ms (73–109) | 23 ms (22–42) | 2.8 ms (2–3) | 4.2 ms (4–6) | **2.9× faster** | 1.1× slower |
| record_access | 31 ms (30–34) | 95 ms (91–143) | 23 ms (21–25) | 2.6 ms (2–3) | 2.8 ms (2–3) | **3.0× faster** | 1.4× slower |
| struct_access | 31 ms (28–33) | 85 ms (80–109) | 23 ms (22–37) | 2.5 ms (2–5) | 2.9 ms (2–6) | **2.8× faster** | 1.3× slower |

### Peak memory

Highest resident memory of any sustained run (cold run for start-up).

| Benchmark | Khipu | Python | Node | C | Rust |
|---|---|---|---|---|---|
| fib | 2.1 MiB | 12.8 MiB | 52.0 MiB | 1.6 MiB | 1.8 MiB |
| fib_iterative | 2.1 MiB | 12.9 MiB | 53.3 MiB | 1.6 MiB | 1.8 MiB |
| loop | 2.1 MiB | 12.8 MiB | 54.3 MiB | 1.6 MiB | 1.8 MiB |
| nested_loops | 2.2 MiB | 12.8 MiB | 52.4 MiB | 1.6 MiB | 1.8 MiB |
| sieve | 141.6 MiB | 35.8 MiB | 80.0 MiB | 4.5 MiB | 4.7 MiB |
| matrix | 3.5 MiB | 13.4 MiB | 55.2 MiB | 2.0 MiB | 2.2 MiB |
| mandelbrot | 2.2 MiB | 12.9 MiB | 54.0 MiB | 1.6 MiB | 1.8 MiB |
| lists | 141.6 MiB | 129.8 MiB | 226.4 MiB | 26.6 MiB | 26.8 MiB |
| records | 63.2 MiB | 12.9 MiB | 55.0 MiB | 1.6 MiB | 1.8 MiB |
| strings | 63.2 MiB | 12.8 MiB | 60.5 MiB | 1.6 MiB | 1.9 MiB |
| calls | 93.7 MiB | 12.8 MiB | 54.2 MiB | 1.6 MiB | 1.8 MiB |
| closures | 154.7 MiB | 12.9 MiB | 52.6 MiB | 1.6 MiB | 1.8 MiB |
| locals | 2.1 MiB | 12.9 MiB | 54.2 MiB | 1.6 MiB | 1.8 MiB |
| globals | 2.1 MiB | 12.8 MiB | 52.6 MiB | 1.6 MiB | 1.8 MiB |
| record_access | 2.1 MiB | 12.8 MiB | 52.4 MiB | 1.6 MiB | 1.8 MiB |
| struct_access | 2.1 MiB | 12.9 MiB | 52.4 MiB | 1.6 MiB | 1.8 MiB |
| empty | 2.0 MiB | 12.8 MiB | 48.3 MiB | 1.6 MiB | 1.8 MiB |

Raw samples for every run are in [`benchmarks/results/latest.json`](benchmarks/results/latest.json).
