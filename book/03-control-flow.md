# 3. Control flow

## if, else if, else

The condition has no parentheses, the body always has braces, and the condition must be a bool:

```js
let temperature = 18

if temperature > 25 {
    print("warm")
} else if temperature > 15 {
    print("mild")
} else {
    print("cold")
}
```

```text
mild
```

Combine conditions with `&&` and `||`. Both stop early: the right side only runs if it is needed.

```js
let age = 30
let member = false

if age >= 18 && !member {
    print("adult, not a member")
}
```

```text
adult, not a member
```

Because there is no truthiness, `if age {` is an error. Write the comparison you mean.

## while

```js
let n = 1

while n < 100 {
    n = n * 2
}
print(n)
```

```text
128
```

## for ... in

`for` walks over a list, over the characters of a string, or over a `range`:

```js
for fruit in ["apple", "pear"] {
    print(fruit)
}

for letter in "hi" {
    print(letter)
}
```

```text
apple
pear
h
i
```

To walk over the fields of a record, loop over `keys(record)` and look each one up (chapter 5).

## range

`range(end)` counts from 0 up to but not including `end`. `range(start, end)` starts where you say. A range counts on demand, so `range(1000000)` does not build a million-element list.

```js
for i in range(3) {
    print(i)
}
for i in range(2, 5) {
    print(i)
}
print(range(1, 4))
```

```text
0
1
2
2
3
4
[1, 2, 3]
```

When you do not need the counter, call it `_`:

```js
for _ in range(2) {
    print("hello")
}
```

## break and continue

`break` leaves the loop. `continue` skips to the next turn:

```js
for i in range(10) {
    if i % 2 == 0 {
        continue
    }
    if i > 6 {
        break
    }
    print(i)
}
```

```text
1
3
5
```

## Exercises

1. Print the numbers from 1 to 20, but print `fizz` instead of any multiple of 3.
2. Find the first power of 2 larger than 1000 with a `while` loop and print it.
3. Print the sum of the odd numbers below 100 using `for`, `range` and `continue`.

### Solutions

```js
for i in range(1, 21) {
    if i % 3 == 0 {
        print("fizz")
    } else {
        print(i)
    }
}
```

```js
let power = 1
while power <= 1000 {
    power = power * 2
}
print(power)
```

```js
let sum = 0
for i in range(100) {
    if i % 2 == 0 {
        continue
    }
    sum = sum + i
}
print(sum)
```

The second prints `1024` and the third `2500`.
