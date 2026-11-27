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
| date | 2026-11-27T01:01:05 |
| khipu | 62be53b (uncommitted changes) |
| cpu | Apple M5 |
| os | macOS-27.2-arm64-arm-64bit-Mach-O |
| python | Python 3.15.2 |
| node | v25.9.0 |
| c | Apple clang version 21.0.0 (clang-2100.3.34.2) |
| rust | rustc 1.100.0 (0ef1bc386 2026-11-09) |

### Start-up

An empty program, median of 21 runs: Khipu 4.8 ms · Python 16 ms · Node 28 ms · C 2.7 ms · Rust 3.1 ms.

### Sustained

Median of 5 runs, with the fastest and slowest run in brackets.

| Benchmark | Khipu | Python | Node | C | Rust | vs Python | vs Node |
|---|---|---|---|---|---|---|---|
| fib | 21 ms (21–22) | 138 ms (135–146) | 35 ms (34–37) | 6.2 ms (6–6) | 6.2 ms (6–10) | **6.6× faster** | **1.7× faster** |
| fib_iterative | 78 ms (78–95) | 431 ms (429–472) | 44 ms (44–61) | 9.7 ms (10–10) | 11 ms (11–18) | **5.5× faster** | 1.8× slower |
| loop | 25 ms (24–35) | 2168 ms (2051–2235) | 147 ms (144–155) | 2.3 ms (2–4) | 2.7 ms (2–3) | **86.9× faster** | **5.9× faster** |
| nested_loops | 8.9 ms (9–9) | 227 ms (223–236) | 26 ms (25–47) | 4.3 ms (4–5) | 4.4 ms (4–5) | **25.5× faster** | **2.9× faster** |
| sieve | 178 ms (171–181) | 535 ms (496–578) | 52 ms (51–56) | 4.9 ms (4–5) | 5.2 ms (5–5) | **3.0× faster** | 3.4× slower |
| matrix | 114 ms (112–127) | 250 ms (242–253) | 28 ms (27–29) | 3.0 ms (3–3) | 4.4 ms (4–5) | **2.2× faster** | 4.1× slower |
| mandelbrot | 7.0 ms (6–11) | 250 ms (229–271) | 28 ms (27–34) | 3.9 ms (4–7) | 4.5 ms (4–9) | **35.9× faster** | **4.0× faster** |
| lists | 88 ms (86–93) | 290 ms (279–476) | 80 ms (73–96) | 5.5 ms (5–6) | 6.3 ms (6–11) | **3.3× faster** | 1.1× slower |
| records | 174 ms (169–180) | 366 ms (342–426) | 31 ms (29–32) | 2.3 ms (2–3) | 2.8 ms (2–4) | **2.1× faster** | 5.7× slower |
| strings | 201 ms (201–256) | 290 ms (284–305) | 44 ms (43–95) | 88 ms (83–115) | 106 ms (105–108) | **1.4× faster** | 4.6× slower |
| calls | 61 ms (59–75) | 203 ms (199–226) | 36 ms (34–41) | 2.7 ms (2–3) | 2.6 ms (2–3) | **3.3× faster** | 1.7× slower |
| closures | 122 ms (118–144) | 246 ms (241–278) | 31 ms (27–36) | 5.2 ms (5–8) | 4.4 ms (4–6) | **2.0× faster** | 4.0× slower |
| locals | 11 ms (11–12) | 281 ms (277–283) | 66 ms (64–75) | 2.5 ms (2–5) | 2.9 ms (2–4) | **24.9× faster** | **5.9× faster** |
| globals | 100 ms (97–115) | 357 ms (342–364) | 29 ms (29–34) | 5.3 ms (5–5) | 12 ms (11–12) | **3.6× faster** | 3.4× slower |
| record_access | 255 ms (248–414) | 1009 ms (945–1353) | 30 ms (29–36) | 5.7 ms (5–43) | 12 ms (11–17) | **4.0× faster** | 8.5× slower |
| struct_access | 12 ms (11–24) | 826 ms (750–955) | 39 ms (28–49) | 6.4 ms (5–18) | 14 ms (11–34) | **70.3× faster** | **3.3× faster** |
| parallel | 257 ms (239–280) | 416 ms (381–503) | 76 ms (69–112) | 22 ms (21–25) | 22 ms (21–44) | **1.6× faster** | 3.4× slower |

### Cold start

Small inputs where start-up time matters. Median of 21 runs.

