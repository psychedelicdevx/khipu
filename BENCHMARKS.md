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
| date | 2026-11-26T14:32:42 |
| khipu | 5530285 (uncommitted changes) |
| cpu | Apple M5 |
| os | macOS-27.2-arm64-arm-64bit-Mach-O |
| python | Python 3.15.2 |
| node | v25.9.0 |
| c | Apple clang version 21.0.0 (clang-2100.3.34.2) |
| rust | rustc 1.95.0 (59807616e 2026-04-14) |

### Start-up

An empty program, median of 21 runs: Khipu 2.6 ms · Python 13 ms · Node 23 ms · C 2.3 ms · Rust 2.5 ms.

### Sustained

Median of 5 runs, with the fastest and slowest run in brackets.

| Benchmark | Khipu | Python | Node | C | Rust | vs Python | vs Node |
|---|---|---|---|---|---|---|---|
| fib | 161 ms (147–178) | 149 ms (136–179) | 35 ms (33–39) | 6.2 ms (6–10) | 6.2 ms (6–11) | 1.1× slower | 4.6× slower |
| fib_iterative | 613 ms (609–617) | 431 ms (427–450) | 44 ms (42–51) | 9.8 ms (9–10) | 10 ms (9–11) | 1.4× slower | 13.9× slower |
| loop | 356 ms (324–430) | 2206 ms (2085–2458) | 148 ms (147–149) | 2.5 ms (2–5) | 2.7 ms (2–3) | **6.2× faster** | 2.4× slower |
| nested_loops | 171 ms (169–173) | 227 ms (224–239) | 28 ms (26–40) | 4.2 ms (4–4) | 4.3 ms (4–5) | **1.3× faster** | 6.1× slower |
| sieve | 334 ms (322–350) | 549 ms (534–576) | 51 ms (51–60) | 5.0 ms (5–5) | 5.3 ms (5–6) | **1.6× faster** | 6.5× slower |
| matrix | 136 ms (135–150) | 259 ms (249–265) | 28 ms (27–37) | 3.3 ms (3–4) | 4.8 ms (4–5) | **1.9× faster** | 4.8× slower |
| mandelbrot | 79 ms (77–96) | 234 ms (221–237) | 27 ms (26–37) | 4.4 ms (4–5) | 4.3 ms (4–5) | **2.9× faster** | 2.9× slower |
| lists | 111 ms (110–118) | 283 ms (275–290) | 77 ms (72–90) | 5.2 ms (5–5) | 6.0 ms (5–11) | **2.5× faster** | 1.4× slower |
| records | 215 ms (209–232) | 340 ms (318–371) | 27 ms (26–32) | 2.4 ms (2–3) | 2.9 ms (2–5) | **1.6× faster** | 8.0× slower |
| strings | 231 ms (223–286) | 298 ms (285–402) | 44 ms (42–48) | 90 ms (84–119) | 110 ms (106–113) | **1.3× faster** | 5.3× slower |
| calls | 71 ms (69–88) | 198 ms (190–224) | 35 ms (34–36) | 2.6 ms (2–3) | 2.5 ms (2–3) | **2.8× faster** | 2.0× slower |
| closures | 130 ms (127–151) | 237 ms (235–240) | 29 ms (27–43) | 5.6 ms (5–6) | 4.2 ms (4–5) | **1.8× faster** | 4.5× slower |
| locals | 147 ms (125–159) | 279 ms (274–283) | 65 ms (64–84) | 2.7 ms (2–5) | 2.7 ms (2–3) | **1.9× faster** | 2.3× slower |
| globals | 94 ms (90–98) | 332 ms (321–355) | 27 ms (26–39) | 5.0 ms (5–5) | 11 ms (11–12) | **3.5× faster** | 3.5× slower |
| record_access | 275 ms (272–306) | 844 ms (807–893) | 27 ms (26–30) | 5.0 ms (5–5) | 6.4 ms (6–7) | **3.1× faster** | 10.3× slower |
| struct_access | 275 ms (263–285) | 739 ms (705–760) | 27 ms (25–32) | 5.3 ms (5–8) | 6.5 ms (6–7) | **2.7× faster** | 10.2× slower |
| parallel | 264 ms (239–277) | 421 ms (396–464) | 72 ms (68–73) | 21 ms (20–25) | 22 ms (21–25) | **1.6× faster** | 3.7× slower |

### Cold start

Small inputs where start-up time matters. Median of 21 runs.

