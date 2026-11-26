# Privacy

Khipu collects nothing. There is no telemetry, no crash reporting, no update check and no account.

The `khipu` command talks to the network only when a program asks it to (`Http`) or when you add or install a package, which runs `git` against the source you named. The language server, the formatter, the test runner and `khipu web` work offline. The playground runs the interpreter in your browser and sends nothing anywhere; **Copy link** puts the program in the URL you copy, and nothing else.

The repository has no analytics either. The benchmark results in `benchmarks/results` record the CPU model, operating system and the versions of the languages measured, and nothing about the machine's owner.