| Benchmark | Khipu | Python | Node | C | Rust | vs Python | vs Node |
|---|---|---|---|---|---|---|---|
| fib | 5.6 ms (5–7) | 26 ms (25–43) | 24 ms (23–36) | 2.8 ms (2–4) | 3.0 ms (3–4) | **4.6× faster** | **4.4× faster** |
| fib_iterative | 11 ms (11–15) | 56 ms (55–75) | 26 ms (24–39) | 3.3 ms (3–4) | 3.5 ms (3–7) | **4.9× faster** | **2.3× faster** |
| loop | 6.2 ms (6–8) | 231 ms (213–279) | 36 ms (34–48) | 2.4 ms (2–3) | 2.6 ms (2–3) | **37.3× faster** | **5.8× faster** |
| nested_loops | 4.1 ms (4–10) | 18 ms (17–22) | 23 ms (21–36) | 2.3 ms (2–3) | 2.6 ms (2–5) | **4.5× faster** | **5.6× faster** |
| sieve | 9.2 ms (8–12) | 27 ms (25–65) | 25 ms (23–49) | 2.4 ms (2–3) | 2.6 ms (2–10) | **3.0× faster** | **2.7× faster** |
| matrix | 6.1 ms (6–7) | 19 ms (18–30) | 24 ms (22–26) | 2.3 ms (2–4) | 2.7 ms (2–4) | **3.1× faster** | **3.9× faster** |
| mandelbrot | 4.4 ms (4–5) | 29 ms (24–45) | 27 ms (23–71) | 2.5 ms (2–5) | 3.0 ms (2–5) | **6.6× faster** | **6.0× faster** |
| lists | 12 ms (12–28) | 42 ms (40–115) | 29 ms (27–46) | 3.1 ms (3–6) | 3.3 ms (3–5) | **3.4× faster** | **2.4× faster** |
| records | 21 ms (20–41) | 46 ms (44–51) | 26 ms (23–36) | 2.4 ms (2–3) | 2.6 ms (2–3) | **2.2× faster** | **1.2× faster** |
| strings | 24 ms (23–45) | 43 ms (40–63) | 28 ms (26–73) | 11 ms (10–22) | 11 ms (11–14) | **1.8× faster** | **1.1× faster** |
| calls | 9.9 ms (9–20) | 33 ms (32–39) | 25 ms (24–30) | 2.5 ms (2–3) | 2.7 ms (2–3) | **3.4× faster** | **2.6× faster** |
| closures | 15 ms (15–20) | 37 ms (35–88) | 25 ms (24–35) | 2.8 ms (2–20) | 2.8 ms (2–5) | **2.4× faster** | **1.6× faster** |
| locals | 5.1 ms (4–6) | 42 ms (40–53) | 28 ms (26–32) | 2.5 ms (2–4) | 2.6 ms (2–3) | **8.3× faster** | **5.6× faster** |
| globals | 27 ms (22–50) | 91 ms (80–245) | 28 ms (26–60) | 3.3 ms (3–16) | 4.7 ms (4–12) | **3.4× faster** | **1.1× faster** |
| record_access | 29 ms (28–52) | 105 ms (98–183) | 26 ms (25–43) | 2.9 ms (3–3) | 3.7 ms (3–4) | **3.6× faster** | 1.1× slower |
| struct_access | 4.9 ms (4–10) | 92 ms (86–114) | 25 ms (24–34) | 2.7 ms (2–4) | 3.6 ms (3–8) | **18.9× faster** | **5.2× faster** |
| parallel | 11 ms (10–14) | 145 ms (126–313) | 50 ms (46–224) | 3.3 ms (3–29) | 3.5 ms (3–5) | **13.0× faster** | **4.5× faster** |

### Peak memory

Highest resident memory of any sustained run (cold run for start-up).

| Benchmark | Khipu | Python | Node | C | Rust |
|---|---|---|---|---|---|
| fib | 5.8 MiB | 12.8 MiB | 52.0 MiB | 1.6 MiB | 1.8 MiB |
| fib_iterative | 5.7 MiB | 12.9 MiB | 53.1 MiB | 1.6 MiB | 1.8 MiB |
| loop | 5.6 MiB | 12.8 MiB | 54.4 MiB | 1.6 MiB | 1.8 MiB |
| nested_loops | 5.9 MiB | 12.8 MiB | 52.4 MiB | 1.6 MiB | 1.8 MiB |
| sieve | 9.0 MiB | 35.8 MiB | 80.0 MiB | 4.5 MiB | 4.7 MiB |
| matrix | 7.1 MiB | 13.4 MiB | 55.2 MiB | 2.0 MiB | 2.2 MiB |
| mandelbrot | 6.2 MiB | 12.9 MiB | 54.7 MiB | 1.6 MiB | 1.8 MiB |
| lists | 29.0 MiB | 129.8 MiB | 226.4 MiB | 26.6 MiB | 26.8 MiB |
| records | 7.2 MiB | 12.9 MiB | 54.9 MiB | 1.6 MiB | 1.8 MiB |
| strings | 4.3 MiB | 12.8 MiB | 60.6 MiB | 1.6 MiB | 1.9 MiB |
| calls | 5.6 MiB | 12.9 MiB | 54.1 MiB | 1.6 MiB | 1.8 MiB |
| closures | 4.2 MiB | 12.9 MiB | 52.6 MiB | 1.6 MiB | 1.8 MiB |
| locals | 5.9 MiB | 12.9 MiB | 54.6 MiB | 1.6 MiB | 1.8 MiB |
| globals | 4.3 MiB | 12.8 MiB | 52.5 MiB | 1.6 MiB | 1.8 MiB |
| record_access | 4.2 MiB | 12.8 MiB | 52.5 MiB | 1.6 MiB | 1.8 MiB |
| struct_access | 5.7 MiB | 12.9 MiB | 52.7 MiB | 1.6 MiB | 1.8 MiB |
| parallel | 14.0 MiB | 22.4 MiB | 129.7 MiB | 1.8 MiB | 1.9 MiB |
| empty | 4.2 MiB | 12.8 MiB | 48.5 MiB | 1.6 MiB | 1.8 MiB |

Raw samples for every run are in [`benchmarks/results/latest.json`](benchmarks/results/latest.json).
