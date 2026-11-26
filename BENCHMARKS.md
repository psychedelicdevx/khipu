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
| date | 2026-11-26T19:44:29 |
| khipu | b2e8325 |
| cpu | Apple M5 |
| os | macOS-27.2-arm64-arm-64bit-Mach-O |
| python | Python 3.15.2 |
| node | v25.9.0 |
| c | Apple clang version 21.0.0 (clang-2100.3.34.2) |
| rust | rustc 1.95.0 (59807616e 2026-04-14) |

### Start-up

An empty program, median of 21 runs: Khipu 3.8 ms · Python 14 ms · Node 23 ms · C 2.4 ms · Rust 2.7 ms.

### Sustained

Median of 5 runs, with the fastest and slowest run in brackets.

| Benchmark | Khipu | Python | Node | C | Rust | vs Python | vs Node |
|---|---|---|---|---|---|---|---|
| fib | 21 ms (21–21) | 136 ms (134–147) | 33 ms (33–35) | 5.9 ms (6–6) | 6.2 ms (6–10) | **6.5× faster** | **1.6× faster** |
| fib_iterative | 119 ms (116–137) | 421 ms (417–428) | 42 ms (42–43) | 9.5 ms (9–10) | 9.7 ms (9–10) | **3.6× faster** | 2.8× slower |
| loop | 25 ms (25–38) | 2095 ms (2034–2335) | 149 ms (146–154) | 2.6 ms (2–5) | 2.9 ms (2–3) | **83.3× faster** | **5.9× faster** |
| nested_loops | 9.4 ms (9–11) | 230 ms (221–238) | 26 ms (25–38) | 4.6 ms (4–5) | 4.3 ms (4–5) | **24.5× faster** | **2.8× faster** |
| sieve | 161 ms (155–168) | 561 ms (526–587) | 53 ms (50–54) | 5.0 ms (5–5) | 5.5 ms (5–6) | **3.5× faster** | 3.0× slower |
| matrix | 112 ms (106–130) | 272 ms (243–348) | 29 ms (27–33) | 3.5 ms (3–4) | 4.7 ms (4–6) | **2.4× faster** | 3.9× slower |
| mandelbrot | 6.2 ms (6–10) | 214 ms (211–291) | 25 ms (23–33) | 3.9 ms (4–7) | 4.2 ms (4–18) | **34.7× faster** | **4.0× faster** |
| lists | 76 ms (76–78) | 274 ms (264–300) | 72 ms (71–90) | 5.2 ms (5–5) | 6.1 ms (5–10) | **3.6× faster** | 1.1× slower |
| records | 197 ms (191–223) | 320 ms (313–324) | 26 ms (25–27) | 2.2 ms (2–2) | 2.5 ms (2–5) | **1.6× faster** | 7.6× slower |
| strings | 194 ms (188–196) | 282 ms (274–294) | 41 ms (40–44) | 83 ms (82–83) | 105 ms (104–108) | **1.5× faster** | 4.7× slower |
| calls | 52 ms (50–139) | 195 ms (185–324) | 37 ms (34–71) | 2.5 ms (2–3) | 2.3 ms (2–3) | **3.7× faster** | 1.4× slower |
| closures | 112 ms (106–141) | 262 ms (227–285) | 28 ms (25–37) | 5.0 ms (4–7) | 5.2 ms (4–7) | **2.3× faster** | 4.0× slower |
| locals | 15 ms (13–27) | 484 ms (342–868) | 82 ms (77–112) | 3.6 ms (2–5) | 3.1 ms (3–5) | **32.8× faster** | **5.6× faster** |
| globals | 149 ms (134–171) | 503 ms (479–544) | 37 ms (34–51) | 6.2 ms (6–8) | 16 ms (14–21) | **3.4× faster** | 4.1× slower |
| record_access | 365 ms (325–769) | 1394 ms (1028–2041) | 35 ms (24–65) | 8.6 ms (4–14) | 9.3 ms (6–16) | **3.8× faster** | 10.4× slower |
| struct_access | 290 ms (227–632) | 826 ms (781–1037) | 37 ms (29–55) | 6.4 ms (5–10) | 7.6 ms (7–9) | **2.8× faster** | 7.9× slower |
| parallel | 209 ms (200–292) | 428 ms (418–490) | 70 ms (66–76) | 24 ms (21–26) | 22 ms (22–24) | **2.1× faster** | 3.0× slower |

### Cold start

Small inputs where start-up time matters. Median of 21 runs.

