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
| date | 2026-11-27T12:58:39 |
| khipu | 1c5b235 |
| cpu | Apple M5 |
| os | macOS-27.2-arm64-arm-64bit-Mach-O |
| python | Python 3.15.2 |
| node | v25.9.0 |
| c | Apple clang version 21.0.0 (clang-2100.3.34.2) |
| rust | rustc 1.100.0 (0ef1bc386 2026-11-09) |

### Start-up

An empty program, median of 21 runs: Khipu 3.3 ms · Python 13 ms · Node 21 ms · C 2.1 ms · Rust 2.3 ms.

### Sustained

Median of 5 runs, with the fastest and slowest run in brackets.

| Benchmark | Khipu | Python | Node | C | Rust | vs Python | vs Node |
|---|---|---|---|---|---|---|---|
| fib | 20 ms (19–20) | 132 ms (131–147) | 33 ms (32–38) | 5.4 ms (5–6) | 5.6 ms (6–11) | **6.7× faster** | **1.7× faster** |
| fib_iterative | 79 ms (77–97) | 413 ms (411–417) | 41 ms (40–42) | 9.1 ms (9–9) | 10.0 ms (10–10) | **5.2× faster** | 1.9× slower |
| loop | 24 ms (23–40) | 2110 ms (2055–2209) | 140 ms (137–158) | 2.3 ms (2–5) | 2.6 ms (2–3) | **89.1× faster** | **5.9× faster** |
| nested_loops | 8.9 ms (8–12) | 220 ms (218–223) | 25 ms (23–47) | 4.2 ms (4–4) | 4.1 ms (4–4) | **24.6× faster** | **2.8× faster** |
| sieve | 178 ms (174–179) | 471 ms (462–487) | 48 ms (47–49) | 4.8 ms (4–5) | 5.3 ms (5–6) | **2.6× faster** | 3.7× slower |
| matrix | 117 ms (114–135) | 248 ms (240–262) | 26 ms (25–27) | 3.1 ms (2–3) | 4.5 ms (4–5) | **2.1× faster** | 4.6× slower |
| mandelbrot | 6.0 ms (6–13) | 233 ms (212–235) | 24 ms (23–34) | 3.5 ms (3–6) | 4.0 ms (3–6) | **38.8× faster** | **3.9× faster** |
| lists | 87 ms (86–88) | 270 ms (258–274) | 72 ms (68–85) | 4.9 ms (5–6) | 5.5 ms (5–11) | **3.1× faster** | 1.2× slower |
| list_index | 14 ms (8–19) | 77 ms (76–81) | 24 ms (23–25) | 2.8 ms (3–3) | 2.9 ms (3–6) | **5.6× faster** | **1.8× faster** |
| records | 161 ms (157–164) | 326 ms (325–329) | 28 ms (26–40) | 2.4 ms (2–3) | 2.6 ms (2–3) | **2.0× faster** | 5.7× slower |
| strings | 197 ms (194–198) | 275 ms (269–279) | 40 ms (38–49) | 84 ms (81–86) | 102 ms (101–104) | **1.4× faster** | 4.9× slower |
| calls | 74 ms (73–75) | 186 ms (182–204) | 34 ms (33–50) | 2.2 ms (2–2) | 2.6 ms (2–3) | **2.5× faster** | 2.1× slower |
| closures | 110 ms (108–112) | 227 ms (221–230) | 28 ms (25–38) | 4.8 ms (5–8) | 4.2 ms (4–5) | **2.1× faster** | 4.0× slower |
| locals | 11 ms (10–11) | 262 ms (261–288) | 61 ms (60–61) | 2.1 ms (2–3) | 2.5 ms (2–3) | **24.7× faster** | **5.8× faster** |
| globals | 99 ms (93–126) | 318 ms (311–337) | 26 ms (25–31) | 4.5 ms (4–5) | 11 ms (10–11) | **3.2× faster** | 3.8× slower |
| record_access | 241 ms (228–244) | 852 ms (818–885) | 28 ms (25–31) | 5.4 ms (5–6) | 11 ms (10–12) | **3.5× faster** | 8.5× slower |
| struct_access | 10 ms (10–15) | 186 ms (185–190) | 25 ms (24–26) | 4.8 ms (4–9) | 11 ms (10–11) | **18.2× faster** | **2.5× faster** |
| parallel | 215 ms (209–239) | 342 ms (337–352) | 64 ms (62–72) | 20 ms (20–22) | 20 ms (20–21) | **1.6× faster** | 3.4× slower |

### Cold start

Small inputs where start-up time matters. Median of 21 runs.

