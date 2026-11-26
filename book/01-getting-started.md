# 1. Getting started

Khipu looks like JavaScript with the sharp edges filed off. It has no `null` and `undefined` (just `none`), no truthiness, no silent conversion between types, and it checks types before your program runs. It also runs tasks on every core of your machine, which chapter 10 covers.

## Install

You need [Rust](https://rustup.rs). Then:

```bash
git clone git@github.com:psychedelicdevx/khipu.git
cd khipu
scripts/build-playground.sh
cargo install --path .
```

The `build-playground.sh` step is optional: it builds the web runtime that `khipu web` embeds (chapter 13). Without it everything else works.

## Run a file

Khipu files end in `.kp`. Make a file called `hello.kp`:

```js
let name = "world"
print("hello, ${name}")
```

Run it:

```bash
khipu hello.kp
```

```text
hello, world
```

`print` takes any number of values and writes them on one line, separated by spaces.

## Mistakes show where they happen

Errors name the problem and point at the line:

```text
error: 'total' should be number, got string
 --> shop.kp:2
  |
2 | let total: number = "free"
  |     ^^^^^
```

Many mistakes like this one are found before the first line runs, because Khipu checks the whole program first. Chapter 6 explains how.

## The prompt

Run `khipu` with no file to get an interactive prompt. Each line you type is run, and the value of an expression is printed back:

```text
> 1 + 2
= 3
> let x = 4
> x * 2
= 8
```

Variables stay defined from one line to the next. A block can span several lines: the prompt waits until your braces are closed. A mistake prints an error and leaves the prompt ready for the next line.

## Format your code

`khipu fmt` rewrites a file into the one standard layout: four-space indents, no semicolons, comments kept.

```bash
khipu fmt hello.kp
khipu fmt --check hello.kp
```

The second form changes nothing and exits with an error if the file is not already formatted, which is handy in a pre-commit hook. The code in this book is in `khipu fmt` style.

## Comments

A comment starts with `//` and runs to the end of the line.

## Exercises

1. Write a program that prints `Khipu` followed by the number `2026` on one line, using `print` with two values.
2. Write a program using `${}` that prints `2 plus 3 is 5`, where the 5 is computed by the program.
3. Open the prompt, define `let r = 3`, and compute `r * r * 3.14159`. The answer is `28.27431`.

### Solutions

```js
print("Khipu", 2026)
```

```js
print("2 plus 3 is ${2 + 3}")
```

For the third, `r * r * 3.14159` gives `28.27431`. Numbers are ordinary floating point, as in JavaScript.
