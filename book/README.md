# The Khipu Book

A tour of Khipu for someone who knows a little JavaScript. Each chapter teaches one part of the language with examples you can run, and ends with a few exercises and their solutions.

Every `js` code block in this book is a complete program. `python3 scripts/check-docs.py` runs all of them, so the examples shown here run as written.

1. [Getting started](01-getting-started.md): install, run a file, the prompt, `khipu fmt`
2. [Values and variables](02-values-and-variables.md): numbers, strings, bools, `none`, `let`, scope
3. [Control flow](03-control-flow.md): `if`, `while`, `for`, `range`, `break`, `continue`
4. [Functions](04-functions.md): declarations, closures, functions as values
5. [Lists and records](05-lists-and-records.md): literals, mutation, equality, methods
6. [Types](06-types.md): annotations, inference, optional types, generics
7. [Structs and enums](07-structs-and-enums.md): methods, variants, `match`
8. [Modules](08-modules.md): splitting a program across files
9. [Contracts](09-contracts.md): `requires`, `ensures`, `example`, `test`
10. [Tasks and channels](10-tasks-and-channels.md): `spawn`, `send`, `receive`, `select`
11. [The standard library tour](11-standard-library.md): `Math`, `File`, `Process`, `input`, `Json`, `Http`
12. [Packages](12-packages.md): `khipu init`, `khipu add`, `khipu.json`, `khipu.lock`
13. [The web](13-the-web.md): `khipu web` and the playground
14. [Performance](14-performance.md): what runs natively, and why

If you want exact rules rather than a tour, [A tour of Khipu](../docs/tour.md) is the short reference and `src/prelude.kp` lists every standard-library method.
