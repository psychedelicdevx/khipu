# Experiments

Each file is the write-up of one item from the [Experimental section of the roadmap](../ROADMAP.md#experimental): the context, what was built, the numbers, and what was left out. The numbers are taken the way [docs/benchmarks.md](../docs/benchmarks.md) describes, and the honest ones include the runs that lost.

| File | Roadmap item | Outcome |
|---|---|---|
| [1-flat-structs.md](1-flat-structs.md) | Flat typed structs, first slice: structs inside a kernel | 20× on struct-heavy loops; merged |
| [1b-structs-across-calls-and-lists.md](1b-structs-across-calls-and-lists.md) | Flat typed structs, second slice: structs across calls, lists of numbers | `list_index` 5.6× faster than Python, 1.8× than Node; merged |

A new experiment gets the next number, a file here, and a box in the roadmap it will tick or leave open.
