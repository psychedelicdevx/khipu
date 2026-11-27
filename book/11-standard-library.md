# 11. The standard library tour

Chapter 5 covered the methods on strings, lists and numbers. This chapter covers the named groups of functions: `Math`, `File`, `Process`, `input`, `Json` and `Http`. Most of the standard library is written in Khipu itself, in `src/prelude.kp`, with fast Rust underneath.

## Math

`Math` has numeric functions. `Math.pi()` and `Math.e()` are functions, so they need parentheses:

```js
print(Math.sqrt(16), Math.pow(2, 8), Math.abs(-3))
print(Math.floor(-1.5), Math.ceil(1.2), Math.round(2.5))
print(Math.min(3, 2), Math.max(3, 2))
print(Math.pi(), Math.e())
print(Math.sin(0), Math.cos(0), Math.tan(0), Math.atan2(0, 1))
print(Math.log(Math.e()), Math.exp(0))
```

```text
4 256 3
-2 2 3
2 3
3.141592653589793 2.718281828459045
0 1 0 0
1 1
```

`Math.random()` gives a number from 0 up to but not including 1. `Math.random_int(low, high)` gives a whole number between the two, both included:

```js
let roll = Math.random_int(1, 6)
print(roll >= 1 && roll <= 6)
let r = Math.random()
print(r >= 0 && r < 1)
```

```text
true
true
```

## File

Files are read and written as text. Paths are relative to where you run the program:

```js
File.write("notes.txt", "first line\n")
File.append("notes.txt", "second line\n")

print(File.exists("notes.txt"))
print(File.read("notes.txt"))
for line in File.lines("notes.txt") {
    print(line.upper())
}

File.delete("notes.txt")
print(File.exists("notes.txt"))
```

```text
true
first line
second line

FIRST LINE
SECOND LINE
false
```

`File.read_bytes(path)` and `File.write_bytes(path, data)` do the same with `bytes` (described under Http below), for files that are not text. `File.list("folder")` gives the names inside a folder. A file that cannot be read stops the program with an error that says why and points at the line that tried:

```text
error: cannot read nope.txt: it does not exist
 --> t.kp:6
  |
6 | File.read("nope.txt")
  | ^^^^^^^^^^^^^^^^^^^^^
```

## Process and input

`Process.args()` is the list of command-line arguments after the file name, `Process.env(name)` reads an environment variable (or gives `none`), and `Process.exit(code)` ends the program.

`input(prompt)` prints the prompt and reads a line from the terminal. It gives `none` when there is no more input, for instance when the input is closed, so you handle that with `??`:

```js
let name = input("Your name? ") ?? "stranger"
print("hello", name, "you passed", Process.args())
print(Process.env("A_VARIABLE_THAT_IS_NOT_SET"))
```

Run as `khipu greet.kp one two` with nothing typed (the prompt is printed, then the program carries on):

```text
Your name? hello stranger you passed ["one", "two"]
none
```

(With the terminal closed, as in the checker that runs this book, the list is empty: `[]`.) A program that wants to stop early does it with `Process.exit(1)`, and the number is the exit code your shell sees.

## Json

`Json.parse` turns text into lists, records, numbers, strings, bools and `none` (for `null`). Its result has type `any`, so its fields can be read directly. `Json.stringify` goes the other way, writing fields in alphabetical order:

```js
let data = Json.parse("{\"name\": \"Ada\", \"scores\": [3, 5], \"spouse\": null}")
print(data.name, data.scores[1], data.spouse)
print(Json.stringify({ n: 1, tags: ["a", "b"], ok: true }))
```

```text
Ada 5 none
{"n":1,"ok":true,"tags":["a","b"]}
```

## Http

`Http.serve(port, handler)` runs a small web server on `localhost`. The handler is a function from a `Request` to a `Response`. A request has `method`, `path`, `query`, `headers` and `body`, and a `json()` method. `Response` has constructors for the common cases: `Response.text`, `Response.html`, `Response.from_json` and `Response.error(status, message)`:

```js
fn handle(request: Request) -> Response {
    if request.path == "/hello" {
        return Response.from_json({ message: "hello", path: request.path })
    }
    return Response.error(404, "nothing at ${request.path}")
}

Http.serve(8080, handle)
```

`Http.serve` never returns, so a program that serves is stopped with Ctrl-C, which lets requests already being answered finish. Handlers run on every core at once; a connection that is waiting for its next request, or a WebSocket waiting for its next message, waits in one event loop instead of holding a thread, so ten thousand idle connections cost memory, not workers. The calls on the other side are `Http.get(url)`, `Http.post(url, body)`, `Http.post_json(url, value)` and `Http.request(method, url, body, headers)`, over `http://` or `https://`. Each gives a `Response` with `status`, `body`, `headers`, an `ok()` that is true for 2xx statuses, and `json()`. The body is there twice: `body` as text, with invalid UTF-8 replaced, and `bytes` exactly as it arrived, so a download of an image can be written to disk with `File.write_bytes("logo.png", reply.bytes ?? bytes([]))` untouched. (`bytes` is typed `bytes?` because a `Response` the program builds itself may leave it out.)

```js
let reply = Http.get("http://localhost:8080/hello")
print(reply.status, reply.ok(), reply.json().message)

let missing = Http.get("http://localhost:8080/other")
print(missing.status, missing.body)
```

