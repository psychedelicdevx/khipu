# 5. Lists and records

## Lists

A list is an ordered collection in square brackets. Indexes start at 0, and `len` or `.len()` gives the length:

```js
let primes = [2, 3, 5, 7]
print(primes[0], primes[3], primes.len())
primes[0] = 11
print(primes)
```

```text
2 7 4
[11, 3, 5, 7]
```

Reading past the end is an error, not `undefined`: `index 10 is out of range, the length is 4`. Negative indexes are errors too.

`push` adds to the end. Lists are changed in place, and a variable assigned from another variable is the same list:

```js
let a = [1, 2]
let b = a
push(b, 3)
print(a)
```

```text
[1, 2, 3]
```

Lists can nest, and `for` walks them:

```js
let grid = [[1, 2], [3, 4]]
let total = 0
for row in grid {
    for cell in row {
        total = total + cell
    }
}
print(grid[1][0], total)
```

```text
3 10
```

## Records

A record is a set of named fields. Read with `.name`, or with `["name"]` when the name is in a variable or is not a plain identifier. Change an existing field by assigning to it:

```js
let ada = {
    name: "Ada Lovelace",
    born: 1815,
    known_for: ["the first program"],
}

print(ada.name, ada["born"], ada.known_for[0])
ada.born = 1816
print(ada)
```

```text
Ada Lovelace 1815 the first program
{name: "Ada Lovelace", born: 1816, known_for: ["the first program"]}
```

The fields of a record are fixed when it is created. Reading a field that is not there is an error (`record has no field 'b'`), and so is adding one (`record has no field 'y', fields can only be added when the record is created`). If you need a collection that grows, use a list.

`keys` gives the field names, in the order they were written:

```js
let point = { x: 3, y: 4 }
for field in keys(point) {
    print(field, "->", point[field])
}
```

```text
x -> 3
y -> 4
```

## Equality compares contents

Lists and records are equal when their contents are, however deep. This is not JavaScript, where `[1] == [1]` is false:

```js
print([1, [2, 3]] == [1, [2, 3]])
print({ a: 1 } == { a: 1 }, { a: 1 } == { a: 2 })
```

```text
true
true false
```

## String methods

Strings have methods, and none of them change the string; each gives a new value:

```js
let s = "Hello, World"
print(s.len(), s.upper(), s.lower())
print(s.contains("World"), s.starts_with("He"), s.ends_with("x"))
print(s.split(", "), s.slice(0, 5), s.replace("l", "L"))
print("ab".repeat(3), "  padded ".trim(), "".is_empty())
print("a\nb".lines(), "abc".chars())
```

```text
12 HELLO, WORLD hello, world
true true false
["Hello", "World"] Hello HeLLo, WorLd
ababab padded true
["a", "b"] ["a", "b", "c"]
```

`index_of` and `to_number` can fail, so they give `none` instead of a magic `-1` or `NaN`:

```js
print("hello".index_of("l"), "hello".index_of("z"))
print("42".to_number(), "forty".to_number())
print(("42".to_number() ?? 0) + 1)
```

```text
2 none
42 none
43
```

The type checker will not let you add to a `number?` without handling the `none` first; the `??` operator gives a fallback (chapter 6).

## List methods

Methods that return a changed copy leave the original alone (`sorted`, `reversed`, `concat`, `slice`, `map`, `filter`). Methods that change the list in place are `push`, `pop`, `insert` and `remove`.

```js
let l = [3, 1, 2]
print(l.sorted(), l.reversed(), l)
print(l.sum(), l.min(), l.max(), l.len())
print(l.contains(2), l.index_of(2), l.index_of(9))
print(l.join("+"), l.first(), l.last(), [].first())
print(l.slice(0, 2), l.concat([7, 8]))
```

```text
[1, 2, 3] [2, 1, 3] [3, 1, 2]
6 1 3 3
true 2 none
3+1+2 3 2 none
[3, 1] [3, 1, 2, 7, 8]
```

`first`, `last`, `pop`, `min` and `max` give `none` for an empty list. `insert(index, item)` puts an item at a position, `remove(index)` takes one out and returns it, and `pop()` removes and returns the last.

```js
let stack = [1, 2, 3]
print(stack.pop(), stack)
stack.insert(0, 99)
print(stack)
print(stack.remove(1))
print(stack)
```

```text
3 [1, 2]
[99, 1, 2]
1
[99, 2]
```

The ones that take a function are where closures (chapter 4) pay off:

```js
let numbers = [1, 2, 3, 4, 5]
let double = fn(n: number) -> number { return n * 2 }
let is_even = fn(n: number) -> bool { return n % 2 == 0 }
let add = fn(total: number, n: number) -> number { return total + n }

print(numbers.map(double))
print(numbers.filter(is_even))
print(numbers.reduce(0, add))
print(numbers.find(fn(n: number) -> bool { return n > 3 }))
print(numbers.any(is_even), numbers.all(is_even), numbers.count(is_even))
print(["pear", "fig", "apple"].sort_by(fn(s: string) -> number { return s.len() }))
```

```text
[2, 4, 6, 8, 10]
[2, 4]
15
4
true false 2
["fig", "pear", "apple"]
```

`reduce(start, f)` folds the list into one value, `find` gives the first match or `none`, and `sort_by` sorts by whatever the function returns. For these methods, write the types in the function (`n: number`, `-> bool`): they are checked against the list.

Numbers have methods as well, written with parentheses around a literal: `(9).sqrt()`, `(2.5).round()`, `(7).min(2)`, `(5).clamp(1, 3)`, `(2).pow(10)`, `(5).to_string()`.

## Exercises

1. Make a list of the squares of 1 to 10 using `range` and `push`, then print the sum.
2. Given `let words = ["pear", "fig", "apple"]`, print them joined with commas, in alphabetical order.
3. Write a function `count_words(text)` that returns how many words a sentence has, splitting on a space.

### Solutions

```js
let squares = []
for n in range(1, 11) {
    push(squares, n * n)
}
print(squares.sum())
```

```js
let words = ["pear", "fig", "apple"]
print(words.sorted().join(","))
```

```js
fn count_words(text) {
    return text.trim().split(" ").len()
}

print(count_words("the quick brown fox"))
```

The first prints `385`, the second `apple,fig,pear` and the third `4`.
