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
