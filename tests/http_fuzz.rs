use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use khipu::interpreter::Interpreter;
use khipu::lexer::lex;
use khipu::parser::Parser;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

const ATOMS: [&str; 25] = [
    "GET",
    "POST",
    "PUT",
    "DELETE",
    "HTTP/1.1",
    "HTTP/1.0",
    "\r\n",
    "\n",
    "\r",
    " ",
    "\t",
    "Content-Length",
    "content-length",
    "Transfer-Encoding",
    "chunked",
    "identity",
    ":",
    "0",
    "999999999999999999999",
    "-1",
    "+1",
    "1x",
    "\x00",
    "\u{ff}",
    "\r\n\r\n",
];

const SEEDS: [&[u8]; 6] = [
    b"GET / HTTP/1.1\r\nHost: x\r\n\r\n",
    b"POST / HTTP/1.1\r\nContent-Length: 3\r\n\r\nabc",
    b"GET /a?x=1 HTTP/1.1\r\nHost: x\r\nX: y\r\n\r\n",
    b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n0\r\n\r\n",
    b"\xff\xfeGET / HTTP/1.1\r\nHost: x\r\n\r\n",
    b"GET / HTTP/1.1\r\nContent-Length: 100\r\n\r\nshort",
];

fn mutate(rng: &mut Rng, seed: &[u8]) -> Vec<u8> {
    let mut source = seed.to_vec();
    for _ in 0..1 + rng.below(4) {
        let len = source.len();
        let at = rng.below(len + 1);
        match rng.below(6) {
            0 => {
                let end = (at + rng.below(16)).min(len);
                source.drain(at..end);
            }
            1 => {
                let piece = atom_bytes(rng);
                source.splice(at..at, piece.iter().copied());
            }
            2 => {
                let byte = (rng.next() & 0xff) as u8;
                source.insert(at, byte);
            }
            3 => {
                let end = len.saturating_sub(rng.below(len + 1));
                source.truncate(end);
            }
            4 => {
                let piece = atom_bytes(rng);
                let at = rng.below(source.len() + 1);
                source.splice(at..at, piece.iter().copied());
            }
            _ => {
                source.push(b'\n');
            }
        }
    }
    source
}

fn atom_bytes(rng: &mut Rng) -> Vec<u8> {
    ATOMS[rng.below(ATOMS.len())].as_bytes().to_vec()
}

fn send_raw(port: u16, raw: &[u8]) {
    if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
        let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
        let _ = stream.set_write_timeout(Some(Duration::from_millis(200)));
        let _ = stream.write_all(raw);
        let _ = stream.shutdown(std::net::Shutdown::Write);
        let mut buf = [0u8; 1024];
        let _ = stream.read(&mut buf);
    }
}