| Benchmark | Khipu | Python | Node | C | Rust | vs Python | vs Node |
|---|---|---|---|---|---|---|---|
| fib | 5.2 ms (5–6) | 23 ms (23–47) | 22 ms (21–28) | 2.5 ms (2–3) | 2.7 ms (2–3) | **4.5× faster** | **4.3× faster** |
| fib_iterative | 11 ms (11–14) | 53 ms (52–74) | 24 ms (22–35) | 3.0 ms (3–5) | 3.2 ms (3–12) | **4.8× faster** | **2.1× faster** |
| loop | 5.5 ms (5–6) | 220 ms (207–252) | 32 ms (31–43) | 2.0 ms (2–2) | 2.4 ms (2–3) | **39.7× faster** | **5.9× faster** |
| nested_loops | 4.2 ms (4–9) | 18 ms (18–35) | 22 ms (22–27) | 2.4 ms (2–3) | 2.6 ms (2–13) | **4.4× faster** | **5.4× faster** |
| sieve | 9.1 ms (9–10) | 26 ms (25–29) | 24 ms (23–46) | 2.3 ms (2–3) | 2.5 ms (2–3) | **2.8× faster** | **2.6× faster** |
| matrix | 5.8 ms (5–17) | 17 ms (16–34) | 21 ms (20–33) | 2.1 ms (2–3) | 2.3 ms (2–3) | **2.9× faster** | **3.6× faster** |
| mandelbrot | 4.0 ms (3–9) | 26 ms (24–57) | 24 ms (21–48) | 2.3 ms (2–4) | 2.6 ms (2–25) | **6.5× faster** | **6.0× faster** |
| lists | 12 ms (11–14) | 39 ms (37–54) | 26 ms (25–32) | 2.6 ms (2–6) | 2.9 ms (3–4) | **3.3× faster** | **2.2× faster** |
| list_index | 4.9 ms (4–10) | 19 ms (18–31) | 22 ms (20–25) | 2.2 ms (2–3) | 2.4 ms (2–3) | **3.9× faster** | **4.5× faster** |
| records | 19 ms (19–23) | 44 ms (42–46) | 24 ms (22–37) | 2.3 ms (2–3) | 2.5 ms (2–6) | **2.3× faster** | **1.3× faster** |
| strings | 24 ms (22–27) | 40 ms (38–42) | 25 ms (23–27) | 10 ms (10–12) | 11 ms (10–12) | **1.7× faster** | **1.1× faster** |
| calls | 11 ms (10–14) | 30 ms (29–37) | 23 ms (22–39) | 2.3 ms (2–4) | 2.4 ms (2–3) | **2.8× faster** | **2.1× faster** |
| closures | 14 ms (14–18) | 34 ms (33–78) | 22 ms (21–25) | 2.5 ms (2–5) | 2.5 ms (2–3) | **2.4× faster** | **1.5× faster** |
| locals | 4.4 ms (4–10) | 38 ms (37–43) | 25 ms (24–52) | 2.3 ms (2–3) | 2.5 ms (2–3) | **8.5× faster** | **5.8× faster** |
| globals | 23 ms (21–30) | 74 ms (73–97) | 23 ms (21–31) | 2.8 ms (3–3) | 4.2 ms (4–4) | **3.2× faster** | 1.0× slower |
| record_access | 27 ms (26–28) | 95 ms (91–99) | 22 ms (21–40) | 2.6 ms (2–3) | 3.3 ms (3–5) | **3.5× faster** | 1.2× slower |
| struct_access | 4.2 ms (4–5) | 30 ms (29–35) | 23 ms (21–32) | 2.5 ms (2–4) | 3.2 ms (3–4) | **7.1× faster** | **5.3× faster** |
| parallel | 9.9 ms (9–18) | 111 ms (109–128) | 41 ms (39–55) | 2.9 ms (2–3) | 3.1 ms (2–4) | **11.3× faster** | **4.2× faster** |

### Peak memory

Highest resident memory of any sustained run (cold run for start-up).

| Benchmark | Khipu | Python | Node | C | Rust |
|---|---|---|---|---|---|
| fib | 6.0 MiB | 12.8 MiB | 52.0 MiB | 1.6 MiB | 1.8 MiB |
| fib_iterative | 6.0 MiB | 12.9 MiB | 53.2 MiB | 1.6 MiB | 1.8 MiB |
| loop | 5.9 MiB | 12.8 MiB | 54.2 MiB | 1.6 MiB | 1.8 MiB |
| nested_loops | 6.2 MiB | 12.8 MiB | 52.4 MiB | 1.6 MiB | 1.8 MiB |
| sieve | 9.2 MiB | 35.8 MiB | 79.9 MiB | 4.5 MiB | 4.7 MiB |
| matrix | 7.2 MiB | 13.4 MiB | 55.2 MiB | 2.0 MiB | 2.2 MiB |
| mandelbrot | 6.3 MiB | 12.9 MiB | 53.9 MiB | 1.6 MiB | 1.8 MiB |
| lists | 29.2 MiB | 129.8 MiB | 226.3 MiB | 26.6 MiB | 26.8 MiB |
| list_index | 6.3 MiB | 12.8 MiB | 52.4 MiB | 1.6 MiB | 1.8 MiB |
| records | 6.8 MiB | 12.8 MiB | 54.9 MiB | 1.6 MiB | 1.8 MiB |
| strings | 4.5 MiB | 12.8 MiB | 60.4 MiB | 1.6 MiB | 1.9 MiB |
| calls | 5.8 MiB | 12.8 MiB | 54.0 MiB | 1.6 MiB | 1.8 MiB |
| closures | 4.5 MiB | 12.9 MiB | 52.5 MiB | 1.6 MiB | 1.8 MiB |
| locals | 6.2 MiB | 12.8 MiB | 54.0 MiB | 1.6 MiB | 1.8 MiB |
| globals | 4.6 MiB | 12.8 MiB | 52.5 MiB | 1.6 MiB | 1.8 MiB |
| record_access | 4.5 MiB | 12.8 MiB | 52.5 MiB | 1.6 MiB | 1.8 MiB |
| struct_access | 6.0 MiB | 12.9 MiB | 52.5 MiB | 1.6 MiB | 1.8 MiB |
| parallel | 15.2 MiB | 22.4 MiB | 129.9 MiB | 1.8 MiB | 1.9 MiB |
| empty | 4.5 MiB | 12.8 MiB | 48.2 MiB | 1.6 MiB | 1.8 MiB |

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
