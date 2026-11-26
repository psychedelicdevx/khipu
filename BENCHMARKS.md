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
- **Parallel uses each language's own way to use every core.** The work is split into 8 independent pieces: Khipu uses `spawn` and a channel, Python a `ProcessPoolExecutor` (separate processes, since threads share one interpreter lock), Node `worker_threads`, C `pthread`, and Rust scoped threads. Start-up of the workers is part of the time.
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
| date | 2026-11-26T20:11:06 |
| khipu | 0099263 (uncommitted changes) |
| cpu | Apple M5 |
| os | macOS-27.2-arm64-arm-64bit-Mach-O |
| python | Python 3.15.2 |
| node | v25.9.0 |
| c | Apple clang version 21.0.0 (clang-2100.3.34.2) |
| rust | rustc 1.95.0 (59807616e 2026-04-14) |

### Start-up

An empty program, median of 21 runs: Khipu 3.3 ms · Python 13 ms · Node 21 ms · C 2.1 ms · Rust 2.3 ms.

### Sustained

Median of 5 runs, with the fastest and slowest run in brackets.

| Benchmark | Khipu | Python | Node | C | Rust | vs Python | vs Node |
|---|---|---|---|---|---|---|---|
| fib | 21 ms (20–21) | 145 ms (133–150) | 34 ms (32–35) | 6.0 ms (6–6) | 6.2 ms (6–9) | **7.0× faster** | **1.6× faster** |
| fib_iterative | 123 ms (120–147) | 425 ms (419–433) | 42 ms (41–45) | 9.7 ms (9–16) | 9.8 ms (10–10) | **3.5× faster** | 2.9× slower |
| loop | 24 ms (23–42) | 2039 ms (1980–2223) | 147 ms (143–150) | 2.4 ms (2–6) | 2.6 ms (2–3) | **84.4× faster** | **6.1× faster** |
| nested_loops | 9.5 ms (9–11) | 233 ms (226–243) | 26 ms (24–47) | 4.2 ms (4–5) | 4.5 ms (4–5) | **24.5× faster** | **2.8× faster** |
| sieve | 164 ms (161–170) | 506 ms (498–538) | 50 ms (50–52) | 4.9 ms (5–6) | 4.9 ms (5–5) | **3.1× faster** | 3.2× slower |
| matrix | 110 ms (108–136) | 248 ms (241–251) | 28 ms (26–28) | 3.0 ms (3–3) | 4.5 ms (4–5) | **2.3× faster** | 4.0× slower |
| mandelbrot | 6.3 ms (6–12) | 223 ms (210–230) | 24 ms (23–36) | 3.7 ms (3–7) | 3.9 ms (3–6) | **35.5× faster** | **3.8× faster** |
| lists | 88 ms (87–91) | 275 ms (262–293) | 80 ms (72–88) | 4.6 ms (4–5) | 6.3 ms (5–8) | **3.1× faster** | 1.1× slower |
| records | 160 ms (158–182) | 319 ms (312–323) | 25 ms (25–27) | 2.3 ms (2–2) | 2.5 ms (2–8) | **2.0× faster** | 6.3× slower |
| strings | 200 ms (194–202) | 270 ms (270–273) | 39 ms (39–46) | 82 ms (81–83) | 105 ms (104–106) | **1.4× faster** | 5.1× slower |
| calls | 54 ms (52–79) | 187 ms (183–188) | 34 ms (33–35) | 2.3 ms (2–3) | 2.2 ms (2–3) | **3.5× faster** | 1.6× slower |
| closures | 100 ms (99–103) | 224 ms (219–231) | 28 ms (25–50) | 4.7 ms (4–5) | 4.1 ms (4–5) | **2.2× faster** | 3.6× slower |
| locals | 11 ms (10–11) | 262 ms (261–265) | 62 ms (62–85) | 2.4 ms (2–5) | 2.4 ms (2–3) | **24.7× faster** | **5.8× faster** |
| globals | 94 ms (93–95) | 318 ms (311–341) | 26 ms (24–27) | 4.8 ms (4–5) | 11 ms (11–11) | **3.4× faster** | 3.5× slower |
| record_access | 209 ms (207–238) | 801 ms (797–848) | 26 ms (24–30) | 4.9 ms (5–6) | 6.4 ms (6–7) | **3.8× faster** | 8.2× slower |
| struct_access | 220 ms (208–269) | 746 ms (730–959) | 27 ms (25–34) | 4.9 ms (5–9) | 6.3 ms (6–7) | **3.4× faster** | 8.2× slower |
| parallel | 219 ms (219–284) | 398 ms (385–434) | 69 ms (67–72) | 21 ms (20–28) | 22 ms (21–25) | **1.8× faster** | 3.2× slower |

### Cold start

Small inputs where start-up time matters. Median of 21 runs.