#[test]
fn malformed_requests_never_crash_or_hang_the_server() {
    let port: u16 = 21000 + (std::process::id() % 20000) as u16;
    let source = format!(
        "fn handle(request: Request) -> Response {{ return Response.text(\"ok\") }}\nHttp.serve_with({port}, handle, {{ read_timeout: 1, request_timeout: 1 }})"
    );
    std::thread::spawn(move || {
        let program = Parser::new(lex(&source).unwrap()).parse_program().unwrap();
        let _ = Interpreter::new().run_main(&program);
    });
    for _ in 0..100 {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    let mut rng = Rng(0x9e3779b97f4a7c15);
    for _ in 0..3000 {
        let seed = SEEDS[rng.below(SEEDS.len())];
        let raw = mutate(&mut rng, seed);
        send_raw(port, &raw);
    }

    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n")
        .unwrap();
    let mut out = Vec::new();
    stream.read_to_end(&mut out).unwrap();
    let text = String::from_utf8_lossy(&out);
    assert!(text.starts_with("HTTP/1.1 200 OK"), "{text}");
    assert!(text.ends_with("ok"), "{text}");
}

const HANDSHAKE: &[u8] = b"GET / HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n";

const ECHO: &str = "fn chat(socket: channel) {\n    let message = receive(socket)\n    while message != none {\n        send(socket, \"echo: \" + message)\n        message = receive(socket)\n    }\n}\nfn handle(request: Request) -> Response {\n    return Response.websocket(chat)\n}";

fn masked(first: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![first];
    match payload.len() {
        len @ 0..=125 => out.push(0x80 | len as u8),
        len @ 126..=0xffff => {
            out.push(0x80 | 126);
            out.extend_from_slice(&(len as u16).to_be_bytes());
        }
        len => {
            out.push(0x80 | 127);
            out.extend_from_slice(&(len as u64).to_be_bytes());
        }
    }
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(payload);
    out
}

fn frames(rng: &mut Rng) -> Vec<u8> {
    let mut out = Vec::new();
    for _ in 0..1 + rng.below(4) {
        let first = [0x81, 0x82, 0x01, 0x80, 0x00, 0x88, 0x89, 0x8a, 0x83, 0xc1][rng.below(10)];
        let len = [0, 1, 2, 125, 126, 300, 70000][rng.below(7)];
        out.extend(masked(first, &vec![b'a' + rng.below(26) as u8; len]));
    }
    for _ in 0..rng.below(4) {
        if out.is_empty() {
            break;
        }
        let len = out.len();
        let at = rng.below(len);
        match rng.below(4) {
            0 => out[at] = (rng.next() & 0xff) as u8,
            1 => out.truncate(at),
            2 => out.insert(at, (rng.next() & 0xff) as u8),
            _ => out
                .splice(at..at, [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff])
                .for_each(drop),
        }
    }
    out
}

fn upgraded(port: u16) -> TcpStream {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream.write_all(HANDSHAKE).unwrap();
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap() == 1 {
        head.push(byte[0]);
    }
    assert!(
        head.starts_with(b"HTTP/1.1 101 "),
        "{}",
        String::from_utf8_lossy(&head)
    );
    stream
}

#[test]
fn random_frames_after_an_upgrade_never_crash_or_hang_the_server() {
    let port: u16 = 21000 + (std::process::id() % 20000) as u16 + 1;
    let source = format!(
        "{ECHO}\nHttp.serve_with({port}, handle, {{ read_timeout: 1, idle_timeout: 0.1, max_body: 100000 }})"
    );
    std::thread::spawn(move || {
        let program = Parser::new(lex(&source).unwrap()).parse_program().unwrap();
        let _ = Interpreter::new().run_main(&program);
    });
    for _ in 0..100 {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    let mut rng = Rng(0x2545f4914f6cdd1d);
    for _ in 0..1000 {
        let mut raw = HANDSHAKE.to_vec();
        raw.extend(frames(&mut rng));
        send_raw(port, &raw);
    }

    let mut stream = upgraded(port);
    stream.write_all(&masked(0x81, b"still here")).unwrap();
    let mut echo = [0u8; 18];
    stream.read_exact(&mut echo).unwrap();
    assert_eq!(&echo, b"\x81\x10echo: still here");
}

fn khipu_errors(source: &str, port: u16, cases: impl FnOnce()) -> String {
    let path = std::env::temp_dir().join(format!("khipu-websocket-{port}.kp"));
    std::fs::write(&path, source).unwrap();
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_khipu"))
        .arg(&path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    for _ in 0..200 {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    cases();
    std::thread::sleep(Duration::from_millis(200));
    let _ = child.kill();
    let output = child.wait_with_output().unwrap();
    let _ = std::fs::remove_file(&path);
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn points_at(stderr: &str, message: &str, line: usize) {
    let at = stderr
        .find(&format!("error: {message}\n"))
        .unwrap_or_else(|| panic!("no '{message}' in {stderr}"));
    let next = stderr[at..].lines().nth(1).unwrap_or("");
    assert!(next.ends_with(&format!(".kp:{line}")), "{next}");
}

fn read_rest(stream: &mut TcpStream) -> Vec<u8> {
    let mut out = Vec::new();
    let _ = stream.read_to_end(&mut out);
    out
}

#[test]
fn websocket_errors_name_the_problem_and_the_line() {
    let port: u16 = 21000 + (std::process::id() % 20000) as u16 + 2;
    let source = format!(
        "fn chat(socket: channel) {{\n    let first = receive(socket)\n    if first == \"number\" {{\n        send(socket, 42)\n    }}\n    if first == \"code\" {{\n        close(socket, 5000, \"no\")\n    }}\n    if first == \"plain\" {{\n        close(channel(), 1000)\n    }}\n    if first == \"late\" {{\n        receive(socket)\n        send(socket, \"too late\")\n    }}\n}}\nfn handle(request: Request) -> Response {{\n    return Response.websocket(chat)\n}}\nHttp.serve({port}, handle)\n"
    );
    let stderr = khipu_errors(&source, port, || {
        for word in ["number", "code", "plain"] {
            let mut stream = upgraded(port);
            stream.write_all(&masked(0x81, word.as_bytes())).unwrap();
            assert_eq!(read_rest(&mut stream), [0x88, 0x02, 0x03, 0xf3]);
        }
        let mut stream = upgraded(port);
        stream.write_all(&masked(0x81, b"late")).unwrap();
        stream.write_all(&masked(0x88, &[0x03, 0xe8])).unwrap();
        assert_eq!(read_rest(&mut stream), [0x88, 0x02, 0x03, 0xe8]);
    });
    points_at(
        &stderr,
        "a websocket can only send a string or bytes, got number",
        4,
    );
    points_at(
        &stderr,
        "a close code must be 1000 to 1003, 1007 to 1011 or 3000 to 4999, got 5000",
        7,
    );
    points_at(&stderr, "close on a channel takes no code or reason", 10);
    points_at(&stderr, "cannot send on a closed websocket", 14);
}
