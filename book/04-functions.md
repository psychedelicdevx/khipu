# 4. Functions

## Declarations

```js
fn greet(name) {
    return "hello, ${name}"
}

print(greet("Ada"))
```

```text
hello, Ada
```

A function without a `return`, or with a bare `return`, gives back `none`. Calling a function with the wrong number of arguments is an error: `greet expects 1 argument, got 2`.

A function must be declared before the line that calls it runs, but one function may call another declared further down, as long as both exist by the time the call happens.

## Recursion

A function can call itself:

```js
fn factorial(n) {
    if n <= 1 {
        return 1
    }
    return n * factorial(n - 1)
}

print(factorial(10))
```

```text
3628800
```

Recursion is limited to 10000 calls deep. Going past that stops the program with `too much recursion: more than 10000 calls deep` on the line that recursed, instead of crashing.

## Functions are values

A function can be stored in a variable, passed to another function and returned. An anonymous function is written with `fn` and no name:

```js
let double = fn(n) {
    return n * 2
}

fn apply_twice(f, value) {
    return f(f(value))
}

print(double(4))
print(apply_twice(double, 3))
print(apply_twice(fn(s) { return s + "!" }, "hey"))
```

```text
8
12
hey!!
```

The standard library takes advantage of this: `[1, 2, 3].map(double)` calls `double` on each item (chapter 5).

## Closures

A function remembers the variables around it, even after the outer function has returned. Each call to `make_counter` gets its own `n`:

```js
fn make_counter() {
    let n = 0
    return fn() {
        n = n + 1
        return n
    }
}

let a = make_counter()
let b = make_counter()
a()
a()
print(a(), b())
```

```text
3 1
```

Closures work for configuration too:

```js
fn adder(amount) {
    return fn(x) {
        return x + amount
    }
}

let add10 = adder(10)
print(add10(5))
```

```text
15
```

## Types are optional here

The examples above leave parameters and results untyped, and Khipu works out what it can. Chapter 6 shows how to write them down (`fn greet(name: string) -> string`), which lets the checker catch wrong calls before the program runs.

## Exercises

1. Write `max_of(a, b)` that returns the larger number, without using a built-in.
2. Write `compose(f, g)` that returns a function computing `f(g(x))`, then use it to build "double, then add one".
3. Write a function `fib(n)` using recursion and print the first ten Fibonacci numbers.

### Solutions

```js
fn max_of(a, b) {
    if a > b {
        return a
    }
    return b
}

print(max_of(3, 9))
```

```js
fn compose(f, g) {
    return fn(x) {
        return f(g(x))
    }
}

let double_then_inc = compose(fn(x) { return x + 1 }, fn(x) { return x * 2 })
print(double_then_inc(5))
```

```js
fn fib(n) {
    if n < 2 {
        return n
    }
    return fib(n - 1) + fib(n - 2)
}

for i in range(10) {
    print(fib(i))
}
```
