# Security

## Reporting

Report a vulnerability privately through GitHub: the **Security** tab of this repository, then **Report a vulnerability**. Do not open a public issue for it. You will get an answer within a week, and a fix or a written reason before anything is disclosed.

## What counts

Anything that lets a Khipu program, a package, a document or a web page do what the language says it cannot:

- A package that runs code during `khipu install` or `khipu add`, reaches outside the project folder, or pulls a dependency from a local path. Installing never runs package code, package imports only look inside the project, and packages' own dependencies come only from `https://`, `ssh://` or `git@` sources.
- A `khipu web` page whose program text breaks out of the page's script.
- A crash, hang or memory corruption from source text: the lexer, parser, checker and VM are fuzzed and must not crash on any input.
- Native code (the JIT or the WebAssembly backend) that disagrees with the interpreter, or that reads or writes memory it was not given.
- A deadlock the runtime does not report, or a data race between tasks. Tasks share nothing; whatever is sent is copied.

## What does not

Programs you run do what they say: `File`, `Http` and `Process` are there to be used, and a Khipu program can read and write your files and reach the network like any program you run.
