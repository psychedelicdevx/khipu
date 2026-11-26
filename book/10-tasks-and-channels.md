# 10. Tasks and channels

Khipu runs lightweight tasks on every core of your machine. Tasks never share values. They talk through channels, and everything sent over a channel is copied, so two tasks cannot race on the same data.

## spawn, channel, send, receive

`channel()` makes a channel. `spawn(f, args...)` starts a task running `f` with those arguments. `send(ch, value)` puts a value on a channel, and `receive(ch)` waits for one:

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

```text
385
```

Ten tasks run `square` in whatever order the cores get to them. The main program waits at `receive` until ten answers have arrived. The sum does not depend on the order, but the order of arrival does, so do not rely on it.

Tasks are cheap: fifty thousand can be waiting at once in about 100 MB. A long-running task gives the others a turn, in loops and in deep recursion alike.

## Values are copied

A task receives its own copy of the values it is given, and a value sent over a channel is copied too. A task changing a list cannot affect the original:

```js
let log = []
let done = channel()

fn work() {
    push(log, "from the task")
    send(done, log)
}

spawn(work)
let seen = receive(done)
print(seen, log)
```

```text
["from the task"] []
```

The task's `log` and the main program's `log` are different lists. If you want an answer back, send it.

## Workers and a job queue

A common shape is a few workers all taking jobs from one channel and answering on another:

```js
let jobs = channel()
let results = channel()

fn worker() {
    while true {
        let n = receive(jobs)
        send(results, n * n)
    }
}

for _ in range(3) {
    spawn(worker)
}

for n in range(1, 7) {
    send(jobs, n)
}

let answers = []
for _ in range(6) {
    push(answers, receive(results))
}
print(answers.sorted())
```

```text
[1, 4, 9, 16, 25, 36]
```

The program ends when the main program ends, even though the workers are still waiting for jobs. The answers come back in any order, so the program sorts them before printing.

## select

`select` waits on several channels at once and runs the arm of whichever has a value first. An arm can bind the value (`name = receive(ch) => ...`) or ignore it:

```js
let numbers = channel()
let words = channel()

fn speak() {
    send(words, "hello")
}

spawn(speak)

select {
    n = receive(numbers) => print("a number", n)
    w = receive(words) => print("a word", w)
}
```

```text
a word hello
```

Here only `words` ever receives something, so that is the arm that runs. A worker that wants to be told when to stop uses a second channel for it:

```js
let jobs = channel()
let quit = channel()
let finished = channel()

fn worker() {
    while true {
        select {
            job = receive(jobs) => print("working on", job)
            receive(quit) => {
                send(finished, true)
                return
            }
        }
    }
}

spawn(worker)
send(jobs, "the report")
send(quit, true)
receive(finished)
print("worker stopped")
```

```text
working on the report
worker stopped
```

## Deadlock detection

If every task is waiting on a channel that nothing will ever send to, a JavaScript program just hangs. Khipu stops and points at the line that waits:

```text
error: deadlock: every task is waiting on a channel that will never get a value
 --> t.kp:4
  |
4 | let y = receive(ch)
  | ^^^^^^^^^^^^^^^^^^^
```

That was a program that sent one value on `ch` but received two. The error means no task could ever make progress, which almost always means a `send` is missing or a `receive` is one too many.

## Tasks run bytecode

Functions that work only with numbers can be compiled to machine code (chapter 14), but spawned tasks keep running ordinary bytecode, so they can pause and take turns with each other. Tasks are about doing several things at once, not about making a single numeric function faster.

## Exercises

1. Spawn three tasks that each send their own number (1, 2, 3) to a channel, receive all three in the main program, and print the sum.
2. Why does the main program in the "values are copied" sample print an empty `log` at the end?
3. Write a program with two channels where only one is ever sent to, and use `select` to receive from whichever one has a value.

### Solutions

```js
let ch = channel()

fn announce(n: number) {
    send(ch, n)
}

for n in range(1, 4) {
    spawn(announce, n)
}

let sum = 0
for _ in range(3) {
    sum = sum + receive(ch)
}
print(sum)
```

The task got its own copy of `log`; its `push` changed that copy, and only the copy was sent back.

```js
let left = channel()
let right = channel()

fn speak() {
    send(right, "right side")
}

spawn(speak)

select {
    v = receive(left) => print("left", v)
    v = receive(right) => print("right", v)
}
```
