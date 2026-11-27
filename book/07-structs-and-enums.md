# 7. Structs and enums

## Structs

A struct is a record with a name, fixed typed fields, and methods. Field declarations end in commas; methods go inside the same braces:

```js
struct Point {
    x: number,
    y: number,

    fn plus(self, other: Point) -> Point {
        return Point { x: self.x + other.x, y: self.y + other.y }
    }

    fn length_squared(self) -> number {
        return self.x * self.x + self.y * self.y
    }
}

let a = Point { x: 1, y: 2 }
let b = Point { x: 3, y: 4 }
print(a.plus(b))
print(b.length_squared())
print(a.x)
```

```text
Point { x: 4, y: 6 }
25
1
```

A method that takes `self` as its first parameter is called on a value with a dot. Inside it, `self.x` reads the value's own field and `self.x = ...` changes it.

Creating a struct with a missing field (`Point { x: 1 }`) is an error: `Point is missing field 'y'`. The exception is a field whose type ends in `?`: it may be left out, and is then `none`, so `struct Note { text: string, tag: string? }` can be written `Note { text: "hi" }`. So is a wrong type (`'Point.x' should be number, got string`) or reading a field that does not exist (`Point has no field 'z'`). Structs, like lists and records, are shared when assigned, and compared by contents:

```js
struct Account {
    owner: string,
    balance: number,

    fn deposit(self, amount: number) {
        self.balance = self.balance + amount
    }
}

let mine = Account { owner: "Ada", balance: 0 }
let same = mine
same.deposit(50)
print(mine.balance)
print(Account { owner: "x", balance: 1 } == Account { owner: "x", balance: 1 })
```

```text
50
true
```

## Static methods

A method without `self` belongs to the struct itself and is called on its name. It is the usual way to write a constructor:

```js
struct Counter {
    count: number,

    fn new() -> Counter {
        return Counter { count: 0 }
    }

    fn bump(self) -> number {
        self.count = self.count + 1
        return self.count
    }
}

let c = Counter.new()
c.bump()
print(c.bump())
```

```text
2
```

## Enums

An enum is a value that is one of a fixed set of variants. A variant can carry values, listed in parentheses with their types in the declaration:

```js
enum Shape {
    Circle(number),
    Rect(number, number),
    Empty,
}

let shapes = [Shape.Circle(1), Shape.Rect(3, 4), Shape.Empty]
print(shapes)
print(Shape.Rect(3, 4) == Shape.Rect(3, 4))
```

```text
[Shape.Circle(1), Shape.Rect(3, 4), Shape.Empty]
true
```

Calling a variant with the wrong number of values is an error: `Shape.Circle expects 1 argument, got 2`.

## match

`match` takes apart an enum. Each arm names a variant, binds the values it carries to names, and gives a result:

```js
enum Shape {
    Circle(number),
    Rect(number, number),
    Empty,
}

fn area(shape: Shape) -> number {
    return match shape {
        Shape.Circle(r) => 3.14159 * r * r,
        Shape.Rect(w, h) => w * h,
        Shape.Empty => 0,
    }
}

print(area(Shape.Circle(1)), area(Shape.Rect(3, 4)), area(Shape.Empty))
```

```text
3.14159 12 0
```

The checker insists that every variant is covered. Leave one out and the program does not run:

```text
error: match on Shape is missing Shape.Empty
 --> shapes.kp:10
   |
10 |     return match shape {
   |     ^^^^^^^^^^^^^^^^^^^^
```

So when you add a variant to an enum later, the checker lists every `match` that needs a new arm. If an arm should catch everything left over, use `_`, which also works to ignore a value a variant carries:

```js
enum Shape {
    Circle(number),
    Rect(number, number),
    Empty,
}

let shape = Shape.Rect(2, 3)
let label = match shape {
    Shape.Circle(_) => "round",
    _ => "not round",
}
print(label)
```

```text
not round
```

### Where match can go

`match` can be a statement, or it can follow `let`, `=` or `return`. It cannot sit inside a bigger expression: `1 + match ...` or `print(match ...)` is an error (`match can only be used as a statement, or right after let, = or return`). Assign the result to a variable first.

As a statement, arms run code, so they use braces, or a single expression, and need no commas:

```js
enum Command {
    Go(number),
    Say(string),
    Stop,
}

fn run(command: Command) {
    match command {
        Command.Go(steps) => print("walking", steps)
        Command.Say(words) => {
            print("saying", words)
        }
        Command.Stop => print("stopping")
    }
}

run(Command.Go(3))
run(Command.Say("hello"))
run(Command.Stop)
```

```text
walking 3
saying hello
stopping
```

`match` also works on numbers and strings, with `_` for the rest:

```js
let code = 404

match code {
    200 => print("ok")
    404 => print("not found")
    _ => print("something else")
}
```

```text
not found
```

## Exercises

1. Write a struct `Rect` with `width` and `height` and a method `area(self)`. Print the area of a 3 by 4 one.
2. Add a static method `Rect.square(side)` that builds a square.
3. Write an enum `Light` with `Red`, `Yellow`, `Green` and a function `next(light: Light) -> Light` using `match`.

### Solutions

```js
struct Rect {
    width: number,
    height: number,

    fn area(self) -> number {
        return self.width * self.height
    }

    fn square(side: number) -> Rect {
        return Rect { width: side, height: side }
    }
}

print(Rect { width: 3, height: 4 }.area())
print(Rect.square(5).area())
```

```js
enum Light {
    Red,
    Yellow,
    Green,
}

fn next(light: Light) -> Light {
    return match light {
        Light.Red => Light.Green,
        Light.Green => Light.Yellow,
        Light.Yellow => Light.Red,
    }
}

print(next(Light.Red))
```
