<p align="center">
  <img src="editors/vscode/icons/khipu.svg" width="120" alt="Khipu logo">
</p>

<h1 align="center">Khipu</h1>

<p align="center">
  <strong>The smart adapt to the world. The fool tries to change it.</strong>
</p>

<p align="center">
  <a href="https://github.com/psychedelicdevx/khipu/actions/workflows/ci.yml"><img src="https://github.com/psychedelicdevx/khipu/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <img src="https://img.shields.io/badge/written%20in-Rust-B7410E?logo=rust&logoColor=white" alt="Written in Rust">
  <img src="https://img.shields.io/badge/status-early%20development-FB923C" alt="Status: early development">
  <img src="https://img.shields.io/badge/files-.kp-E8A23A" alt="File extension: .kp">
  <img src="https://img.shields.io/badge/license-MIT-3B82F6" alt="MIT licence">
</p>

---

Every tool you trust was a fool's idea first. The smart see the world as it is and fit themselves to it. The fool decides the world is wrong, tries to fix it, fails in front of everyone, and tries again. That is the only way anything new has ever been made, and it is what this repository is for.

Khipu is a programming language and a laboratory. The language is typed, runs on every core, compiles to native code and WebAssembly, and is written in Rust. The laboratory is the [Experimental section of the roadmap](ROADMAP.md#experimental): ideas that should not work, each written down with the experiment that will prove it or kill it. Shaw wrote that all progress depends on the unreasonable man. Here we try to be unreasonable on purpose, and to measure it.

In daily use, Khipu keeps the feel of JavaScript and drops the parts people complain about: no silent type coercion, no truthiness, no `null` *and* `undefined`. Static types catch mistakes before anything runs, and lightweight tasks with channels give safe parallelism across every CPU core.

The name comes from the **khipu**, the knotted cords the Inca used to record numbers and data: many strings working side by side, which is the idea behind Khipu's concurrency.

## Performance

On sustained runs Khipu beats Python on 18 of 18 benchmarks and Node on 7 of 18, and uses less memory than both on every one. Start-up, an empty program (median of 21 runs): Khipu 3.2 ms · Python 12 ms · Node 20 ms · C 2.0 ms · Rust 2.1 ms. The tables, spreads, memory use and the method are in [docs/benchmarks.md](docs/benchmarks.md).

## A taste

Types that are checked before anything runs, a contract on the function, and work spread over every core with nothing shared:

```js
let results = channel()

fn square(n: number) -> number
    ensures result >= 0
{
    return n * n
}

fn work(n: number) {
    send(results, square(n))
}

for n in range(1, 11) {
    spawn(work, n)
}

let total = 0
for _ in range(10) {
    total = total + receive(results)
}
print(total)
```

## Getting started

You need [Rust](https://rustup.rs).

```bash
git clone git@github.com:psychedelicdevx/khipu.git
cd khipu
scripts/build-playground.sh   # optional: the web runtime that khipu web embeds
cargo install --path .
```

Run a file:

```bash
khipu examples/primes.kp
```

New to the language? [The Khipu Book](book/README.md) teaches it chapter by chapter, with examples that run.

## Documentation

- [The Khipu Book](book/README.md): the language, chapter by chapter, with examples that run
- [A tour of Khipu](docs/tour.md): the short reference: contracts, the standard library, tasks, packages, and where Khipu is strict
- [Tooling](docs/tooling.md): editor support, native code, `khipu web` and the playground
- [docs/benchmarks.md](docs/benchmarks.md): every measurement and how it was taken
- [ROADMAP.md](ROADMAP.md): what is done, what is next, and the experiments
- [docs/audit.md](docs/audit.md): the Phase 8 audit and independent review

## Roadmap

| Phase | Progress |
|---|---|
| 0. Foundations | `██████████` 12 / 12 |
| 1. Core language | `██████████` 10 / 10 |
| 2. Speed | `██████████` 7 / 7 |
| 3. Types | `██████████` 7 / 7 |
| 4. Benchmarks | `██████████` 8 / 8 |
| 5. Tooling | `██████████` 8 / 8 |
| 6. Concurrency | `██████████` 5 / 5 |
| 7. Ecosystem | `██████████` 6 / 6 |
| 8. Performance and final audit | `█████████░` 17 / 18 |
| 9. HTTP without limits | `██████████` 23 / 24 |
| 10. Cleanup | `█░░░░░░░░░` 1 / 10 |
| 11. Documentation | `██░░░░░░░░` 1 / 5 |

**Next up:** Competitive with Node on the benchmark suite (as of 2026-11-27: faster on 7 of 18, within 1.2× on lists, 2–9× slower on records, strings, closures, globals and calls; see docs/audit.md)

The full list is in [ROADMAP.md](ROADMAP.md). The progress bars update themselves on every commit.

## Contributing, security, privacy

[CONTRIBUTING.md](CONTRIBUTING.md) explains how to work on the language and how to run or propose an experiment, and [CODE_OF_CONDUCT.md](.github/CODE_OF_CONDUCT.md) is short: argue with code, be kind to people. [SECURITY.md](.github/SECURITY.md) says how to report a vulnerability privately and what counts as one. [PRIVACY.md](.github/PRIVACY.md) is short: Khipu collects nothing.

## Licence

[MIT](LICENSE).
