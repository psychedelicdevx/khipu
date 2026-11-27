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
| date | 2026-11-27T19:08:14 |
| khipu | 0611aad |
| cpu | Apple M5 |
| os | macOS-27.2-arm64-arm-64bit-Mach-O |
| python | Python 3.15.2 |
| node | v25.9.0 |
| c | Apple clang version 21.0.0 (clang-2100.3.34.2) |
| rust | rustc 1.100.0 (0ef1bc386 2026-11-09) |

### Start-up

An empty program, median of 21 runs: Khipu 3.2 ms · Python 12 ms · Node 20 ms · C 2.0 ms · Rust 2.1 ms.

### Sustained

Median of 5 runs, with the fastest and slowest run in brackets.

| Benchmark | Khipu | Python | Node | C | Rust | vs Python | vs Node |
|---|---|---|---|---|---|---|---|
| fib | 20 ms (20–20) | 132 ms (132–150) | 32 ms (31–33) | 5.8 ms (5–8) | 5.6 ms (5–11) | **6.7× faster** | **1.6× faster** |
| fib_iterative | 79 ms (78–99) | 414 ms (410–420) | 41 ms (41–42) | 9.2 ms (9–10) | 11 ms (10–11) | **5.2× faster** | 1.9× slower |
| loop | 25 ms (24–27) | 2141 ms (2025–2581) | 141 ms (139–145) | 2.2 ms (2–3) | 2.6 ms (2–3) | **86.1× faster** | **5.7× faster** |
| nested_loops | 8.6 ms (8–11) | 224 ms (221–231) | 24 ms (23–48) | 3.9 ms (4–4) | 4.0 ms (4–4) | **26.0× faster** | **2.8× faster** |
| sieve | 177 ms (174–177) | 456 ms (444–500) | 49 ms (47–49) | 4.8 ms (5–5) | 5.0 ms (4–5) | **2.6× faster** | 3.6× slower |
| matrix | 111 ms (109–124) | 257 ms (241–263) | 27 ms (26–29) | 2.9 ms (3–3) | 4.7 ms (4–5) | **2.3× faster** | 4.1× slower |
| mandelbrot | 6.1 ms (6–10) | 241 ms (222–293) | 30 ms (23–41) | 3.7 ms (3–5) | 3.9 ms (3–7) | **39.4× faster** | **4.9× faster** |
| lists | 78 ms (77–79) | 259 ms (251–265) | 73 ms (70–90) | 4.7 ms (5–5) | 5.5 ms (5–14) | **3.3× faster** | 1.1× slower |
| list_index | 13 ms (13–27) | 75 ms (74–85) | 24 ms (23–24) | 2.6 ms (3–3) | 2.6 ms (2–6) | **5.6× faster** | **1.8× faster** |
| records | 161 ms (156–164) | 315 ms (312–338) | 25 ms (24–28) | 2.3 ms (2–2) | 2.1 ms (2–3) | **2.0× faster** | 6.5× slower |
| strings | 188 ms (186–189) | 270 ms (267–276) | 43 ms (40–45) | 83 ms (80–86) | 105 ms (102–108) | **1.4× faster** | 4.3× slower |
| calls | 78 ms (78–79) | 185 ms (181–192) | 33 ms (32–59) | 2.0 ms (2–2) | 2.4 ms (2–3) | **2.4× faster** | 2.4× slower |
| closures | 112 ms (112–114) | 225 ms (223–236) | 26 ms (24–41) | 4.6 ms (4–13) | 3.8 ms (4–4) | **2.0× faster** | 4.3× slower |
| locals | 11 ms (10–11) | 260 ms (257–289) | 59 ms (59–64) | 1.9 ms (2–3) | 2.4 ms (2–3) | **24.5× faster** | **5.6× faster** |
| globals | 100 ms (97–113) | 319 ms (314–323) | 26 ms (25–27) | 4.3 ms (4–5) | 10 ms (10–11) | **3.2× faster** | 3.9× slower |
| record_access | 224 ms (215–236) | 812 ms (785–831) | 26 ms (24–28) | 4.8 ms (5–5) | 11 ms (10–11) | **3.6× faster** | 8.7× slower |
| struct_access | 10 ms (10–17) | 184 ms (183–192) | 24 ms (23–24) | 4.6 ms (4–10) | 10 ms (10–11) | **18.2× faster** | **2.4× faster** |
| parallel | 218 ms (210–230) | 326 ms (317–336) | 63 ms (60–64) | 20 ms (20–20) | 20 ms (19–24) | **1.5× faster** | 3.5× slower |

