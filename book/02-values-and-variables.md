# 2. Values and variables

## Numbers

There is one kind of number, a 64-bit floating point value, with the usual operators:

```js
print(10 / 4, 7 % 3, 2 * 3 + 1, -5)
print(0.1 + 0.2)
```

```text
2.5 1 7 -5
0.30000000000000004
```

Division always gives a fractional result when it needs to, and `%` is the remainder. Multiplication and division bind tighter than addition, and parentheses group, as you would expect.

## Strings

Strings use double quotes, and `+` joins them:

```js
let first = "Ada"
let last = "Lovelace"
print(first + " " + last)
print("tab:\there", "quote: \"hi\"")
```

To put values inside a string, write `${...}` with any expression inside:

```js
let items = 3
let price = 4.5
print("${items} items cost ${items * price}")
```

```text
3 items cost 13.5
```

`+` does not convert anything. `"a" + 1` is an error that tells you to use `${...}` instead, which is exactly what you should do.

## Bools

`true` and `false`, with `&&`, `||` and `!`:

```js
let ready = true
let late = false
print(ready && late, ready || late, !ready)
print(3 > 2, "a" == "a", 1 != 2)
```

```text
false true false
true true true
```

## none

`none` is the single empty value. There is no `null` and no `undefined`. A function that returns nothing gives `none`, and a lookup that can miss gives `none` when it misses (chapter 6 shows how the type checker makes you handle that).

```js
let nothing = none
print(nothing, nothing == none, 1 == none)
```

```text
none true false
```

## No truthiness, no coercion

In JavaScript `if (1)` runs and `1 + true` is `2`. In Khipu both are errors:

```text
error: if needs true or false, got number
 --> t.kp:1
  |
1 | if 1 { print(1) }
  | ^^^^^^^^^^^^^^^^^

error: cannot use '+' on number and bool
 --> t.kp:1
  |
1 | print(1 + true)
  |         ^
```

Comparing values of different types with `==` is an error too, except against `none`. If you meant something, say it: `count > 0` rather than `count`, `name != ""` rather than `name`.

## Variables

`let` declares a variable. Assigning to a name that was never declared is an error, so a typo cannot silently create a global:

```js
let count = 0
count = count + 1
print(count)
```

```text
error: 'x' is not defined, use 'let x = ...' first
 --> t.kp:1
  |
1 | x = 5
  | ^
```

(That is the error for `x = 5` without a `let`.) A variable keeps the type it started with: after `let count = 0`, writing `count = "zero"` is the error `'count' should be number, got string`.

## Block scope

A variable lives until the end of the block it was declared in. Inside an `if` or loop body you can read and change outer variables, and declare new ones that disappear at the closing brace. An inner `let` with an existing name hides the outer one only inside the block:

```js
let x = 1
if true {
    let inside = 10
    x = x + inside
    let x = 100
    print("inner x", x)
}
print("outer x", x)
```

```text
inner x 100
outer x 11
```

Reading `inside` after the block is an error: `'inside' is not defined`.

## Exercises

1. Compute and print the average of 4, 8 and 15 with one `print`.
2. Print `Total: 7.5` using a variable `total` and interpolation.
3. What does `print(1 == none)` print, and why does `print(1 == "1")` not print anything?

### Solutions

```js
print((4 + 8 + 15) / 3)
```

```js
let total = 7.5
print("Total: ${total}")
```

The first prints `false`: comparing against `none` is allowed and a number is never `none`. The second is a type error (`cannot use '==' on number and string`), so nothing runs.
