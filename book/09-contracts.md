# 9. Contracts

A contract is what a function says about itself: what it needs, what it promises, and a few examples of it working. Khipu checks the first two while the program runs, and runs the examples on demand.

## requires and ensures

Contract lines go between the signature and the opening brace of the body:

```js
fn divide(a: number, b: number) -> number
    requires b != 0
    ensures result * b == a
    example divide(10, 2) == 5
{
    return a / b
}

print(divide(10, 2))
```

```text
5
```

`requires` is checked on every call, before the body runs. `ensures` is checked on every return, and inside it `result` is the value being returned. A function can have several of each.

A broken `requires` blames the caller, and names the rule and the values:

If `divide` is called with `b` equal to 0, the program stops there:

```text
error: divide requires b != 0 (a = 3, b = 0)
 --> shop.kp:12
   |
12 | print(divide(total, count))
   |       ^^^^^^^^^^^^^^^^^^^^
```

A broken `ensures` blames the function, pointing at its body: `bad ensures result > 100, but it returned 1`.

That makes a contract more useful than a comment: when someone changes a function, or an AI does, and the new version breaks what its callers rely on, the program stops at the exact call and says which promise failed.

## example

An `example` line is an expression that must be true. It does not run in a normal program run, so it costs nothing. `khipu test` evaluates every example and reports the ones that fail.

```js
fn clamp01(x: number) -> number
    ensures result >= 0 && result <= 1
    example clamp01(5) == 1
    example clamp01(-2) == 0
    example clamp01(0.25) == 0.25
{
    if x < 0 {
        return 0
    }
    if x > 1 {
        return 1
    }
    return x
}

print(clamp01(0.5))
```

```text
0.5
```

Run `khipu test file.kp` to see:

```text
file.kp
  ✓ clamp01: clamp01(5) == 1
  ✓ clamp01: clamp01(-2) == 0
  ✓ clamp01: clamp01(0.25) == 0.25

3 passed, 0 failed
```

A failing example shows what it expected:

```text
  ✗ d: d(10) == 11
error: expected 11, got 10
```

## test blocks

For anything bigger than a one-liner, write a `test` block. It has a name and a body, and uses `assert` and `assert_eq`:

```js
fn add(a: number, b: number) -> number {
    return a + b
}

test "adds numbers" {
    assert_eq(add(2, 3), 5)
    assert(add(-1, 1) == 0, "should cancel out")
}

test "adds fractions" {
    assert_eq(add(0.5, 0.25), 0.75)
}
```

`assert_eq(actual, expected)` fails with `expected 2, got 1` on the line of the assertion, and `assert(condition, message)` fails with your message. A normal run of the file skips every `test` block, so tests can live right beside the code they check. `khipu test file.kp` runs them, counts passes and failures, and exits with an error if any failed, so it works as a step in a commit hook or a build.

## Exercises

1. Write `sqrt_of(x: number) -> number` that `requires x >= 0` and `ensures result * result` is close to `x` (compare with a small tolerance, written as `Math.abs(result * result - x) < 0.001`), and prints `sqrt_of(16)`.
2. Add two `example` lines to a function `double(n)` and run `khipu test`.
3. Write a `test` block for a function `is_even(n)` that checks `4` and `7`.

### Solutions

```js
fn sqrt_of(x: number) -> number
    requires x >= 0
    ensures Math.abs(result * result - x) < 0.001
{
    return Math.sqrt(x)
}

print(sqrt_of(16))
```

```js
fn double(n: number) -> number
    example double(2) == 4
    example double(0) == 0
{
    return n * 2
}

print(double(21))
```

```js
fn is_even(n: number) -> bool {
    return n % 2 == 0
}

test "is_even" {
    assert(is_even(4), "4 is even")
    assert(!is_even(7), "7 is odd")
}
```