### Cold start

Small inputs where start-up time matters. Median of 21 runs.

| Benchmark | Khipu | Python | Node | C | Rust | vs Python | vs Node |
|---|---|---|---|---|---|---|---|
| fib | 5.2 ms (5–7) | 23 ms (23–56) | 22 ms (21–46) | 2.5 ms (2–4) | 2.7 ms (2–3) | **4.5× faster** | **4.3× faster** |
| fib_iterative | 11 ms (10–13) | 53 ms (51–56) | 23 ms (22–45) | 2.9 ms (3–3) | 3.1 ms (3–6) | **4.7× faster** | **2.1× faster** |
| loop | 6.1 ms (5–10) | 225 ms (208–287) | 34 ms (31–44) | 2.3 ms (2–5) | 2.5 ms (2–5) | **37.1× faster** | **5.6× faster** |
| nested_loops | 3.7 ms (3–9) | 17 ms (16–19) | 21 ms (20–39) | 2.0 ms (2–3) | 2.3 ms (2–5) | **4.5× faster** | **5.7× faster** |
| sieve | 8.7 ms (8–9) | 25 ms (24–29) | 23 ms (22–47) | 2.1 ms (2–3) | 2.3 ms (2–3) | **2.8× faster** | **2.6× faster** |
| matrix | 5.5 ms (5–6) | 17 ms (16–35) | 21 ms (20–27) | 2.0 ms (2–3) | 2.1 ms (2–3) | **3.0× faster** | **3.8× faster** |
| mandelbrot | 3.8 ms (3–5) | 24 ms (23–28) | 21 ms (20–43) | 2.1 ms (2–5) | 2.4 ms (2–3) | **6.4× faster** | **5.5× faster** |
| lists | 11 ms (10–11) | 37 ms (36–60) | 25 ms (24–26) | 2.4 ms (2–7) | 2.8 ms (2–3) | **3.5× faster** | **2.3× faster** |
| list_index | 4.8 ms (4–11) | 18 ms (18–33) | 21 ms (20–26) | 2.0 ms (2–4) | 2.2 ms (2–4) | **3.8× faster** | **4.3× faster** |
| records | 19 ms (18–22) | 43 ms (42–54) | 22 ms (21–42) | 2.0 ms (2–3) | 2.2 ms (2–5) | **2.3× faster** | **1.2× faster** |
| strings | 23 ms (21–34) | 38 ms (37–42) | 25 ms (23–33) | 10 ms (10–12) | 10 ms (10–12) | **1.7× faster** | **1.1× faster** |
| calls | 11 ms (10–14) | 29 ms (28–31) | 22 ms (21–44) | 2.0 ms (2–6) | 2.2 ms (2–2) | **2.7× faster** | **2.0× faster** |
| closures | 14 ms (14–15) | 34 ms (32–59) | 22 ms (20–24) | 2.3 ms (2–3) | 2.3 ms (2–3) | **2.4× faster** | **1.5× faster** |
| locals | 4.3 ms (4–10) | 37 ms (36–43) | 24 ms (23–47) | 2.0 ms (2–3) | 2.2 ms (2–3) | **8.6× faster** | **5.7× faster** |
| globals | 22 ms (20–24) | 74 ms (71–106) | 23 ms (21–26) | 2.6 ms (2–5) | 3.8 ms (4–4) | **3.3× faster** | **1.0× faster** |
| record_access | 26 ms (24–27) | 93 ms (89–98) | 22 ms (20–40) | 2.3 ms (2–3) | 3.1 ms (3–5) | **3.6× faster** | 1.2× slower |
| struct_access | 4.3 ms (4–6) | 31 ms (29–67) | 22 ms (21–38) | 2.3 ms (2–5) | 3.1 ms (3–4) | **7.1× faster** | **5.0× faster** |
| parallel | 9.4 ms (9–18) | 105 ms (102–158) | 40 ms (39–52) | 2.8 ms (2–3) | 3.1 ms (2–4) | **11.1× faster** | **4.2× faster** |

