# 8. Modules

A program can be split across files. An import whose path ends in `.kp` is a file next to the one doing the importing. (An import without `.kp` is a package, covered in chapter 12.)

Here is a small module. Its code is also a runnable sample, written to `geometry.kp` by the checker that runs this book, and the next samples import it:

```js geometry.kp
let pi = 3.14159

fn circle_area(radius: number) -> number {
    return pi * radius * radius
}

fn rectangle_area(width: number, height: number) -> number {
    return width * height
}
```

## Import a whole file

`import name from "file.kp"` gives you a record holding everything the file defined at its top level, its functions and variables. Use it with a dot:

```js
import geometry from "geometry.kp"

print("circle:", geometry.circle_area(2))
print("rectangle:", geometry.rectangle_area(3, 4))
print(geometry.pi)
```

```text
circle: 12.56636
rectangle: 12
3.14159
```

## Import some names

`import { a, b } from "file.kp"` brings in just those names, as if you had defined them yourself:

```js
import { circle_area, rectangle_area } from "geometry.kp"

print(circle_area(1))
print(rectangle_area(2, 5))
```

```text
3.14159
10
```

Asking for a name the file does not have is an error naming the file and the name, and a file that is not there is `cannot find missing.kp`.

## How loading works

A module's code runs once, the first time anything imports it, and every later import of it gets the same module. That means top-level `print` calls in a module run once, and a module's variables are shared between everyone who imports it. Types are checked across the boundary: calling `circle_area("x")` is rejected, like any other call with a typed parameter.

Paths are relative to the file that contains the import, so a project can keep files in folders: `import util from "lib/util.kp"`.

## Exercises

1. Write a module `text.kp` with a function `shout(word: string) -> string` that upper-cases a word and adds `!`, and import it by name.
2. Why would a module that prints at the top level print only once even if three files import it?
3. Import a module whole and call one of its functions with the dot syntax.

### Solutions

```js text.kp
fn shout(word: string) -> string {
    return word.upper() + "!"
}
```

```js
import { shout } from "text.kp"

print(shout("hey"))
```

Because a module is loaded once and then reused. For the third:

```js
import text from "text.kp"

print(text.shout("whole module"))
```
