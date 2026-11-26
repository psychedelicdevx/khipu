# 13. The web

Khipu programs can run in a browser, in two ways: as a single HTML page you build with `khipu web`, and in the playground, an editor in the browser.

## khipu web

```bash
khipu web app.kp
```

writes `app.html`: one file that holds your program and the Khipu runtime, compiled to WebAssembly. Open it with a double-click. It works offline, runs the program in the background so the page stays responsive, and shows output and errors just as the terminal does.

Take a program that prints a lot of numbers:

```js
fn fib(n: number) -> number {
    if n < 2 {
        return n
    }
    return fib(n - 1) + fib(n - 2)
}

for i in range(15) {
    print(fib(i))
}
```

Run `khipu web fib.kp` on that and open `fib.html`: the page shows the same fifteen lines the terminal would. The program is type-checked first, so a mistake is reported when you build the page, before anything is written.

Functions that work only with numbers and bools, like `fib` here, are compiled straight to WebAssembly instructions when the page is built, which makes `fib(32)` take 25 ms in the browser instead of over a second in the interpreter (chapter 14 explains which functions qualify).

## What web pages cannot do yet

A page lives in the browser's sandbox, so some things are not available:

- Importing other files or packages. `khipu web` refuses a program with an `import` and says `web pages cannot import other files or packages yet`. Put the whole program in one file.
- Files, HTTP and tasks. `File`, `Http`, `spawn` and `channel` do not work in a page yet.
- `Process.exit`, which stops with `Process.exit is not available in the playground`. A page also has no terminal, so do not rely on `input`.

Everything else behaves as it does in the terminal: types, structs, enums, `match`, contracts, closures and the whole list and string library.

## The playground

The playground runs the interpreter in the browser, in a web worker, so a long loop never freezes the page, and Stop ends it. Errors are underlined as you type, Format tidies the code the way `khipu fmt` does, and Copy link shares a program. The same limits apply as for web pages. To run it yourself:

```bash
scripts/build-playground.sh
python3 -m http.server -d playground 8000
```

Then open http://localhost:8000. You need the WebAssembly target once: `rustup target add wasm32-unknown-unknown`.

## Exercises

1. Build a page from a program that prints the first ten square numbers.
2. Why can the `Http.serve` example from chapter 11 not run in a page?
3. How would you share a small program with a friend who has no Khipu installed?

### Solutions

```js
for n in range(1, 11) {
    print(n * n)
}
```

Build it with `khipu web squares.kp`.

A page cannot open a server socket; HTTP is not available in pages yet.

Send the playground's "Copy link" URL, or send the `.html` file that `khipu web` wrote: it needs nothing but a browser.
