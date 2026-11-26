## What

One or two sentences: what changes, and what a user of Khipu notices.

## Why

The problem, the issue it closes, or the roadmap item it ticks.

## Checks

- [ ] `cargo test` passes, and a new behaviour has a test
- [ ] A new or changed error message has a test for its wording and line
- [ ] Examples in the README, `docs/` or `book/` still run (`python3 scripts/check-docs.py`)
- [ ] No comments in the code, except on `unsafe` or where the reason cannot be seen from the code
- [ ] Benchmarks rerun with `python3 scripts/bench.py` if speed or memory changed

## Experiment results (if this is an experimental item)

The hypothesis, what was measured, the numbers, and whether the bet lives or dies.