| Benchmark | Khipu | Python | Node | C | Rust | vs Python | vs Node |
|---|---|---|---|---|---|---|---|
| fib | 16 ms (15–23) | 26 ms (24–57) | 25 ms (23–35) | 2.8 ms (2–6) | 3.1 ms (3–8) | **1.6× faster** | **1.5× faster** |
| fib_iterative | 64 ms (62–71) | 56 ms (54–63) | 26 ms (24–38) | 3.2 ms (3–17) | 3.5 ms (3–7) | 1.1× slower | 2.5× slower |
| loop | 37 ms (32–46) | 227 ms (211–278) | 36 ms (33–49) | 2.5 ms (2–3) | 2.6 ms (2–4) | **6.2× faster** | 1.0× slower |
| nested_loops | 6.4 ms (6–16) | 19 ms (17–29) | 26 ms (21–33) | 2.4 ms (2–4) | 2.7 ms (2–6) | **2.9× faster** | **4.0× faster** |
| sieve | 12 ms (11–17) | 27 ms (25–31) | 25 ms (24–36) | 2.5 ms (2–4) | 2.7 ms (2–4) | **2.2× faster** | **2.0× faster** |
| matrix | 5.6 ms (5–6) | 19 ms (18–33) | 23 ms (22–30) | 2.4 ms (2–4) | 2.6 ms (2–4) | **3.4× faster** | **4.1× faster** |
| mandelbrot | 7.1 ms (7–8) | 27 ms (25–31) | 24 ms (22–35) | 2.5 ms (2–5) | 2.7 ms (2–4) | **3.8× faster** | **3.4× faster** |
| lists | 14 ms (13–15) | 40 ms (38–59) | 27 ms (26–29) | 2.8 ms (2–6) | 3.1 ms (3–4) | **3.0× faster** | **2.0× faster** |
| records | 24 ms (23–41) | 45 ms (44–53) | 24 ms (23–26) | 2.2 ms (2–3) | 2.6 ms (2–3) | **1.9× faster** | **1.0× faster** |
| strings | 25 ms (24–27) | 42 ms (39–46) | 27 ms (24–33) | 11 ms (10–13) | 11 ms (10–16) | **1.7× faster** | **1.1× faster** |
| calls | 9.7 ms (9–19) | 34 ms (32–46) | 26 ms (23–59) | 2.6 ms (2–5) | 2.9 ms (2–7) | **3.5× faster** | **2.7× faster** |
| closures | 15 ms (14–19) | 36 ms (35–44) | 23 ms (23–42) | 2.6 ms (2–5) | 2.6 ms (2–4) | **2.4× faster** | **1.6× faster** |
| locals | 17 ms (15–19) | 40 ms (39–58) | 27 ms (25–31) | 2.4 ms (2–3) | 2.5 ms (2–3) | **2.4× faster** | **1.6× faster** |
| globals | 22 ms (20–35) | 78 ms (75–83) | 24 ms (23–30) | 3.0 ms (2–4) | 4.3 ms (4–5) | **3.6× faster** | **1.1× faster** |
| record_access | 31 ms (30–35) | 99 ms (95–117) | 24 ms (22–30) | 2.8 ms (2–4) | 3.0 ms (3–4) | **3.2× faster** | 1.3× slower |
| struct_access | 31 ms (28–32) | 88 ms (83–97) | 23 ms (23–40) | 2.8 ms (2–4) | 3.0 ms (2–5) | **2.9× faster** | 1.3× slower |
| parallel | 7.9 ms (7–9) | 158 ms (153–188) | 43 ms (41–62) | 2.9 ms (2–4) | 3.2 ms (3–4) | **19.9× faster** | **5.4× faster** |

### Peak memory

Highest resident memory of any sustained run (cold run for start-up).

| Benchmark | Khipu | Python | Node | C | Rust |
|---|---|---|---|---|---|
| fib | 2.2 MiB | 12.8 MiB | 52.1 MiB | 1.6 MiB | 1.8 MiB |
| fib_iterative | 2.3 MiB | 12.9 MiB | 53.2 MiB | 1.6 MiB | 1.8 MiB |
| loop | 2.2 MiB | 12.8 MiB | 54.3 MiB | 1.6 MiB | 1.8 MiB |
| nested_loops | 2.4 MiB | 12.8 MiB | 52.3 MiB | 1.6 MiB | 1.8 MiB |
| sieve | 141.8 MiB | 35.8 MiB | 79.9 MiB | 4.5 MiB | 4.7 MiB |
| matrix | 3.7 MiB | 13.4 MiB | 55.1 MiB | 2.0 MiB | 2.2 MiB |
| mandelbrot | 2.4 MiB | 12.9 MiB | 54.3 MiB | 1.6 MiB | 1.8 MiB |
| lists | 141.7 MiB | 129.8 MiB | 226.4 MiB | 26.6 MiB | 26.8 MiB |
| records | 63.4 MiB | 12.8 MiB | 55.0 MiB | 1.6 MiB | 1.8 MiB |
| strings | 63.4 MiB | 12.9 MiB | 60.5 MiB | 1.6 MiB | 1.9 MiB |
| calls | 93.8 MiB | 12.8 MiB | 54.1 MiB | 1.6 MiB | 1.8 MiB |
| closures | 154.9 MiB | 12.9 MiB | 52.5 MiB | 1.6 MiB | 1.8 MiB |
| locals | 2.2 MiB | 12.8 MiB | 54.1 MiB | 1.6 MiB | 1.8 MiB |
| globals | 2.2 MiB | 12.9 MiB | 52.4 MiB | 1.6 MiB | 1.8 MiB |
| record_access | 2.2 MiB | 12.8 MiB | 52.4 MiB | 1.6 MiB | 1.8 MiB |
| struct_access | 2.2 MiB | 12.9 MiB | 52.6 MiB | 1.6 MiB | 1.8 MiB |
| parallel | 2.6 MiB | 25.4 MiB | 130.0 MiB | 1.8 MiB | 1.9 MiB |
| empty | 2.2 MiB | 12.7 MiB | 48.4 MiB | 1.6 MiB | 1.8 MiB |

Raw samples for every run are in [`benchmarks/results/latest.json`](benchmarks/results/latest.json).
