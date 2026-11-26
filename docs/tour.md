# A tour of Khipu

The short reference: what the language does, with examples that `scripts/check-docs.py` runs as written. For a gentler path, read [The Khipu Book](../book/README.md).

## Contracts

A function can say what it needs, what it promises, and show examples. Khipu checks `requires` on every call and `ensures` on every return, and `khipu test` runs every `example`:

```js
fn divide(a: number, b: number) -> number
    requires b != 0
    ensures result * b == a
    example divide(10, 2) == 5
{
    return a / b
}
```

A broken promise names the rule, the values and the line that broke it:

```
error: divide requires b != 0 (a = 3, b = 0)
 --> shop.kp:12
   |
12 | print(divide(total, count))
   |       ^^^^^^^^^^^^^^^^^^^^
```

Contracts make intent part of the code, so a person or an AI changing a function finds out at once when the change breaks what callers rely on.

## Standard library

Strings, lists and numbers have methods, and every one is type-checked, including the ones that take a function:

```js
let words = "  the quick brown fox  ".trim().split(" ")
let long = words.filter(fn(w: string) -> bool { return w.len() > 3 })
print(long.map(fn(w: string) -> string { return w.upper() }).join(", "))
print([5, 3, 9].sorted(), [1, 2, 3].sum(), (2).pow(10), Math.sqrt(16))
```

| On | Methods |
|---|---|
| strings | `len` `is_empty` `upper` `lower` `trim` `split` `lines` `chars` `contains` `starts_with` `ends_with` `replace` `index_of` `slice` `repeat` `to_number` |
| lists | `len` `is_empty` `push` `pop` `insert` `remove` `first` `last` `contains` `index_of` `slice` `concat` `reversed` `sorted` `sort_by` `join` `map` `filter` `reduce` `find` `any` `all` `count`, and `sum` `min` `max` on numbers |
| numbers | `sqrt` `floor` `ceil` `round` `abs` `pow` `min` `max` `clamp` `to_string` |
| `Math` | `pi` `e` `sqrt` `pow` `abs` `floor` `ceil` `round` `min` `max` `sin` `cos` `tan` `atan2` `log` `exp` `random` `random_int` |

Files and the terminal:

```js
File.write("notes.txt", "first line\n")
File.append("notes.txt", "second line\n")
for line in File.lines("notes.txt") {
    print(line.upper())
}
let name = input("Your name? ") ?? "stranger"
print("hello", name, "you passed", Process.args())
```

The web, with JSON built in:

```js
fn handle(request: Request) -> Response {
    if request.path == "/hello" {
        return Response.from_json({ message: "hello", path: request.path })
    }
    return Response.error(404, "nothing here")
}

Http.serve(8080, handle)
```

```js
let reply = Http.get("http://localhost:8080/hello")
print(reply.status, reply.json().message)
```

`Http` also has `post`, `post_json` and `request` for any method and headers, over `http://` or `https://`. `Http.serve` answers one request at a time on `localhost`; see [`examples/web`](examples/web). `Json.parse` and `Json.stringify` turn text into records, lists, numbers and back.

`File` also has `read`, `exists`, `list` and `delete`; `Process` has `args`, `env` and `exit`. A file that cannot be read stops the program with an error that says why, on the line that tried.

`range(n)` counts on demand instead of building a list, and a list of nothing but numbers or bools is stored compactly. Lookups that can miss give back `none` instead of a magic `-1`: `"abc".index_of("z")`, `[].first()` and `"x".to_number()` all return `none`, so the type checker makes you handle it with `??`. Methods that take a callback are written in Khipu itself, in [`src/prelude.kp`](src/prelude.kp).

## Tasks on every core

`spawn` starts a task. Tasks are cheap: fifty thousand can wait at once in about 100 MB. They run on every core and talk only through channels: whatever you `send` is copied, so two tasks never share a value and data races can't happen. A task that is waiting to start goes to whichever core is free, and a long-running task gives the others a turn, in loops and in deep recursion alike.

```js
let results = channel()

fn square(n: number) {
    send(results, n * n)
}

for n in range(1, 11) {
    spawn(square, n)
}

let total = 0
for _ in range(10) {
    total = total + receive(results)
}
print(total)
```

`select` waits on several channels and runs the arm for whichever gets a value first:

```js
let jobs = channel()
let quit = channel()

fn worker() {
    while true {
        select {
            job = receive(jobs) => print("working on", job)
            receive(quit) => return
        }
    }
}

spawn(worker)
send(jobs, "the report")
send(quit, true)
```

If every task ends up waiting on a channel that nothing will ever send to, Khipu stops with a deadlock error that points at the line, instead of hanging.

## Strict where JavaScript is loose

| JavaScript | Khipu |
|---|---|
| `if (1) { ... }` runs, because `1` is "truthy" | `if 1 { ... }` is an error: *if needs true or false, got number* |
| `1 + true` is `2` | `1 + true` is an error: *cannot use '+' on number and bool* |
| `1 == true` is `true` | `1 == true` is an error: you can't compare different types |
| A typo'd variable is `undefined` | `print(totl)` is an error: *'totl' is not defined* |
| `x = 5` without declaring creates a global | `x = 5` is an error until you write `let x = ...` |
| `let count = 0` then `count = "zero"` is fine | A type error, *'count' should be number, got string*, caught before the program runs |
| `user.name` crashes at runtime when `user` is `null` | `find_user()` returning `record?` must be handled with `?.` or `??` first |

## Packages

Packages come straight from git, with no central registry. A project is a folder with a `khipu.json`:

```bash
khipu init                                                  # creates khipu.json and main.kp
khipu add colors https://github.com/someone/colors.git#v1.0 # a tag, branch or commit after #
khipu add util path:../util                                 # or a folder next to yours
khipu run                                                   # runs the main file
```

```js
import colors from "colors"
print(colors.red("hello"))
```

An import without `.kp` is a package; with `.kp` it is a file next to yours. `khipu add` records the exact commit in `khipu.lock`, so `khipu install` gives everyone the same code; packages' own dependencies are installed too, but only from `https://`, `ssh://` or `git@` sources, so a package can never pull in a folder from your machine. Installing never runs a package's code, and package imports only look inside your own project.