```text
200 true hello
404 nothing at /other
```

Run the server in one terminal and the client in another. `examples/web` in the repository has a bigger pair: a notes server and a client that posts to it.

Bodies are not always text. `bytes` holds binary data: `"text".to_bytes()` makes it from a string and `bytes([104, 105])` from a list of whole numbers 0 to 255, `data[i]` reads one byte as a number, `len()` and `slice(start, end)` count and cut it by byte, and `to_string()` gives the text back, or `none` if the bytes are not valid UTF-8:

```js
let data = "héllo".to_bytes()
print(data.len(), data.slice(0, 3).to_string(), data.slice(0, 2).to_string(), data)
print(data[0], data[1], bytes([104, 105]).to_string())
```

```text
6 hé none <6 bytes>
104 195 hi
```

An index past the end is the same error a list gives, and a number outside 0 to 255, or not whole, is an error that names it and its index.

A request carries its body both ways, as `body` (the text, with invalid UTF-8 replaced) and as `bytes`. A `Response` body can be a string or `bytes`, and `Response.file` reads the file as bytes, so images and archives are served untouched:

```js
fn handle(request: Request) -> Response {
    if request.path == "/logo.png" {
        return Response.file("logo.png")
    }
    return Response { status: 200, body: request.bytes, headers: { "content-type": "application/octet-stream" } }
}

Http.serve(8080, handle)
```

### WebSockets

A handler that sees `request.is_websocket()` can return `Response.websocket(chat)`. The server answers the upgrade and runs `chat(socket)` as a task of its own, where `socket` is a channel: `send` writes a message, a string as text or `bytes` as binary, and `receive` waits for the next one and gives `none` once the other side has gone. `select` waits on sockets and channels together, and `close(socket, code, reason)` ends the conversation with a code and reason of the program's choosing (`close(socket)` sends 1000, a normal close). Returning from `chat` closes with 1000 too, and an error closes with 1011. On a plain channel `close(ch)` takes no code: it ends the channel the way chapter 10 describes, and `receive` gives `none` once the queue is empty, just as it does for a socket whose peer has gone:

```js
let news = channel()

fn chat(socket: channel) {
    while true {
        select {
            message = receive(socket) => {
                if message == none {
                    return
                }
                if message == "bye" {
                    close(socket, 4000, "see you")
                    return
                }
                send(socket, "heard ${message}")
            }
            item = receive(news) => send(socket, "news: ${item}")
        }
    }
}

fn handle(request: Request) -> Response {
    if request.is_websocket() {
        return Response.websocket(chat)
    }
    return Response.text("connect with a WebSocket")
}

Http.serve(8080, handle)
```

A socket waiting for a message costs memory, not a thread, so a server can hold thousands of them.

A client may ask for one of several subprotocols, and the program says which it speaks: `request.protocol(["chat.v2", "chat.v1"])` gives the first of the program's list that the client offered, or `none`, and `Response.websocket_with(chat, { protocol: chosen })` answers with it. When the client offers `permessage-deflate`, as browsers do, the server accepts it and compresses messages of 64 bytes or more; nothing in the program changes:

```js
fn chat(socket: channel, protocol: string?) {
    send(socket, "speaking ${protocol ?? "plain"}")
    let message = receive(socket)
    while message != none {
        send(socket, "echo: ${message}")
        message = receive(socket)
    }
}

fn handle(request: Request) -> Response {
    let protocol = request.protocol(["chat.v2", "chat.v1"])
    return Response.websocket_with(fn(socket: channel) {
        chat(socket, protocol)
    }, { protocol: protocol })
}

Http.serve(8080, handle)
```

`Http.websocket(url)` is the other side: it connects to a `ws://` or `wss://` address and gives a socket that works the same way, with `send`, `receive`, `select` and `close`. `Http.websocket_with(url, options)` can list `protocols` to offer and add a `certificate` file to trust, for a server whose certificate is self-signed. `socket.protocol()` gives the protocol the server chose, or `none`, and works on the server's side of a socket too:

```js
let socket = Http.websocket_with("ws://localhost:8080/", { protocols: ["chat.v1"] })
print(socket.protocol())
print(receive(socket))
send(socket, "hello")
print(receive(socket))
close(socket)
```

```text
chat.v1
speaking chat.v1
echo: hello
```

The client offers `permessage-deflate` and compresses messages of 64 bytes or more when the server accepts it, as Khipu's server does. It also watches the connection: after 30 seconds without hearing anything it sends a ping, and if 30 more pass with no answer, the socket is closed and the `receive` (or `select`) waiting on it stops the program with an error saying the server timed out, so a socket to a machine that has vanished does not wait forever. `ping_interval` and `ping_timeout` in the options change the two periods, in seconds: `Http.websocket_with(url, { ping_interval: 5, ping_timeout: 2 })`.

## Exercises

1. Print a random whole number from 1 to 100.
2. Write three lines to a file, read them back with `File.lines`, print how many there are, and delete the file.
3. Parse `[1, 2, 3]` with `Json.parse` and print the sum of the items. (The result is `any`, so use `.sum()` on it.)

### Solutions

```js
print(Math.random_int(1, 100))
```

```js
File.write("three.txt", "a\nb\nc\n")
let lines = File.lines("three.txt")
print(lines.len())
File.delete("three.txt")
```

```js
let items = Json.parse("[1, 2, 3]")
print(items.sum())
```
