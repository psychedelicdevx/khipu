# 6. Types

Khipu is statically typed: it checks your whole program before running any of it. You rarely have to write types, because most are worked out from the code.

## Inference

A variable gets the type of its first value and keeps it:

```js
let count = 5
count = count + 1
print(count)
```

Writing `count = "six"` afterwards is rejected before the program starts:

```text
error: 'count' should be number, got string
 --> t.kp:2
  |
2 | count = "six"
  | ^^^^^
```

The basic types are `number`, `string`, `bool` and `none`, plus `list<T>`, `record`, `any` and function types. Struct and enum names are types too (chapter 7).

## Annotations

Write a type after a colon when you want to be explicit, or when there is no value to infer from. An empty list, for instance, needs one:

```js
let total: number = 0
let names: list<string> = []
let grid: list<list<number>> = [[1], [2, 3]]
let settings: record = { debug: true }
names.push("Ada")
print(total, names, grid, settings)
```

```text
0 ["Ada"] [[1], [2, 3]] {debug: true}
```

Pushing a number into `names` would be an error: `argument 1 of 'list.push' should be string, got number`.

`any` turns checking off for one value. It is useful at the edges of a program (the result of `Json.parse` is `any`) and best avoided elsewhere:

```js
let data: any = Json.parse("{\"id\": 7}")
print(data.id + 1)
```

```text
8
```

## Typed signatures

Annotate parameters with `name: type` and the result with `-> type`:

```js
fn price_with_tax(price: number, rate: number) -> number {
    return price + price * rate
}

fn label(title: string, price: number) -> string {
    return "${title}: ${price_with_tax(price, 0.2)}"
}

print(label("Book", 40))
```

```text
Book: 48
```

Now the checker can reject wrong calls without running anything: `argument 1 of 'price_with_tax' should be number, got string`, `'f' should return number, got string` for a wrong `return`, or `'g' should return number, but can reach the end without returning` for a path with no `return`.

A function type is written `fn(number) -> number`:

```js
fn apply(f: fn(number) -> number, value: number) -> number {
    return f(value)
}

print(apply(fn(n: number) -> number { return n * 2 }, 21))
```

```text
42
```

## Optional types

A type followed by `?` may also be `none`. Any function that can fail to find something returns one:

```js
fn find_discount(code: string) -> number? {
    if code == "WELCOME" {
        return 5
    }
    return none
}

print(find_discount("WELCOME"), find_discount("HELLO"))
```

```text
5 none
```

The checker will not let you use a `number?` as a `number`. `find_discount("x") + 1` is an error: `cannot use '+' on number? and number`. You have two ways to deal with the `none`.

`??` gives a fallback when the left side is `none`:

```js
fn find_discount(code: string) -> number? {
    if code == "WELCOME" {
        return 5
    }
    return none
}

let discount = find_discount("HELLO") ?? 0
print(discount)
```

```text
0
```

`?.` reads a field only when the value is there, and gives `none` otherwise:

```js
fn find_user(id: number) -> record? {
    if id == 1 {
        return { name: "Ada", city: "London" }
    }
    return none
}

print(find_user(1)?.name)
print(find_user(2)?.name)
print(find_user(2)?.name ?? "nobody")
```

```text
Ada
none
nobody
```

Plain `find_user(2).name` is rejected before the program runs: `cannot read field 'name' of a value that might be none, use '?.name'`. This is the class of bug that crashes JavaScript programs at runtime.

## Generics

A function can work for any element type by naming a type variable in angle brackets. `T` stands for whatever type the caller uses, and the checker keeps track of it:

```js
fn first_or<T>(items: list<T>, fallback: T) -> T {
    if items.is_empty() {
        return fallback
    }
    return items[0]
}

print(first_or([4, 5], 0))
print(first_or(["a"], "none yet"))
print(first_or([], "none yet"))
```

```text
4
a
none yet
```

With two type variables, a function can change the element type, like `map` does:

```js
fn transform<T, U>(items: list<T>, f: fn(T) -> U) -> list<U> {
    let out: list<U> = []
    for item in items {
        out.push(f(item))
    }
    return out
}

print(transform([1, 2, 3], fn(n: number) -> string { return "#${n}" }))
```

```text
["#1", "#2", "#3"]
```

The built-in list methods are generic in just this way: `list#map<T, U>` is written in Khipu in `src/prelude.kp`.

## Exercises

1. Write `safe_div(a: number, b: number) -> number?` that returns `none` when `b` is 0, and print `safe_div(1, 0) ?? -1`.
2. Write a generic `last_or<T>(items: list<T>, fallback: T) -> T`.
3. Why is `let x: number = none` an error, and how do you declare a variable that can hold a number or `none`?

### Solutions

```js
fn safe_div(a: number, b: number) -> number? {
    if b == 0 {
        return none
    }
    return a / b
}

print(safe_div(1, 0) ?? -1)
print(safe_div(9, 3) ?? -1)
```

```js
fn last_or<T>(items: list<T>, fallback: T) -> T {
    if items.is_empty() {
        return fallback
    }
    return items[items.len() - 1]
}

print(last_or([1, 2, 3], 0))
```

Plain `number` never holds `none`; write `let x: number? = none`.
