# Tooling

The editor support, the native tier, web pages and the playground. Installing the `khipu` command itself is in the [README](../README.md#getting-started).

## Editor support

`khipu lsp` is a language server. Any editor that speaks LSP gets:

- errors as you type, underlined where the problem is
- hover to see the type of any name, field or method
- autocomplete for names, keywords, struct fields and methods, and enum variants, which keeps working while the line is half typed
- formatting, the same as `khipu fmt`

`editors/vscode` is a VS Code extension that uses it, plus syntax highlighting, the `.kp` file icon, comment toggling and bracket handling. It runs `khipu` from your `PATH`; set `khipu.path` if it lives somewhere else. To build it:

```bash
cargo install --path .
cd editors/vscode && npm install && npx @vscode/vsce package
code --install-extension khipu-*.vsix
```

If you use the Symbols icon theme, run `editors/vscode/patch-symbols.sh` to give `.kp` files the KP icon there too.

`editors/tree-sitter-khipu` is a tree-sitter grammar with highlight queries, for Zed, Neovim, Helix and anything else built on tree-sitter. It parses every example and benchmark in this repository; run its tests with `npx tree-sitter-cli test` in that folder.

## Native code

Functions that work only with numbers and bools are compiled to machine code while the program runs. A function qualifies when its body uses only number and bool values, arithmetic, comparisons, `&&` `||` `!`, `let`, assignment, `if`, `while`, `break`, `continue`, `return`, and calls to other functions that qualify, itself included. The compiler ([Cranelift](https://cranelift.dev)) runs the 64th time such a function is called, or on the first call if the function contains a loop, and from then on the call runs natively: `fib(32)` goes from 165 ms to 24 ms. The same goes for a `while` or `for ... in range(...)` loop whose variables hold numbers or bools, wherever it appears, even at the top level of a program: the loop is compiled when it is first reached, its variables go in, and their final values come back out. `loop` goes from 341 ms to 26 ms and `mandelbrot` from 78 ms to 7 ms. Nothing changes for the program: arguments or variables of another type fall back to the bytecode, too much recursion is still an error on the right line, and reassigning a global function throws the native code for it away. Spawned tasks keep running bytecode, so they keep taking turns.

## Web pages

`khipu web app.kp` turns a program into `app.html`: one file with the program and the Khipu runtime (compiled to WebAssembly) inside. It opens with a double-click, works offline, runs the program in the background so the page stays responsive, and shows output and errors just like the terminal. The program is type-checked first, so mistakes are caught before the page is written. The functions that qualify for native code (see above) are compiled straight to WebAssembly instructions when the page is built, so `fib(32)` takes 25 ms in the browser instead of over a second in the interpreter. Web pages cannot import other files or use files, HTTP or tasks yet.

## Playground

`playground/` runs Khipu in the browser. The interpreter is compiled to WebAssembly and runs in a web worker, so a long loop never freezes the page and Stop ends it. Errors are underlined as you type, Format tidies the code, and Copy link shares a program.

```bash
scripts/build-playground.sh
python3 -m http.server -d playground 8000
```

Then open http://localhost:8000. You need the WebAssembly target once: `rustup target add wasm32-unknown-unknown`.
