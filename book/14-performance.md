# 14. Performance

Khipu runs your program on a bytecode interpreter, and compiles the parts that it can prove are plain arithmetic to machine code while the program runs. You do not ask for this and nothing about the program changes. This chapter says which code gets the fast path, so you can write for it when speed matters.

## What qualifies

A function is compiled when its body uses only number and bool values, arithmetic, comparisons, `&&` `||` `!`, `let`, assignment, `if`, `while`, `break`, `continue`, `return`, and calls to other functions that qualify, itself included. These all qualify:

```js
fn fib(n: number) -> number {
    if n < 2 {
        return n
    }
    return fib(n - 1) + fib(n - 2)
}

fn collatz_steps(start: number) -> number {
    let n = start
    let steps = 0
    while n != 1 {
        if n % 2 == 0 {
            n = n / 2
        } else {
            n = 3 * n + 1
        }
        steps = steps + 1
    }
    return steps
}

print(fib(30))
print(collatz_steps(27))
```

```text
832040
111
```

The compiler ([Cranelift](https://cranelift.dev)) runs the 64th time such a function is called, or on the first call if the function contains a loop. From then on the call runs natively: `fib(32)` goes from 165 ms to 24 ms.

## Loops too

A `while` or `for ... in range(...)` loop whose variables hold numbers or bools is compiled the first time it is reached, wherever it appears, even at the top level of a program. Its variables go in, and their final values come back out:

```js
let total = 0
for i in range(1000000) {
    total = total + i % 7
}
print(total)
```

```text
2999997
```

`loop` in the benchmark suite goes from 341 ms to 26 ms this way, and `mandelbrot` from 78 ms to 7 ms.

## What does not qualify, and why

Anything that touches a value that is not a number or a bool stays on the bytecode: strings, lists, records, structs, enums, closures, `print`, and calls to functions that do not qualify. A function that does that is not wrong or slow in an absolute sense, it is simply interpreted. Compare:

```js
fn sum_to(n: number) -> number {
    let total = 0
    for i in range(n) {
        total = total + i
    }
    return total
}

fn sum_list(items: list<number>) -> number {
    let total = 0
    for item in items {
        total = total + item
    }
    return total
}

print(sum_to(1000))
print(sum_list([1, 2, 3, 4]))
```

```text
499500
10
```

`sum_to` qualifies; `sum_list` takes a list, so it runs on the interpreter. Both give the right answer, and both are fine for small inputs.

The reason is that native code is only safe where the compiler knows the exact shape of every value. Numbers and bools have one; a list or a record can hold anything and change under you. So the rule is conservative on purpose.

## Falling back

Nothing about the program changes when native code applies, and nothing breaks when it cannot:

- Arguments or variables of another type fall back to the bytecode.
- Too much recursion is still an error on the right line, `too much recursion: more than 10000 calls deep`.
- A native loop with a fractional or infinite bound falls back, so `range needs whole numbers` is reported on the right line.
- Reassigning a global function throws the native code for it away.

## Tasks keep running bytecode

Spawned tasks do not run native code. They keep running bytecode so they can take turns, which means a long loop in one task never starves the others.

## Practical advice

- Put hot arithmetic in small functions that take and return numbers. Keep strings, lists and printing outside of them.
- `range` counts on demand, and a list of nothing but numbers or bools is stored compactly, so large numeric lists are cheaper than they look.
- In the browser, the same functions are compiled to WebAssembly when the page is built.
- Measure before you worry. A sustained run on the benchmark suite beats Python on all 17 benchmarks and Node on 5 of 17; strings, records and closures are where it is still slower than Node. The numbers and method are in `BENCHMARKS.md`.

## Exercises

1. Write a function `is_prime(n: number) -> bool` that qualifies for native code, and count the primes below 100.
2. Does a function that calls `print` qualify? What would you change to keep the arithmetic fast?
3. Why does `sum_list` above not qualify, and what would you pass instead to use native code?

### Solutions

```js
fn is_prime(n: number) -> bool {
    if n < 2 {
        return false
    }
    let d = 2
    while d * d <= n {
        if n % d == 0 {
            return false
        }
        d = d + 1
    }
    return true
}

let count = 0
for n in range(100) {
    if is_prime(n) {
        count = count + 1
    }
}
print(count)
```

No: `print` is not a number or bool operation. Return the number from the function and print it outside.

Its parameter is a list. A numeric function that takes the values one at a time, or a count and a loop like `sum_to`, qualifies.