### Peak memory

Highest resident memory of any sustained run (cold run for start-up).

| Benchmark | Khipu | Python | Node | C | Rust |
|---|---|---|---|---|---|
| fib | 6.0 MiB | 12.8 MiB | 52.0 MiB | 1.6 MiB | 1.8 MiB |
| fib_iterative | 6.0 MiB | 12.8 MiB | 53.1 MiB | 1.6 MiB | 1.8 MiB |
| loop | 6.0 MiB | 12.8 MiB | 54.5 MiB | 1.6 MiB | 1.8 MiB |
| nested_loops | 6.3 MiB | 12.8 MiB | 52.3 MiB | 1.6 MiB | 1.8 MiB |
| sieve | 9.3 MiB | 35.8 MiB | 79.9 MiB | 4.5 MiB | 4.7 MiB |
| matrix | 7.3 MiB | 13.4 MiB | 55.3 MiB | 2.0 MiB | 2.2 MiB |
| mandelbrot | 6.3 MiB | 12.9 MiB | 55.1 MiB | 1.6 MiB | 1.8 MiB |
| lists | 29.3 MiB | 129.7 MiB | 226.3 MiB | 26.6 MiB | 26.8 MiB |
| list_index | 6.3 MiB | 12.9 MiB | 52.4 MiB | 1.6 MiB | 1.8 MiB |
| records | 7.0 MiB | 12.8 MiB | 54.9 MiB | 1.6 MiB | 1.8 MiB |
| strings | 4.7 MiB | 12.8 MiB | 60.5 MiB | 1.6 MiB | 1.9 MiB |
| calls | 6.0 MiB | 12.8 MiB | 54.1 MiB | 1.6 MiB | 1.8 MiB |
| closures | 4.6 MiB | 12.9 MiB | 52.6 MiB | 1.6 MiB | 1.8 MiB |
| locals | 6.2 MiB | 12.8 MiB | 53.9 MiB | 1.6 MiB | 1.8 MiB |
| globals | 4.6 MiB | 12.8 MiB | 52.5 MiB | 1.6 MiB | 1.8 MiB |
| record_access | 4.7 MiB | 12.9 MiB | 52.5 MiB | 1.6 MiB | 1.8 MiB |
| struct_access | 6.1 MiB | 12.9 MiB | 52.5 MiB | 1.6 MiB | 1.8 MiB |
| parallel | 15.7 MiB | 22.4 MiB | 129.2 MiB | 1.8 MiB | 2.0 MiB |
| empty | 4.6 MiB | 12.8 MiB | 48.3 MiB | 1.6 MiB | 1.8 MiB |

Raw samples for every run are in [`benchmarks/results/latest.json`](../benchmarks/results/latest.json).
## HTTP

Khipu, Node (built-in `http`), Go (`net/http`) and Rust (`std` only, one thread per connection),
same three routes (`/hello`, `/json`, `/file`), same box, same `wrk` command:
`wrk -t4 -c100 -d30s URL/ROUTE`. Requests/sec. Khipu, Node and Go keep connections alive; Rust-std closes each one.
Generated by `scripts/bench-http.sh`, not edited by hand.

| server | /hello | /json | /file |
|---|---|---|---|
| khipu | 133514.35 | 136417.05 | 91218.03 |
| node | 124161.84 | 121900.40 | 124314.34 |
| go | 129302.28 | 126368.92 | 129382.86 |
| rust-std | 30240.75 | 29718.17 | 29109.29 |