| Benchmark | Khipu | Python | Node | C | Rust | vs Python | vs Node |
|---|---|---|---|---|---|---|---|
| fib | 5.2 ms (5–6) | 24 ms (23–42) | 23 ms (22–47) | 2.6 ms (2–3) | 2.8 ms (2–5) | **4.6× faster** | **4.4× faster** |
| fib_iterative | 16 ms (14–20) | 56 ms (53–65) | 25 ms (23–42) | 3.3 ms (3–5) | 3.3 ms (3–7) | **3.5× faster** | **1.6× faster** |
| loop | 5.8 ms (5–6) | 221 ms (208–265) | 34 ms (32–55) | 2.3 ms (2–4) | 2.5 ms (2–3) | **37.9× faster** | **5.7× faster** |
| nested_loops | 4.0 ms (3–10) | 18 ms (17–22) | 22 ms (21–38) | 2.2 ms (2–3) | 2.5 ms (2–5) | **4.4× faster** | **5.5× faster** |
| sieve | 8.9 ms (8–10) | 26 ms (25–32) | 24 ms (23–44) | 2.3 ms (2–4) | 2.5 ms (2–3) | **3.0× faster** | **2.7× faster** |
| matrix | 5.9 ms (5–7) | 18 ms (17–34) | 23 ms (21–49) | 2.2 ms (2–6) | 2.5 ms (2–8) | **3.1× faster** | **3.9× faster** |
| mandelbrot | 4.0 ms (4–5) | 26 ms (24–32) | 23 ms (21–39) | 2.4 ms (2–6) | 2.7 ms (2–4) | **6.5× faster** | **5.7× faster** |
| lists | 13 ms (12–21) | 41 ms (40–67) | 29 ms (27–103) | 2.7 ms (2–6) | 3.3 ms (3–19) | **3.3× faster** | **2.2× faster** |
| records | 20 ms (18–36) | 44 ms (43–53) | 24 ms (22–27) | 2.3 ms (2–3) | 2.5 ms (2–3) | **2.2× faster** | **1.2× faster** |
| strings | 23 ms (22–24) | 39 ms (38–41) | 24 ms (23–30) | 10 ms (10–11) | 10 ms (10–15) | **1.7× faster** | **1.1× faster** |
| calls | 8.6 ms (8–18) | 31 ms (30–44) | 23 ms (22–27) | 2.2 ms (2–3) | 2.5 ms (2–4) | **3.6× faster** | **2.7× faster** |
| closures | 13 ms (13–17) | 34 ms (34–37) | 23 ms (21–38) | 2.5 ms (2–6) | 2.6 ms (2–3) | **2.6× faster** | **1.7× faster** |
| locals | 4.2 ms (4–5) | 38 ms (37–62) | 25 ms (24–29) | 2.3 ms (2–3) | 2.2 ms (2–3) | **9.1× faster** | **6.1× faster** |
| globals | 22 ms (21–40) | 74 ms (72–88) | 23 ms (22–29) | 2.6 ms (2–3) | 4.1 ms (4–4) | **3.4× faster** | **1.1× faster** |
| record_access | 24 ms (23–75) | 94 ms (90–120) | 23 ms (21–26) | 2.7 ms (2–4) | 2.8 ms (2–4) | **3.9× faster** | 1.1× slower |
| struct_access | 25 ms (23–29) | 85 ms (80–91) | 22 ms (21–44) | 2.5 ms (2–3) | 2.9 ms (2–4) | **3.4× faster** | 1.1× slower |
| parallel | 9.6 ms (9–10) | 159 ms (151–213) | 44 ms (42–66) | 2.8 ms (2–6) | 3.3 ms (3–4) | **16.6× faster** | **4.5× faster** |

### Peak memory

Highest resident memory of any sustained run (cold run for start-up).

| Benchmark | Khipu | Python | Node | C | Rust |
|---|---|---|---|---|---|
| fib | 5.8 MiB | 12.8 MiB | 52.1 MiB | 1.6 MiB | 1.8 MiB |
| fib_iterative | 5.8 MiB | 12.9 MiB | 53.3 MiB | 1.6 MiB | 1.8 MiB |
| loop | 5.8 MiB | 12.8 MiB | 54.4 MiB | 1.6 MiB | 1.8 MiB |
| nested_loops | 6.0 MiB | 12.8 MiB | 52.3 MiB | 1.6 MiB | 1.8 MiB |
| sieve | 9.0 MiB | 35.8 MiB | 80.0 MiB | 4.5 MiB | 4.7 MiB |
| matrix | 7.1 MiB | 13.4 MiB | 55.2 MiB | 2.0 MiB | 2.2 MiB |
| mandelbrot | 6.2 MiB | 12.9 MiB | 54.1 MiB | 1.6 MiB | 1.8 MiB |
| lists | 29.1 MiB | 129.8 MiB | 226.4 MiB | 26.6 MiB | 26.8 MiB |
| records | 7.2 MiB | 12.8 MiB | 55.0 MiB | 1.6 MiB | 1.8 MiB |
| strings | 4.3 MiB | 12.8 MiB | 60.5 MiB | 1.6 MiB | 1.9 MiB |
| calls | 4.3 MiB | 12.8 MiB | 54.1 MiB | 1.6 MiB | 1.8 MiB |
| closures | 4.3 MiB | 12.9 MiB | 52.5 MiB | 1.6 MiB | 1.8 MiB |
| locals | 5.8 MiB | 12.8 MiB | 54.3 MiB | 1.6 MiB | 1.8 MiB |
| globals | 4.3 MiB | 12.9 MiB | 52.5 MiB | 1.6 MiB | 1.8 MiB |
| record_access | 4.3 MiB | 12.8 MiB | 52.5 MiB | 1.6 MiB | 1.8 MiB |
| struct_access | 4.3 MiB | 12.9 MiB | 52.5 MiB | 1.6 MiB | 1.8 MiB |
| parallel | 13.6 MiB | 25.4 MiB | 129.4 MiB | 1.8 MiB | 1.9 MiB |
| empty | 4.3 MiB | 12.7 MiB | 48.3 MiB | 1.6 MiB | 1.8 MiB |

Raw samples for every run are in [`benchmarks/results/latest.json`](benchmarks/results/latest.json).