| Benchmark | Khipu | Python | Node | C | Rust | vs Python | vs Node |
|---|---|---|---|---|---|---|---|
| fib | 5.4 ms (5–7) | 25 ms (24–57) | 24 ms (23–46) | 2.9 ms (2–4) | 3.0 ms (3–4) | **4.7× faster** | **4.5× faster** |
| fib_iterative | 15 ms (15–18) | 55 ms (54–59) | 25 ms (24–36) | 3.1 ms (3–4) | 3.4 ms (3–6) | **3.6× faster** | **1.6× faster** |
| loop | 6.1 ms (5–8) | 227 ms (214–264) | 34 ms (32–41) | 2.3 ms (2–3) | 2.6 ms (2–3) | **37.3× faster** | **5.6× faster** |
| nested_loops | 4.2 ms (3–9) | 19 ms (18–23) | 23 ms (22–35) | 2.4 ms (2–3) | 2.9 ms (2–5) | **4.4× faster** | **5.6× faster** |
| sieve | 9.0 ms (8–10) | 27 ms (25–40) | 25 ms (24–45) | 2.4 ms (2–4) | 2.7 ms (2–3) | **3.0× faster** | **2.8× faster** |
| matrix | 6.0 ms (6–7) | 18 ms (18–27) | 23 ms (22–30) | 2.3 ms (2–3) | 2.5 ms (2–3) | **3.1× faster** | **3.8× faster** |
| mandelbrot | 3.9 ms (3–5) | 26 ms (24–31) | 23 ms (21–37) | 2.4 ms (2–5) | 2.5 ms (2–4) | **6.7× faster** | **5.8× faster** |
| lists | 11 ms (10–12) | 39 ms (38–59) | 27 ms (26–36) | 2.9 ms (2–6) | 3.1 ms (3–4) | **3.5× faster** | **2.4× faster** |
| records | 23 ms (22–39) | 44 ms (42–53) | 24 ms (23–25) | 2.3 ms (2–3) | 2.6 ms (2–3) | **1.9× faster** | **1.1× faster** |
| strings | 23 ms (22–33) | 41 ms (39–48) | 27 ms (23–46) | 11 ms (10–12) | 11 ms (10–18) | **1.8× faster** | **1.2× faster** |
| calls | 8.8 ms (8–32) | 33 ms (30–262) | 26 ms (22–264) | 2.4 ms (2–11) | 2.9 ms (2–124) | **3.7× faster** | **2.9× faster** |
| closures | 13 ms (12–15) | 33 ms (32–44) | 21 ms (19–29) | 2.1 ms (2–5) | 2.2 ms (2–4) | **2.6× faster** | **1.7× faster** |
| locals | 4.9 ms (4–7) | 45 ms (41–58) | 29 ms (26–36) | 2.4 ms (2–4) | 2.5 ms (2–3) | **9.2× faster** | **6.0× faster** |
| globals | 31 ms (25–43) | 117 ms (96–148) | 32 ms (27–46) | 3.6 ms (3–5) | 5.9 ms (5–8) | **3.8× faster** | **1.0× faster** |
| record_access | 26 ms (25–31) | 90 ms (87–192) | 21 ms (20–69) | 2.4 ms (2–4) | 2.6 ms (2–4) | **3.5× faster** | 1.3× slower |
| struct_access | 33 ms (25–49) | 95 ms (82–134) | 26 ms (20–51) | 2.7 ms (2–3) | 3.1 ms (2–6) | **2.9× faster** | 1.3× slower |
| parallel | 11 ms (8–18) | 193 ms (146–2484) | 50 ms (40–93) | 3.0 ms (2–8) | 3.3 ms (3–12) | **18.0× faster** | **4.7× faster** |

### Peak memory

Highest resident memory of any sustained run (cold run for start-up).

| Benchmark | Khipu | Python | Node | C | Rust |
|---|---|---|---|---|---|
| fib | 5.8 MiB | 12.8 MiB | 52.0 MiB | 1.6 MiB | 1.8 MiB |
| fib_iterative | 5.7 MiB | 12.9 MiB | 53.1 MiB | 1.6 MiB | 1.8 MiB |
| loop | 5.7 MiB | 12.8 MiB | 54.1 MiB | 1.6 MiB | 1.8 MiB |
| nested_loops | 5.9 MiB | 12.9 MiB | 52.4 MiB | 1.6 MiB | 1.8 MiB |
| sieve | 9.0 MiB | 35.8 MiB | 80.0 MiB | 4.5 MiB | 4.7 MiB |
| matrix | 7.0 MiB | 13.4 MiB | 55.2 MiB | 2.0 MiB | 2.2 MiB |
| mandelbrot | 6.1 MiB | 12.9 MiB | 54.0 MiB | 1.6 MiB | 1.8 MiB |
| lists | 29.0 MiB | 129.8 MiB | 226.4 MiB | 26.6 MiB | 26.8 MiB |
| records | 6.4 MiB | 12.8 MiB | 55.0 MiB | 1.6 MiB | 1.8 MiB |
| strings | 4.3 MiB | 12.8 MiB | 60.5 MiB | 1.6 MiB | 1.9 MiB |
| calls | 4.3 MiB | 12.8 MiB | 54.2 MiB | 1.6 MiB | 1.8 MiB |
| closures | 4.3 MiB | 12.9 MiB | 52.5 MiB | 1.6 MiB | 1.8 MiB |
| locals | 5.7 MiB | 12.9 MiB | 54.0 MiB | 1.6 MiB | 1.8 MiB |
| globals | 4.2 MiB | 12.9 MiB | 52.5 MiB | 1.6 MiB | 1.8 MiB |
| record_access | 4.2 MiB | 12.8 MiB | 52.4 MiB | 1.6 MiB | 1.8 MiB |
| struct_access | 4.3 MiB | 12.8 MiB | 52.6 MiB | 1.6 MiB | 1.8 MiB |
| parallel | 13.1 MiB | 25.3 MiB | 130.2 MiB | 1.8 MiB | 1.9 MiB |
| empty | 4.2 MiB | 12.8 MiB | 48.4 MiB | 1.6 MiB | 1.8 MiB |

Raw samples for every run are in [`benchmarks/results/latest.json`](benchmarks/results/latest.json).
