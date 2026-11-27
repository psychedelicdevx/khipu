use std::rc::Rc;

use serde_json::Value as Json;

use crate::value::{Fields, Native, Value};

pub static NATIVES: [Native; 10] = [
    Native {
        name: "serve#",
        function: serve,
    },
    Native {
        name: "emit#",
        function: emit_chunk,
    },
    Native {
        name: "serve_tls#",
        function: serve_tls,
    },
    Native {
        name: "fetch#",
        function: fetch,
    },
    Native {
        name: "listen#",
        function: listen,
    },
    Native {
        name: "accept#",
        function: accept,
    },
    Native {
        name: "respond#",
        function: respond,
    },
    Native {
        name: "websocket_request#",
        function: websocket_request,
    },
    Native {
        name: "json_parse#",
        function: json_parse,
    },
    Native {
        name: "json_stringify#",
        function: json_stringify,
    },
];

fn text(value: &Value, what: &str) -> Result<String, String> {
    match value {
        Value::Str(text) => Ok(text.to_string()),
        other => Err(format!("{what} needs a string, got {}", other.type_name())),
    }
}

fn record(pairs: Vec<(String, Value)>) -> Value {
    let fields: Fields = pairs
        .into_iter()
        .map(|(name, value)| (Rc::from(name.as_str()), value))
        .collect();
    Value::record(fields)
}

#[cfg(not(target_arch = "wasm32"))]
fn headers(value: &Value) -> Vec<(String, String)> {
    match value {
        Value::Record(record) => record
            .borrow()
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect(),
        _ => Vec::new(),
    }
}

fn to_json(value: &Value) -> Result<Json, String> {
    let Some(_guard) = crate::value::Nesting::enter() else {
        return Err(format!(
            "cannot write values nested more than {} levels deep as JSON, does one contain itself?",
            crate::value::MAX_DEPTH
        ));
    };
    Ok(match value {
        Value::None => Json::Null,
        Value::Bool(b) => Json::Bool(*b),
        Value::Number(n) if n.fract() == 0.0 && n.abs() < 9e15 => Json::from(*n as i64),
        Value::Number(n) => serde_json::Number::from_f64(*n)
            .map(Json::Number)
            .ok_or_else(|| format!("{n} cannot be written as JSON"))?,
        Value::Str(text) => Json::String(text.to_string()),
        Value::List(items) => Json::Array(
            items
                .borrow()
                .iter()
                .map(|item| to_json(&item))
                .collect::<Result<_, _>>()?,
        ),
        Value::Record(fields) => Json::Object(
            fields
                .borrow()
                .iter()
                .map(|(name, value)| Ok((name.to_string(), to_json(value)?)))
                .collect::<Result<_, String>>()?,
        ),
        Value::Instance(instance) => Json::Object(
            instance
                .def
                .fields
                .iter()
                .zip(instance.fields.borrow().iter())
                .map(|(name, value)| Ok((name.to_string(), to_json(value)?)))
                .collect::<Result<_, String>>()?,
        ),
        other => return Err(format!("a {} cannot be written as JSON", other.type_name())),
    })
}

fn from_json(json: Json) -> Value {
    match json {
        Json::Null => Value::None,
        Json::Bool(b) => Value::Bool(b),
        Json::Number(n) => Value::Number(n.as_f64().unwrap_or(f64::NAN)),
        Json::String(text) => Value::str(&text),
        Json::Array(items) => Value::list(items.into_iter().map(from_json).collect()),
        Json::Object(fields) => record(
            fields
                .into_iter()
                .map(|(name, value)| (name, from_json(value)))
                .collect(),
        ),
    }
}

fn json_parse(args: &[Value]) -> Result<Value, String> {
    let source = text(&args[0], "Json.parse")?;
    serde_json::from_str(&source)
        .map(from_json)
        .map_err(|error| format!("invalid JSON: {error}"))
}

fn json_stringify(args: &[Value]) -> Result<Value, String> {
    Ok(Value::str(&to_json(&args[0])?.to_string()))
}

#[cfg(target_arch = "wasm32")]
fn unavailable(_: &[Value]) -> Result<Value, String> {
    Err("HTTP is not available in the playground".to_string())
}

#[cfg(target_arch = "wasm32")]
use unavailable as fetch;
#[cfg(target_arch = "wasm32")]
use unavailable as listen;
#[cfg(target_arch = "wasm32")]
use unavailable as accept;
#[cfg(target_arch = "wasm32")]
use unavailable as respond;
#[cfg(target_arch = "wasm32")]
use unavailable as serve;
#[cfg(target_arch = "wasm32")]
use unavailable as emit_chunk;
#[cfg(target_arch = "wasm32")]
use unavailable as serve_tls;
#[cfg(target_arch = "wasm32")]
use unavailable as websocket_request;

#[cfg(not(target_arch = "wasm32"))]
pub use server::{
    accept, active_count, begin_shutdown, drain, emit_chunk, fetch, is_shutdown, listen,
    reset_shutdown, respond, serve, serve_tls, websocket_request,
};

#[cfg(not(target_arch = "wasm32"))]
mod server {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::os::fd::{AsRawFd, RawFd};
    use std::rc::Rc;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::mpsc::{Receiver, SyncSender};
    use std::sync::{Arc, Mutex, Weak};
    use std::time::{Duration, Instant};

    use super::{headers, record, text};
    use crate::task::Channel;
    use crate::value::{GlobalStore, Instance, Native, StructDef, Value};
    use crate::vm::Vm;

    const MAX_BODY: usize = 10 * 1024 * 1024;
    const MAX_REQUEST_HEAD: usize = 16 * 1024;
    const MAX_HEADER_LINE: usize = 8 * 1024;
    const MAX_HEADERS: usize = 100;
    const READ_TIMEOUT: Duration = Duration::from_secs(10);
    const IDLE_TIMEOUT: Duration = Duration::from_secs(5);
    const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

    #[derive(Clone, Copy)]
    struct Limits {
        max_body: usize,
        max_request_head: usize,
        max_header_line: usize,
        max_headers: usize,
        max_requests: usize,
        read_timeout: Duration,
        idle_timeout: Duration,
        request_timeout: Duration,
    }

    impl Default for Limits {
        fn default() -> Self {
            Limits {
                max_body: MAX_BODY,
                max_request_head: MAX_REQUEST_HEAD,
                max_header_line: MAX_HEADER_LINE,
                max_headers: MAX_HEADERS,
                max_requests: usize::MAX,
                read_timeout: READ_TIMEOUT,
                idle_timeout: IDLE_TIMEOUT,
                request_timeout: REQUEST_TIMEOUT,
            }
        }
    }

    fn num(record: &crate::value::Record, name: &str, current: usize) -> usize {
        match record.get(name) {
            Some(Value::Number(n)) if n.fract() == 0.0 && n >= 0.0 => n as usize,
            _ => current,
        }
    }

    fn secs(record: &crate::value::Record, name: &str, current: Duration) -> Duration {
        match record.get(name) {
            Some(Value::Number(n)) if n >= 0.0 && n.is_finite() => Duration::from_secs_f64(n),
            _ => current,
        }
    }

    fn limits_from(value: Option<&Value>) -> Limits {
        let base = Limits::default();
        let Some(Value::Record(record)) = value else {
            return base;
        };
        let record = record.borrow();
        let record = &*record;
        let read_timeout = secs(record, "read_timeout", base.read_timeout);
        Limits {
            max_body: num(record, "max_body", base.max_body),
            max_request_head: num(record, "max_request_head", base.max_request_head),
            max_header_line: num(record, "max_header_line", base.max_header_line),
            max_headers: num(record, "max_headers", base.max_headers),
            max_requests: num(record, "max_requests", base.max_requests),
            read_timeout,
            idle_timeout: secs(record, "idle_timeout", base.idle_timeout.min(read_timeout)),
            request_timeout: secs(record, "request_timeout", base.request_timeout),
        }
    }

    static NEXT: AtomicU64 = AtomicU64::new(1);
    static LISTENERS: Mutex<Option<HashMap<u64, TcpListener>>> = Mutex::new(None);
    static CONNECTIONS: Mutex<Option<HashMap<u64, TcpStream>>> = Mutex::new(None);

    fn store<T>(table: &Mutex<Option<HashMap<u64, T>>>, item: T) -> u64 {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        table
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get_or_insert_with(HashMap::new)
            .insert(id, item);
        id
    }

    fn take<T>(table: &Mutex<Option<HashMap<u64, T>>>, id: u64) -> Option<T> {
        table
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_mut()?
            .remove(&id)
    }

    fn id(value: &Value) -> u64 {
        match value {
            Value::Number(n) => *n as u64,
            _ => 0,
        }
    }

    pub fn fetch(args: &[Value]) -> Result<Value, String> {
        let method = text(&args[0], "Http.request")?.to_uppercase();
        let url = text(&args[1], "Http.request")?;
        let body = text(&args[2], "Http.request")?;
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            .build()
            .into();
        let failed = |error: ureq::Error| format!("cannot reach {url}: {error}");
        let with_headers = |mut request: ureq::RequestBuilder<ureq::typestate::WithoutBody>| {
            for (name, value) in headers(&args[3]) {
                request = request.header(name, value);
            }
            request
        };
        let with_body_headers = |mut request: ureq::RequestBuilder<ureq::typestate::WithBody>| {
            for (name, value) in headers(&args[3]) {
                request = request.header(name, value);
            }
            request
        };
        let response = match method.as_str() {
            "GET" => with_headers(agent.get(&url)).call(),
            "DELETE" => with_headers(agent.delete(&url)).call(),
            "HEAD" => with_headers(agent.head(&url)).call(),
            "POST" => with_body_headers(agent.post(&url)).send(body.as_bytes()),
            "PUT" => with_body_headers(agent.put(&url)).send(body.as_bytes()),
            "PATCH" => with_body_headers(agent.patch(&url)).send(body.as_bytes()),
            other => return Err(format!("unknown HTTP method '{other}'")),
        };
        let mut response = response.map_err(failed)?;
        let status = response.status().as_u16();
        let found: Vec<(String, Value)> = response
            .headers()
            .iter()
            .map(|(name, value)| {
                (
                    name.to_string(),
                    Value::str(&String::from_utf8_lossy(value.as_bytes())),
                )
            })
            .collect();
        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|error| format!("cannot read the reply from {url}: {error}"))?;
        Ok(Value::list(vec![
            Value::Number(status as f64),
            Value::str(&body),
            record(found),
        ]))
    }

    pub fn listen(args: &[Value]) -> Result<Value, String> {
        let port = match &args[0] {
            Value::Number(n) if n.fract() == 0.0 && (1.0..=65535.0).contains(n) => *n as u16,
            other => {
                return Err(format!(
                    "Http.serve needs a port from 1 to 65535, got {other}"
                ));
            }
        };
        let listener = TcpListener::bind(("127.0.0.1", port))
            .map_err(|error| format!("cannot listen on port {port}: {error}"))?;
        Ok(Value::Number(store(&LISTENERS, listener) as f64))
    }

    type Incoming = (String, String, Vec<(String, String)>, Vec<u8>);

    const BAD_REQUEST: &[u8] =
        b"HTTP/1.1 400 Bad Request\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";

    fn find_head_end(received: &[u8]) -> Option<(usize, usize)> {
        let find = |needle: &[u8]| received.windows(needle.len()).position(|w| w == needle);
        find(b"\r\n\r\n")
            .map(|at| (at, 4))
            .or_else(|| find(b"\n\n").map(|at| (at, 2)))
    }

    enum Parsed {
        Incomplete,
        Bad,
        Request(Incoming, bool, usize),
    }

    fn parse_request(buf: &[u8], limits: &Limits) -> Parsed {
        let Some((head_len, term_len)) = find_head_end(buf) else {
            return if buf.len() > limits.max_request_head {
                Parsed::Bad
            } else {
                Parsed::Incomplete
            };
        };
        if head_len > limits.max_request_head {
            return Parsed::Bad;
        }
        let Ok(head) = std::str::from_utf8(&buf[..head_len]) else {
            return Parsed::Bad;
        };
        let mut lines = head.split('\n');
        let raw_request_line = lines.next().unwrap_or("");
        let request_line = raw_request_line
            .strip_suffix('\r')
            .unwrap_or(raw_request_line);
        let mut parts = request_line.split_whitespace();
        let (Some(method), Some(target)) = (parts.next(), parts.next()) else {
            return Parsed::Bad;
        };
        let version = parts.next().unwrap_or("HTTP/1.1");
        let mut found = Vec::new();
        let mut length = 0usize;
        let mut seen_length = false;
        let mut count = 0usize;
        let mut connection = String::new();
        for raw in lines {
            let line = raw.strip_suffix('\r').unwrap_or(raw);
            if line.is_empty() {
                continue;
            }
            count += 1;
            if count > limits.max_headers || line.len() > limits.max_header_line {
                return Parsed::Bad;
            }
            let Some((name, value)) = line.split_once(':') else {
                return Parsed::Bad;
            };
            let name = name.trim().to_lowercase();
            let value = value.trim();
            if !valid_header_name(&name) {
                return Parsed::Bad;
            }
            if name == "transfer-encoding" && !value.eq_ignore_ascii_case("identity") {
                return Parsed::Bad;
            }
            if name == "content-length" {
                if seen_length || value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                    return Parsed::Bad;
                }
                seen_length = true;
                length = match value.parse() {
                    Ok(length) if length <= limits.max_body => length,
                    _ => return Parsed::Bad,
                };
            }
            if name == "connection" {
                connection = value.to_lowercase();
            }
            found.push((name, value.to_string()));
        }
        let body_start = head_len + term_len;
        let total = body_start + length;
        if buf.len() < total {
            return Parsed::Incomplete;
        }
        let has = |token: &str| connection.split(',').any(|c| c.trim() == token);
        let keep_alive = if has("close") {
            false
        } else {
            has("keep-alive") || version != "HTTP/1.0"
        };
        let body = buf[body_start..total].to_vec();
        Parsed::Request(
            (method.to_string(), target.to_string(), found, body),
            keep_alive,
            total,
        )
    }

    fn read_request(mut stream: &TcpStream) -> Option<Incoming> {
        stream.set_read_timeout(Some(READ_TIMEOUT)).ok()?;
        let deadline = Instant::now() + REQUEST_TIMEOUT;
        let limits = Limits::default();
        let mut buf = [0u8; 4096];
        let mut received = Vec::new();
        loop {
            match parse_request(&received, &limits) {
                Parsed::Request(incoming, _, _) => return Some(incoming),
                Parsed::Bad => return None,
                Parsed::Incomplete if Instant::now() > deadline => return None,
                Parsed::Incomplete => {}
            }
            let read = stream.read(&mut buf).ok()?;
            if read == 0 {
                return None;
            }
            received.extend_from_slice(&buf[..read]);
        }
    }

    trait Stream: Read + Write + Send {
        fn tcp(&self) -> &TcpStream;
        fn close(&mut self);
        fn impatient(&mut self);
    }

    struct Patient {
        tcp: TcpStream,
        wait: Option<Duration>,
    }

    impl Read for Patient {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.tcp.read(buf)
        }
    }

    impl Write for Patient {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            loop {
                match (self.tcp.write(buf), self.wait) {
                    (Err(error), Some(wait)) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if !crate::poll::writable(self.tcp.as_raw_fd(), wait) {
                            return Err(std::io::ErrorKind::TimedOut.into());
                        }
                    }
                    (result, _) => return result,
                }
            }
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Stream for Patient {
        fn tcp(&self) -> &TcpStream {
            &self.tcp
        }
        fn close(&mut self) {
            let _ = self.tcp.shutdown(std::net::Shutdown::Write);
        }
        fn impatient(&mut self) {
            self.wait = None;
        }
    }

    impl Stream for rustls::StreamOwned<rustls::ServerConnection, Patient> {
        fn tcp(&self) -> &TcpStream {
            &self.sock.tcp
        }
        fn close(&mut self) {
            self.conn.send_close_notify();
            let _ = self.conn.write_tls(&mut self.sock);
            self.sock.close();
        }
        fn impatient(&mut self) {
            self.sock.impatient();
        }
    }

    fn lock<T: ?Sized>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
        mutex.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn send_all(stream: &SharedStream, bytes: &[u8], timeout: Duration) -> std::io::Result<()> {
        let deadline = Instant::now() + timeout;
        let mut sent = 0;
        loop {
            let (result, fd) = {
                let mut stream = lock(stream);
                let result = if sent < bytes.len() {
                    stream.write(&bytes[sent..]).map(Some)
                } else {
                    stream.flush().map(|_| None)
                };
                (result, stream.tcp().as_raw_fd())
            };
            match result {
                Ok(Some(0)) => return Err(std::io::ErrorKind::WriteZero.into()),
                Ok(Some(wrote)) => sent += wrote,
                Ok(None) => return Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    let left = deadline.saturating_duration_since(Instant::now());
                    if left.is_zero() || !crate::poll::writable(fd, left) {
                        return Err(std::io::ErrorKind::TimedOut.into());
                    }
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn read_into(stream: &SharedStream, buf: &mut Vec<u8>, cap: usize) -> bool {
        let mut scratch = [0u8; 16 * 1024];
        while buf.len() < cap {
            match lock(stream).read(&mut scratch) {
                Ok(0) => return false,
                Ok(read) => {
                    buf.extend_from_slice(&scratch[..read]);
                    if read < scratch.len() {
                        return true;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return true,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => return false,
            }
        }
        true
    }

    struct Conn {
        stream: SharedStream,
        fd: RawFd,
        buf: Vec<u8>,
        served: usize,
        since: Instant,
        started: Option<Instant>,
    }

    impl Conn {
        fn reject(self) {
            let mut stream = lock(&self.stream);
            let _ = stream.write_all(BAD_REQUEST);
            let _ = stream.flush();
            stream.close();
        }
    }

    pub fn accept(args: &[Value]) -> Result<Value, String> {
        let server = id(&args[0]);
        let listener = take(&LISTENERS, server).ok_or("the server is not listening")?;
        let result = loop {
            let stream = match listener.accept() {
                Ok((stream, _)) => stream,
                Err(error) => break Err(format!("the server stopped: {error}")),
            };
            match read_request(&stream) {
                Some((method, target, found, body)) => {
                    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
                    let (path, query) = (path.to_string(), query.to_string());
                    let connection = store(&CONNECTIONS, stream);
                    let found = found
                        .into_iter()
                        .map(|(name, value)| (name, Value::str(&value)))
                        .collect();
                    break Ok(Value::list(vec![
                        Value::Number(connection as f64),
                        Value::str(&method),
                        Value::str(&path),
                        Value::str(&query),
                        record(found),
                        Value::str(&String::from_utf8_lossy(&body)),
                    ]));
                }
                None => {
                    let mut stream = stream;
                    let _ = stream.write_all(BAD_REQUEST);
                }
            }
        };
        LISTENERS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get_or_insert_with(Default::default)
            .insert(server, listener);
        result
    }

    fn valid_header_name(name: &str) -> bool {
        const TOKEN: &str = "!#$%&'*+-.^_`|~";
        !name.is_empty()
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || TOKEN.as_bytes().contains(&b))
    }

    fn reason(status: u16) -> &'static str {
        match status {
            101 => "Switching Protocols",
            200 => "OK",
            201 => "Created",
            204 => "No Content",
            301 => "Moved Permanently",
            302 => "Found",
            304 => "Not Modified",
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            404 => "Not Found",
            405 => "Method Not Allowed",
            426 => "Upgrade Required",
            500 => "Internal Server Error",
            _ => "",
        }
    }

    pub fn respond(args: &[Value]) -> Result<Value, String> {
        let mut stream =
            take(&CONNECTIONS, id(&args[0])).ok_or("this request was already answered")?;
        let status = match &args[1] {
            Value::Number(n) if (100.0..=599.0).contains(n) => *n as u16,
            other => {
                return Err(format!(
                    "a response status must be from 100 to 599, got {other}"
                ));
            }
        };
        let body = text(&args[2], "Response")?;
        write_response(&mut stream, status, body.as_bytes(), &args[3], false)?;
        Ok(Value::None)
    }

    fn write_response<S: Write + ?Sized>(
        stream: &mut S,
        status: u16,
        body: &[u8],
        headers_value: &Value,
        keep_alive: bool,
    ) -> Result<(), String> {
        let mut head = format!("HTTP/1.1 {status} {}\r\n", reason(status));
        let mut typed = false;
        for (name, value) in headers(headers_value) {
            if name.contains(['\r', '\n']) || value.contains(['\r', '\n']) {
                return Err(format!("header '{}' contains a line break", name.trim()));
            }
            if !valid_header_name(&name) {
                return Err(format!("header '{name}' is not a valid header name"));
            }
            if matches!(
                name.to_ascii_lowercase().as_str(),
                "content-length" | "connection" | "transfer-encoding"
            ) {
                continue;
            }
            typed |= name.eq_ignore_ascii_case("content-type");
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        if !typed {
            head.push_str("content-type: text/plain; charset=utf-8\r\n");
        }
        head.push_str(&format!(
            "content-length: {}\r\nconnection: {}\r\n\r\n",
            body.len(),
            if keep_alive { "keep-alive" } else { "close" }
        ));
        let mut response = head.into_bytes();
        response.extend_from_slice(body);
        let _ = stream.write_all(&response);
        Ok(())
    }

    const INTERNAL_ERROR: &str = "internal server error";

    type SharedStream = std::sync::Arc<std::sync::Mutex<Box<dyn Stream>>>;

    thread_local! {
        static STREAM_OUT: std::cell::RefCell<Option<SharedStream>> = const { std::cell::RefCell::new(None) };
    }

    pub fn emit_chunk(args: &[Value]) -> Result<Value, String> {
        let chunk = match args.first() {
            Some(Value::Str(text)) => text.to_string(),
            Some(other) => other.to_string(),
            None => String::new(),
        };
        STREAM_OUT.with(|out| {
            if let Some(stream) = out.borrow().as_ref() {
                let mut stream = stream.lock().unwrap_or_else(|p| p.into_inner());
                let _ = stream.write_all(format!("{:x}\r\n{}\r\n", chunk.len(), chunk).as_bytes());
                let _ = stream.flush();
            }
            Ok(Value::None)
        })
    }

    fn marker(value: &Value, name: &str) -> Option<Value> {
        let Value::Record(record) = value else {
            return None;
        };
        let record = record.borrow();
        if record.iter().any(|(field, _)| &**field == name) {
            record.get(name)
        } else {
            None
        }
    }

    fn stream_headers(stream: &mut dyn Write, keep_alive: bool) {
        let mut head = "HTTP/1.1 200 OK\r\ncontent-type: text/plain; charset=utf-8\r\ntransfer-encoding: chunked\r\n".to_string();
        head.push_str(&format!(
            "connection: {}\r\n\r\n",
            if keep_alive { "keep-alive" } else { "close" }
        ));
        let _ = stream.write_all(head.as_bytes());
        let _ = stream.flush();
    }

    fn stream_response(stream: SharedStream, vm: &mut Vm, producer: Value, keep_alive: bool) {
        stream_headers(
            &mut *stream.lock().unwrap_or_else(|p| p.into_inner()),
            keep_alive,
        );
        STREAM_OUT.with(|out| *out.borrow_mut() = Some(std::sync::Arc::clone(&stream)));
        let result = vm.call(producer, vec![Value::Native(&EMIT_NATIVE)]);
        STREAM_OUT.with(|out| *out.borrow_mut() = None);
        match result {
            Ok(_) => {
                let _ = stream
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .write_all(b"0\r\n\r\n");
                let _ = stream.lock().unwrap_or_else(|p| p.into_inner()).flush();
            }
            Err(error) => {
                eprintln!("{}", crate::task::render(&error));
                lock(&stream).close();
            }
        }
    }

    static EMIT_NATIVE: Native = Native {
        name: "emit#",
        function: emit_chunk,
    };

    fn struct_def(globals: &GlobalStore, name: &str) -> Option<Rc<StructDef>> {
        let store = globals.borrow();
        let slot = store.names.iter().position(|n| n == name)?;
        match store.values.get(slot)? {
            Some(Value::StructDef(def)) => Some(Rc::clone(def)),
            _ => None,
        }
    }

    fn response_parts(value: &Value) -> Result<(u16, Vec<u8>, Value), String> {
        let Value::Instance(instance) = value else {
            return Err(format!(
                "a handler must return a Response, got {}",
                value.type_name()
            ));
        };
        let field = |name: &str| -> Option<Value> {
            instance
                .def
                .fields
                .iter()
                .position(|f| &**f == name)
                .and_then(|at| instance.fields.borrow().get(at).cloned())
        };
        let Some(Value::Number(status)) = field("status") else {
            return Err("a handler must return a Response with a number status".to_string());
        };
        if !(100.0..=599.0).contains(&status) || status.fract() != 0.0 {
            return Err(format!(
                "a response status must be from 100 to 599, got {status}"
            ));
        }
        let body = match field("body") {
            Some(Value::Str(text)) => text.as_bytes().to_vec(),
            Some(Value::Bytes(bytes)) => bytes.to_vec(),
            _ => {
                return Err(
                    "a handler must return a Response with a string or bytes body".to_string(),
                );
            }
        };
        let headers = field("headers").unwrap_or_else(|| Value::record(Vec::new()));
        Ok((status as u16, body, headers))
    }

    const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

    fn upgrade_key(method: &str, found: &[(String, String)]) -> Option<String> {
        let header = |name: &str| {
            found
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(name))
                .map(|(_, value)| value.trim())
        };
        let key = header("sec-websocket-key")?;
        let upgrade = method == "GET"
            && header("upgrade")?.eq_ignore_ascii_case("websocket")
            && header("connection")?
                .split(',')
                .any(|token| token.trim().eq_ignore_ascii_case("upgrade"))
            && header("sec-websocket-version")? == "13"
            && key.len() == 24;
        upgrade.then(|| key.to_string())
    }

    pub fn websocket_request(args: &[Value]) -> Result<Value, String> {
        let method = text(&args[0], "Request.is_websocket")?;
        Ok(Value::Bool(
            upgrade_key(&method, &headers(&args[1])).is_some(),
        ))
    }

    fn accept_key(key: &str) -> String {
        let digest = ring::digest::digest(
            &ring::digest::SHA1_FOR_LEGACY_USE_ONLY,
            format!("{key}{WEBSOCKET_GUID}").as_bytes(),
        );
        crate::web::base64(digest.as_ref())
    }

    #[derive(Debug, PartialEq)]
    struct Frame {
        fin: bool,
        opcode: u8,
        payload: Vec<u8>,
    }

    fn parse_frame(buf: &[u8], max: usize) -> Result<Option<(Frame, usize)>, u16> {
        let [first, second, ..] = *buf else {
            return Ok(None);
        };
        let fin = first & 0x80 != 0;
        let opcode = first & 0x0f;
        if first & 0x70 != 0 || !matches!(opcode, 0x0..=0x2 | 0x8..=0xa) || second & 0x80 == 0 {
            return Err(1002);
        }
        let (len, start) = match second & 0x7f {
            126 => match buf.get(2..4) {
                Some(bytes) => (u64::from(u16::from_be_bytes([bytes[0], bytes[1]])), 4),
                None => return Ok(None),
            },
            127 => match buf.get(2..10) {
                Some(bytes) => {
                    let mut wide = [0u8; 8];
                    wide.copy_from_slice(bytes);
                    (u64::from_be_bytes(wide), 10)
                }
                None => return Ok(None),
            },
            short => (u64::from(short), 2),
        };
        if len >> 63 != 0 || (opcode >= 0x8 && (len > 125 || !fin)) {
            return Err(1002);
        }
        if len > max as u64 {
            return Err(1009);
        }
        let len = len as usize;
        let Some(mask) = buf.get(start..start + 4) else {
            return Ok(None);
        };
        let Some(masked) = buf.get(start + 4..start + 4 + len) else {
            return Ok(None);
        };
        let payload = masked
            .iter()
            .zip(mask.iter().cycle())
            .map(|(byte, key)| byte ^ key)
            .collect();
        Ok(Some((
            Frame {
                fin,
                opcode,
                payload,
            },
            start + 4 + len,
        )))
    }

    fn frame_bytes(opcode: u8, payload: &[u8]) -> Vec<u8> {
        let mut out = vec![0x80 | opcode];
        match payload.len() {
            len @ 0..=125 => out.push(len as u8),
            len @ 126..=0xffff => {
                out.push(126);
                out.extend_from_slice(&(len as u16).to_be_bytes());
            }
            len => {
                out.push(127);
                out.extend_from_slice(&(len as u64).to_be_bytes());
            }
        }
        out.extend_from_slice(payload);
        out
    }

    fn close_reply(payload: &[u8]) -> Result<Vec<u8>, u16> {
        match payload {
            [] => Ok(Vec::new()),
            [_] => Err(1002),
            [high, low, reason @ ..] => {
                let code = u16::from_be_bytes([*high, *low]);
                if !matches!(code, 1000..=1003 | 1007..=1011 | 3000..=4999) {
                    return Err(1002);
                }
                if std::str::from_utf8(reason).is_err() {
                    return Err(1007);
                }
                Ok(code.to_be_bytes().to_vec())
            }
        }
    }

    fn code(code: u16) -> Option<Vec<u8>> {
        Some(code.to_be_bytes().to_vec())
    }

    const BUSY: Duration = Duration::from_millis(100);

    struct Socket {
        stream: SharedStream,
        fd: RawFd,
        writing: Mutex<()>,
        closed: AtomicBool,
        channel: Weak<Channel>,
        timeout: Duration,
    }

    impl Socket {
        fn write(&self, opcode: u8, payload: &[u8], patient: bool) -> std::io::Result<()> {
            let frame = frame_bytes(opcode, payload);
            if patient {
                let _turn = lock(&self.writing);
                return send_all(&self.stream, &frame, self.timeout);
            }
            match self.writing.try_lock() {
                Ok(_turn) => send_all(&self.stream, &frame, BUSY),
                Err(_) => Err(std::io::ErrorKind::WouldBlock.into()),
            }
        }

        fn shut(&self, farewell: Option<&[u8]>, patient: bool) {
            if !self.closed.swap(true, Ordering::SeqCst)
                && let Some(payload) = farewell
            {
                let _ = self.write(0x8, payload, patient);
            }
            lock(&self.stream).close();
            if let Some(channel) = self.channel.upgrade() {
                crate::task::seal(&channel);
            }
        }
    }

    impl crate::task::Endpoint for Socket {
        fn send(&self, value: &Value) -> Result<(), String> {
            let (opcode, message) = match value {
                Value::Str(text) => (0x1, text.as_bytes()),
                Value::Bytes(bytes) => (0x2, bytes.as_slice()),
                other => {
                    return Err(format!(
                        "a websocket can only send a string or bytes, got {}",
                        other.type_name()
                    ));
                }
            };
            if self.closed.load(Ordering::SeqCst) || self.write(opcode, message, true).is_err() {
                return Err("cannot send on a closed websocket".to_string());
            }
            Ok(())
        }

        fn close(&self, code: u16, reason: &str) {
            let mut payload = code.to_be_bytes().to_vec();
            payload.extend_from_slice(reason.as_bytes());
            self.shut(Some(&payload), true);
        }
    }

    struct Reader {
        buf: Vec<u8>,
        partial: Option<(u8, Vec<u8>)>,
        max: usize,
        timeout: Duration,
        since: Instant,
        pinged: bool,
    }

    impl Reader {
        fn new(buf: Vec<u8>, limits: &Limits) -> Reader {
            Reader {
                buf,
                partial: None,
                max: limits.max_body,
                timeout: limits.idle_timeout.saturating_mul(12),
                since: Instant::now(),
                pinged: false,
            }
        }

        fn take(&mut self, socket: &Socket, channel: &Channel) -> Result<(), Option<Vec<u8>>> {
            loop {
                let (frame, used) = match parse_frame(&self.buf, self.max) {
                    Err(error) => return Err(code(error)),
                    Ok(None) => return Ok(()),
                    Ok(Some(found)) => found,
                };
                self.buf.drain(..used);
                match frame.opcode {
                    0x8 => return Err(Some(close_reply(&frame.payload).map_err(code)?)),
                    0x9 => socket.write(0xa, &frame.payload, false).map_err(|_| None)?,
                    0xa => {}
                    opcode => {
                        let (kind, mut data) = match (opcode, self.partial.take()) {
                            (0x0, Some(started)) => started,
                            (0x1 | 0x2, None) => (opcode, Vec::new()),
                            _ => return Err(code(1002)),
                        };
                        if data.len() + frame.payload.len() > self.max {
                            return Err(code(1009));
                        }
                        data.extend_from_slice(&frame.payload);
                        if !frame.fin {
                            self.partial = Some((kind, data));
                            continue;
                        }
                        let message = if kind == 0x2 {
                            crate::task::Slot::Bytes(data)
                        } else {
                            crate::task::Slot::Str(String::from_utf8(data).map_err(|_| code(1007))?)
                        };
                        crate::task::deliver(channel, message);
                    }
                }
            }
        }
    }

    struct Open {
        socket: Arc<Socket>,
        channel: Arc<Channel>,
        reader: Reader,
    }

    impl Open {
        fn heard(mut self, hub: &Hub, open: bool) {
            match self.reader.take(&self.socket, &self.channel) {
                Err(farewell) => self.socket.shut(farewell.as_deref(), false),
                Ok(()) if !open => self.socket.shut(None, false),
                Ok(()) if self.socket.closed.load(Ordering::SeqCst) => {}
                Ok(()) => hub.park(Entry::Socket(self)),
            }
        }
    }

    fn websocket(stream: SharedStream, fd: RawFd, handler: Value, upgrade: Upgrade, hub: &Hub) {
        let head = format!(
            "HTTP/1.1 101 Switching Protocols\r\nupgrade: websocket\r\nconnection: Upgrade\r\nsec-websocket-accept: {}\r\n\r\n",
            accept_key(&upgrade.key)
        );
        {
            let mut out = lock(&stream);
            if out
                .write_all(head.as_bytes())
                .and_then(|_| out.flush())
                .is_err()
            {
                return;
            }
            out.impatient();
        }
        let mut made = None;
        let channel = crate::task::endpoint(|channel| {
            let socket = Arc::new(Socket {
                stream,
                fd,
                writing: Mutex::new(()),
                closed: AtomicBool::new(false),
                channel,
                timeout: hub.limits.request_timeout,
            });
            made = Some(Arc::clone(&socket));
            socket
        });
        let Some(socket) = made else { return };
        ACTIVE.fetch_add(1, Ordering::SeqCst);
        let open = Open {
            socket: Arc::clone(&socket),
            channel: Arc::clone(&channel),
            reader: Reader::new(upgrade.leftover, &hub.limits),
        };
        open.heard(hub, true);
        crate::task::spawn_with(
            &[handler, Value::Channel(channel)],
            Box::new(move |ok| {
                let status: u16 = if ok { 1000 } else { 1011 };
                socket.shut(Some(&status.to_be_bytes()), true);
                ACTIVE.fetch_sub(1, Ordering::SeqCst);
            }),
        );
    }

    const WORKER_STACK: usize = 16 * 1024 * 1024;
    const WORKERS_PER_CORE: usize = 2;
    const MIN_WORKERS: usize = 4;
    const MAX_WORKERS: usize = 64;
    const DRAIN_TIMEOUT: Duration = Duration::from_secs(10);

    static SHUTDOWN_ARC: std::sync::OnceLock<std::sync::Arc<std::sync::atomic::AtomicBool>> =
        std::sync::OnceLock::new();
    static ACTIVE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    fn shutdown_flag() -> std::sync::Arc<std::sync::atomic::AtomicBool> {
        SHUTDOWN_ARC
            .get_or_init(|| std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)))
            .clone()
    }

    pub fn is_shutdown() -> bool {
        shutdown_flag().load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn begin_shutdown() {
        shutdown_flag().store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn reset_shutdown() {
        shutdown_flag().store(false, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn active_count() -> usize {
        ACTIVE.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn drain(max: Duration) {
        let start = Instant::now();
        while ACTIVE.load(std::sync::atomic::Ordering::SeqCst) > 0 && start.elapsed() < max {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn percent_decode(text: &str) -> String {
        let bytes = text.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut at = 0;
        while at < bytes.len() {
            match bytes[at] {
                b'+' => {
                    out.push(b' ');
                    at += 1;
                }
                b'%' => {
                    let hex = |b: u8| -> Option<u8> {
                        match b {
                            b'0'..=b'9' => Some(b - b'0'),
                            b'a'..=b'f' => Some(b - b'a' + 10),
                            b'A'..=b'F' => Some(b - b'A' + 10),
                            _ => None,
                        }
                    };
                    match (
                        bytes.get(at + 1).and_then(|h| hex(*h)),
                        bytes.get(at + 2).and_then(|h| hex(*h)),
                    ) {
                        (Some(hi), Some(lo)) => {
                            out.push(hi * 16 + lo);
                            at += 3;
                        }
                        _ => {
                            out.push(bytes[at]);
                            at += 1;
                        }
                    }
                }
                byte => {
                    out.push(byte);
                    at += 1;
                }
            }
        }
        String::from_utf8_lossy(&out).into_owned()
    }

    fn parse_query(query: &str) -> Vec<(Rc<str>, Value)> {
        if query.is_empty() {
            return Vec::new();
        }
        let mut pairs: Vec<(Rc<str>, Value)> = Vec::new();
        for pair in query.split('&').filter(|pair| !pair.is_empty()) {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            let name = Rc::from(percent_decode(name).as_str());
            let value = Value::str(&percent_decode(value));
            match pairs.iter().position(|(existing, _)| *existing == name) {
                Some(at) => pairs[at] = (name, value),
                None => pairs.push((name, value)),
            }
        }
        pairs
    }

    fn build_request(
        request_def: Option<&Rc<StructDef>>,
        method: &str,
        target: &str,
        found: Vec<(String, String)>,
        body: Vec<u8>,
    ) -> Value {
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        let headers_record = Value::record(
            found
                .into_iter()
                .map(|(name, value)| (Rc::from(name.as_str()), Value::str(&value)))
                .collect(),
        );
        let given = [
            ("method", Value::str(method)),
            ("path", Value::str(path)),
            ("query", Value::str(query)),
            ("query_params", Value::record(parse_query(query))),
            ("headers", headers_record),
            ("body", Value::str(&String::from_utf8_lossy(&body))),
            ("bytes", Value::Bytes(Rc::new(body))),
        ];
        match request_def {
            Some(def) => {
                let mut fields = vec![Value::None; def.fields.len()];
                for (name, value) in given {
                    if let Some(at) = def.fields.iter().position(|f| &**f == name) {
                        fields[at] = value;
                    }
                }
                Value::instance(Instance {
                    def: Rc::clone(def),
                    fields: RefCell::new(fields),
                })
            }
            None => Value::record(
                given
                    .into_iter()
                    .map(|(name, value)| (Rc::from(name), value))
                    .collect(),
            ),
        }
    }

    struct Upgrade {
        key: String,
        leftover: Vec<u8>,
    }

    fn answer(
        vm: &mut Vm,
        handler: &Value,
        request: Value,
        conn: &Conn,
        keep_alive: bool,
        upgrade: Option<Upgrade>,
        hub: &Hub,
    ) {
        let stream = Arc::clone(&conn.stream);
        let returned = match vm.call(handler.clone(), vec![request]) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("{}", crate::task::render(&error));
                write_response(
                    lock(&stream).as_mut(),
                    500,
                    INTERNAL_ERROR.as_bytes(),
                    &Value::record(Vec::new()),
                    keep_alive,
                )
                .ok();
                return;
            }
        };
        if let Some(socket_handler) = marker(&returned, "khipu_websocket") {
            match upgrade {
                Some(upgrade) => websocket(stream, conn.fd, socket_handler, upgrade, hub),
                None => {
                    let _ = write_response(
                        lock(&stream).as_mut(),
                        426,
                        b"this address only speaks websockets",
                        &Value::record(vec![(Rc::from("sec-websocket-version"), Value::str("13"))]),
                        keep_alive,
                    );
                }
            }
            return;
        }
        if let Some(producer) = marker(&returned, "khipu_stream") {
            stream_response(stream, vm, producer, keep_alive);
            return;
        }
        let (status, response_body, response_headers) = match response_parts(&returned) {
            Ok(response) => response,
            Err(message) => {
                eprintln!("in a request handler: {message}");
                (
                    500,
                    INTERNAL_ERROR.as_bytes().to_vec(),
                    Value::record(Vec::new()),
                )
            }
        };
        if write_response(
            lock(&stream).as_mut(),
            status,
            &response_body,
            &response_headers,
            keep_alive,
        )
        .is_err()
        {
            let _ = write_response(
                lock(&stream).as_mut(),
                500,
                INTERNAL_ERROR.as_bytes(),
                &Value::record(Vec::new()),
                keep_alive,
            );
        }
    }

    const NEXT_REQUEST: Duration = Duration::from_millis(1);

    struct Job {
        conn: Conn,
        incoming: Incoming,
        keep_alive: bool,
    }

    fn handle(
        job: Job,
        vm: &mut Vm,
        handler: &Value,
        request_def: Option<&Rc<StructDef>>,
        hub: &Hub,
    ) {
        let Job {
            mut conn,
            mut incoming,
            mut keep_alive,
        } = job;
        let limits = hub.limits;
        ACTIVE.fetch_add(1, Ordering::SeqCst);
        loop {
            let (method, target, found, body) = incoming;
            let upgrade = upgrade_key(&method, &found).map(|key| Upgrade {
                key,
                leftover: std::mem::take(&mut conn.buf),
            });
            let request = build_request(request_def, &method, &target, found, body);
            let more = keep_alive
                && upgrade.is_none()
                && conn.served + 1 < limits.max_requests
                && !is_shutdown();
            answer(vm, handler, request, &conn, more, upgrade, hub);
            conn.served += 1;
            if !more {
                break;
            }
            if conn.buf.is_empty()
                && hub.queued.load(Ordering::SeqCst) == 0
                && crate::poll::readable(conn.fd, NEXT_REQUEST)
            {
                let cap = limits.max_request_head + limits.max_body + 4;
                read_into(&conn.stream, &mut conn.buf, cap);
            }
            match parse_request(&conn.buf, &limits) {
                Parsed::Request(next, alive, used) => {
                    conn.buf.drain(..used);
                    incoming = next;
                    keep_alive = alive;
                }
                Parsed::Bad => {
                    conn.reject();
                    break;
                }
                Parsed::Incomplete => {
                    conn.since = Instant::now();
                    conn.started = (!conn.buf.is_empty()).then_some(conn.since);
                    hub.park(Entry::Http(conn));
                    break;
                }
            }
        }
        ACTIVE.fetch_sub(1, Ordering::SeqCst);
    }

    fn http_worker(packed: crate::task::Graph, jobs: Arc<Mutex<Receiver<Job>>>, hub: Arc<Hub>) {
        let handler = crate::task::unpack(packed).remove(0);
        let handler_globals = match &handler {
            Value::Function(closure) => Some(Rc::clone(&closure.globals)),
            _ => None,
        };
        let mut vm = Vm::task(&crate::task::program_dir());
        vm.preemptive = false;
        if let Some(globals) = &handler_globals {
            vm.globals = Rc::clone(globals);
        }
        let request_def = handler_globals
            .as_ref()
            .and_then(|g| struct_def(g, "Request"));
        loop {
            let job = lock(&jobs).recv();
            let Ok(job) = job else { break };
            hub.queued.fetch_sub(1, Ordering::SeqCst);
            handle(job, &mut vm, &handler, request_def.as_ref(), &hub);
        }
    }

    enum Entry {
        Http(Conn),
        Socket(Open),
    }

    impl Entry {
        fn fd(&self) -> RawFd {
            match self {
                Entry::Http(conn) => conn.fd,
                Entry::Socket(open) => open.socket.fd,
            }
        }

        fn expired(&self, now: Instant, limits: &Limits) -> bool {
            match self {
                Entry::Http(conn) => match conn.started {
                    None if conn.served == 0 => now > conn.since + limits.read_timeout,
                    None => now > conn.since + limits.idle_timeout,
                    Some(started) => {
                        now > started + limits.request_timeout
                            || now > conn.since + limits.read_timeout
                    }
                },
                Entry::Socket(open) => now > open.reader.since + open.reader.timeout,
            }
        }
    }

    struct Hub {
        poller: crate::poll::Poller,
        entries: Mutex<HashMap<u64, Entry>>,
        next: AtomicU64,
        queued: std::sync::atomic::AtomicUsize,
        limits: Limits,
    }

    const LISTENER: u64 = 0;
    const TICK: Duration = Duration::from_millis(50);

    impl Hub {
        fn new(limits: Limits) -> Result<Hub, String> {
            Ok(Hub {
                poller: crate::poll::Poller::new()
                    .map_err(|error| format!("cannot start the event loop: {error}"))?,
                entries: Mutex::new(HashMap::new()),
                next: AtomicU64::new(LISTENER + 1),
                queued: std::sync::atomic::AtomicUsize::new(0),
                limits,
            })
        }

        fn park(&self, entry: Entry) {
            let token = self.next.fetch_add(1, Ordering::Relaxed);
            let fd = entry.fd();
            lock(&self.entries).insert(token, entry);
            if self.poller.arm(fd, token).is_err() {
                lock(&self.entries).remove(&token);
            }
        }

        fn readable(&self, entry: Entry, jobs: &SyncSender<Job>) {
            let now = Instant::now();
            match entry {
                Entry::Http(mut conn) => {
                    let cap = self.limits.max_request_head + self.limits.max_body + 4;
                    let before = conn.buf.len();
                    let open = read_into(&conn.stream, &mut conn.buf, cap);
                    if conn.buf.len() > before {
                        conn.since = now;
                        conn.started.get_or_insert(now);
                    }
                    match parse_request(&conn.buf, &self.limits) {
                        Parsed::Request(incoming, keep_alive, used) => {
                            conn.buf.drain(..used);
                            conn.started = None;
                            self.queued.fetch_add(1, Ordering::SeqCst);
                            let _ = jobs.send(Job {
                                conn,
                                incoming,
                                keep_alive,
                            });
                        }
                        Parsed::Bad => conn.reject(),
                        Parsed::Incomplete if open => self.park(Entry::Http(conn)),
                        Parsed::Incomplete if conn.buf.is_empty() => {}
                        Parsed::Incomplete => conn.reject(),
                    }
                }
                Entry::Socket(mut open) => {
                    let before = open.reader.buf.len();
                    let cap = open.reader.max + 14;
                    let alive = read_into(&open.socket.stream, &mut open.reader.buf, cap);
                    if open.reader.buf.len() > before {
                        open.reader.since = now;
                        open.reader.pinged = false;
                    }
                    open.heard(self, alive);
                }
            }
        }

        fn sweep(&self) {
            let now = Instant::now();
            let expired: Vec<Entry> = {
                let mut entries = lock(&self.entries);
                let tokens: Vec<u64> = entries
                    .iter()
                    .filter(|(_, entry)| entry.expired(now, &self.limits))
                    .map(|(token, _)| *token)
                    .collect();
                tokens
                    .iter()
                    .filter_map(|token| entries.remove(token))
                    .collect()
            };
            for entry in expired {
                match entry {
                    Entry::Http(conn) if conn.buf.is_empty() => {}
                    Entry::Http(conn) => conn.reject(),
                    Entry::Socket(open) if open.reader.pinged => {
                        open.socket.shut(Some(&1001u16.to_be_bytes()), false)
                    }
                    Entry::Socket(mut open) => {
                        if open.socket.write(0x9, b"", false).is_err() {
                            open.socket.shut(None, false);
                            continue;
                        }
                        open.reader.pinged = true;
                        open.reader.since = now;
                        self.park(Entry::Socket(open));
                    }
                }
            }
        }

        fn accept(&self, listener: &TcpListener, tls: Option<&Arc<rustls::ServerConfig>>) {
            loop {
                let tcp = match listener.accept() {
                    Ok((tcp, _)) => tcp,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return,
                    Err(error) => {
                        eprintln!("could not accept a connection: {error}");
                        std::thread::sleep(Duration::from_millis(1));
                        return;
                    }
                };
                if is_shutdown() || tcp.set_nonblocking(true).is_err() {
                    continue;
                }
                let fd = tcp.as_raw_fd();
                let tcp = Patient {
                    tcp,
                    wait: Some(self.limits.request_timeout),
                };
                let stream: Box<dyn Stream> = match tls {
                    Some(config) => match rustls::ServerConnection::new(Arc::clone(config)) {
                        Ok(conn) => Box::new(rustls::StreamOwned::new(conn, tcp)),
                        Err(error) => {
                            eprintln!("TLS handshake setup failed: {error}");
                            continue;
                        }
                    },
                    None => Box::new(tcp),
                };
                self.park(Entry::Http(Conn {
                    stream: Arc::new(Mutex::new(stream)),
                    fd,
                    buf: Vec::new(),
                    served: 0,
                    since: Instant::now(),
                    started: None,
                }));
            }
        }

        fn run(
            &self,
            listener: &TcpListener,
            jobs: &SyncSender<Job>,
            tls: Option<&Arc<rustls::ServerConfig>>,
            flag: &AtomicBool,
        ) {
            let listening = listener.as_raw_fd();
            let _ = self.poller.arm(listening, LISTENER);
            let mut tokens = Vec::new();
            let mut swept = Instant::now();
            while !flag.load(Ordering::SeqCst) {
                tokens.clear();
                if self.poller.wait(&mut tokens, TICK).is_err() {
                    std::thread::sleep(TICK);
                }
                for &token in &tokens {
                    if token == LISTENER {
                        self.accept(listener, tls);
                        let _ = self.poller.arm(listening, LISTENER);
                        continue;
                    }
                    let entry = lock(&self.entries).remove(&token);
                    if let Some(entry) = entry {
                        self.readable(entry, jobs);
                    }
                }
                if swept.elapsed() >= TICK {
                    self.sweep();
                    swept = Instant::now();
                }
            }
            let left: Vec<Entry> = lock(&self.entries)
                .drain()
                .map(|(_, entry)| entry)
                .collect();
            for entry in left {
                if let Entry::Socket(open) = entry {
                    open.socket.shut(Some(&1001u16.to_be_bytes()), false);
                }
            }
        }
    }

    pub fn serve(args: &[Value]) -> Result<Value, String> {
        let port = match &args[0] {
            Value::Number(n) if n.fract() == 0.0 && (1.0..=65535.0).contains(n) => *n as u16,
            other => {
                return Err(format!(
                    "Http.serve needs a port from 1 to 65535, got {other}"
                ));
            }
        };
        let handler = args
            .get(1)
            .cloned()
            .ok_or("Http.serve needs a handler".to_string())?;
        match &handler {
            Value::Function(_) | Value::Native(_) => {}
            _ => return Err("Http.serve needs a function to handle requests".to_string()),
        }
        let listener = TcpListener::bind(("127.0.0.1", port))
            .map_err(|error| format!("cannot listen on port {port}: {error}"))?;
        let limits = limits_from(args.get(2));
        start_pool(listener, handler, limits, None, port, "http")
    }

    pub fn serve_tls(args: &[Value]) -> Result<Value, String> {
        let port = match &args[0] {
            Value::Number(n) if n.fract() == 0.0 && (1.0..=65535.0).contains(n) => *n as u16,
            other => {
                return Err(format!(
                    "Http.serve_tls needs a port from 1 to 65535, got {other}"
                ));
            }
        };
        let cert_path = text(&args[1], "Http.serve_tls")?;
        let key_path = text(&args[2], "Http.serve_tls")?;
        let handler = args
            .get(3)
            .cloned()
            .ok_or("Http.serve_tls needs a handler".to_string())?;
        match &handler {
            Value::Function(_) | Value::Native(_) => {}
            _ => return Err("Http.serve_tls needs a function to handle requests".to_string()),
        }
        let certs = {
            use rustls::pki_types::pem::PemObject;
            rustls::pki_types::CertificateDer::pem_file_iter(&cert_path)
                .map_err(|error| format!("cannot read certificate {cert_path}: {error}"))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("cannot read certificate {cert_path}: {error}"))?
        };
        let key = {
            use rustls::pki_types::pem::PemObject;
            rustls::pki_types::PrivateKeyDer::from_pem_file(&key_path)
                .map_err(|error| format!("cannot read key {key_path}: {error}"))?
        };
        let _ = rustls::crypto::CryptoProvider::install_default(
            rustls::crypto::ring::default_provider(),
        );
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|error| format!("cannot build TLS config: {error}"))?;
        let listener = TcpListener::bind(("127.0.0.1", port))
            .map_err(|error| format!("cannot listen on port {port}: {error}"))?;
        start_pool(
            listener,
            handler,
            Limits::default(),
            Some(std::sync::Arc::new(config)),
            port,
            "https",
        )
    }

    fn start_pool(
        listener: TcpListener,
        handler: Value,
        limits: Limits,
        tls: Option<Arc<rustls::ServerConfig>>,
        port: u16,
        scheme: &str,
    ) -> Result<Value, String> {
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("cannot listen on port {port}: {error}"))?;
        let hub = Arc::new(Hub::new(limits)?);
        let flag = shutdown_flag();
        let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, flag.clone());
        let _ = signal_hook::flag::register(signal_hook::consts::SIGTERM, flag.clone());
        println!("listening on {scheme}://localhost:{port}");
        let workers = std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .saturating_mul(WORKERS_PER_CORE)
            .clamp(MIN_WORKERS, MAX_WORKERS);
        let (sender, receiver) = std::sync::mpsc::sync_channel::<Job>(workers.saturating_mul(2));
        let receiver = Arc::new(Mutex::new(receiver));
        for _ in 0..workers {
            let packed = crate::task::pack(std::slice::from_ref(&handler));
            let receiver = Arc::clone(&receiver);
            let hub = Arc::clone(&hub);
            std::thread::Builder::new()
                .name("khipu-http".to_string())
                .stack_size(WORKER_STACK)
                .spawn(move || http_worker(packed, receiver, hub))
                .expect("could not start a request worker");
        }
        hub.run(&listener, &sender, tls.as_ref(), &flag);
        drop(sender);
        drain(DRAIN_TIMEOUT);
        Ok(Value::None)
    }

    #[cfg(test)]
    mod tests {
        use std::io::{Read, Write};
        use std::net::{Shutdown, TcpListener, TcpStream};
        use std::rc::Rc;

        use std::os::fd::AsRawFd;
        use std::sync::atomic::AtomicBool;
        use std::sync::{Arc, Mutex};
        use std::time::Duration;

        use super::Incoming;
        use super::read_request;
        use super::{
            CONNECTIONS, Entry, Frame, Hub, Limits, MAX_BODY, MAX_HEADER_LINE, MAX_HEADERS,
            MAX_REQUEST_HEAD, Open, Patient, Reader, Socket, Stream, accept_key, frame_bytes,
            parse_frame, respond, store, upgrade_key,
        };
        use crate::task::Endpoint;
        use crate::value::Value;

        fn pair() -> (TcpStream, TcpStream) {
            let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let address = listener.local_addr().unwrap();
            let client = TcpStream::connect(address).unwrap();
            let (server, _) = listener.accept().unwrap();
            (client, server)
        }

        fn read_request_bytes(raw: &[u8]) -> Option<Incoming> {
            let (mut client, server) = pair();
            let owned = raw.to_vec();
            let writer = std::thread::spawn(move || {
                let _ = client.write_all(&owned);
                let _ = client.shutdown(Shutdown::Write);
            });
            let result = read_request(&server);
            let _ = writer.join();
            result
        }

        #[test]
        fn reads_a_well_formed_request() {
            let raw = b"POST /submit?a=1&b=2 HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\nX-Test: yes\r\n\r\nhello";
            let (method, target, headers, body) = read_request_bytes(raw).unwrap();
            assert_eq!(method, "POST");
            assert_eq!(target, "/submit?a=1&b=2");
            assert_eq!(body, b"hello");
            assert!(headers.contains(&("host".to_string(), "localhost".to_string())));
            assert!(headers.contains(&("x-test".to_string(), "yes".to_string())));
            assert!(headers.contains(&("content-length".to_string(), "5".to_string())));
        }

        #[test]
        fn reads_headers_and_body_arriving_together_and_in_pieces() {
            let (mut client, server) = pair();
            let server_thread = std::thread::spawn(move || read_request(&server));
            let raw = b"GET /x HTTP/1.1\r\nHost: h\r\nContent-Length: 3\r\n\r\nabc";
            for byte in raw {
                client.write_all(&[*byte]).unwrap();
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            client.shutdown(Shutdown::Write).unwrap();
            let (method, target, _, body) = server_thread.join().unwrap().unwrap();
            assert_eq!(
                (method.as_str(), target.as_str(), body.as_slice()),
                ("GET", "/x", &b"abc"[..])
            );
        }

        #[test]
        fn accepts_lf_only_line_endings() {
            let raw = b"GET /x HTTP/1.1\nHost: h\nContent-Length: 3\n\nabc";
            let (_, _, _, body) = read_request_bytes(raw).unwrap();
            assert_eq!(body, b"abc");
        }

        #[test]
        fn a_truncated_request_is_rejected() {
            assert!(read_request_bytes(b"GET / HTTP/1.1\r\nHost: h\r\n\r").is_none());
            assert!(read_request_bytes(b"").is_none());
            assert!(read_request_bytes(b"GET / HTTP/1.1\r\nHost: h\r\n").is_none());
            let raw = b"POST / HTTP/1.1\r\nContent-Length: 5\r\n\r\nab";
            assert!(read_request_bytes(raw).is_none());
        }

        #[test]
        fn a_malformed_request_is_rejected_never_a_crash() {
            for raw in [
                &b"garbage\r\n\r\n"[..],
                b"GET\r\n\r\n",
                b"GET / HTTP/1.1\r\nNoColonHere\r\n\r\n",
                b"GET / HTTP/1.1\r\n: value\r\n\r\n",
                b"GET / HTTP/1.1\r\nHost h\r\n\r\n",
            ] {
                assert!(read_request_bytes(raw).is_none(), "{raw:?}");
            }
        }

        #[test]
        fn invalid_utf8_in_headers_is_rejected_but_not_a_crash() {
            let mut raw = b"GET / HTTP/1.1\r\nX-Bad: ".to_vec();
            raw.push(0xff);
            raw.extend_from_slice(b"\r\n\r\n");
            assert!(read_request_bytes(&raw).is_none());
        }

        #[test]
        fn request_smuggling_is_rejected() {
            let base = b"POST / HTTP/1.1\r\nHost: h\r\n";
            for head in [
                &b"Content-Length: 3\r\nContent-Length: 3\r\n"[..],
                b"Content-Length: 3\r\nContent-Length: 4\r\n",
                b"Content-Length: 3x\r\n",
                b"Content-Length: -1\r\n",
                b"Content-Length: \r\n",
                b"Content-Length: 3,3\r\n",
                b"Content-Length: +3\r\n",
                b"Transfer-Encoding: chunked\r\n",
                b"Transfer-Encoding: chunked\r\nContent-Length: 3\r\n",
                b"Transfer-Encoding: gzip\r\n",
            ] {
                let mut raw = base.to_vec();
                raw.extend_from_slice(head);
                raw.extend_from_slice(b"\r\nabc");
                assert!(read_request_bytes(&raw).is_none(), "{head:?}");
            }
        }

        #[test]
        fn transfer_encoding_identity_with_content_length_is_allowed() {
            let raw =
                b"POST / HTTP/1.1\r\nTransfer-Encoding: identity\r\nContent-Length: 3\r\n\r\nabc";
            let (_, _, _, body) = read_request_bytes(raw).unwrap();
            assert_eq!(body, b"abc");
        }

        #[test]
        fn body_past_content_length_is_left_aside() {
            let raw = b"POST / HTTP/1.1\r\nContent-Length: 3\r\n\r\nabcEXTRA";
            let (_, _, _, body) = read_request_bytes(raw).unwrap();
            assert_eq!(body, b"abc");
        }

        #[test]
        fn header_size_line_and_count_are_bounded() {
            let mut huge_head = b"GET / HTTP/1.1\r\n".to_vec();
            huge_head.extend(std::iter::repeat_n(b'a', MAX_REQUEST_HEAD + 1));
            assert!(read_request_bytes(&huge_head).is_none());

            let mut long_line = b"GET / HTTP/1.1\r\nX-Long: ".to_vec();
            long_line.extend(std::iter::repeat_n(b'a', MAX_HEADER_LINE + 1));
            long_line.extend_from_slice(b"\r\n\r\n");
            assert!(read_request_bytes(&long_line).is_none());

            let mut many = b"GET / HTTP/1.1\r\n".to_vec();
            for index in 0..MAX_HEADERS + 1 {
                many.extend_from_slice(format!("X-H{index}: v\r\n").as_bytes());
            }
            many.extend_from_slice(b"\r\n");
            assert!(read_request_bytes(&many).is_none());
        }

        #[test]
        fn body_length_is_bounded() {
            let raw = format!(
                "POST / HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
                MAX_BODY + 1
            );
            assert!(read_request_bytes(raw.as_bytes()).is_none());
        }

        #[test]
        fn a_slow_or_stalled_client_is_cut_off() {
            let (mut client, server) = pair();
            let server_thread = std::thread::spawn(move || read_request(&server));
            client.write_all(b"GET / HTTP/1.1\r\nHost:").unwrap();
            client.shutdown(Shutdown::Write).unwrap();
            assert!(server_thread.join().unwrap().is_none());
        }

        fn respond_once(
            status: f64,
            body: &str,
            headers: Vec<(Rc<str>, Value)>,
        ) -> Result<Vec<u8>, String> {
            let (mut client, server) = pair();
            let id = store(&CONNECTIONS, server);
            let result = respond(&[
                Value::Number(id as f64),
                Value::Number(status),
                Value::str(body),
                Value::record(headers),
            ]);
            let mut out = Vec::new();
            let _ = client.read_to_end(&mut out);
            result.map(|_| out)
        }

        #[test]
        fn a_response_is_one_well_formed_message_with_safe_headers() {
            let out = respond_once(
                200.0,
                "hi",
                vec![
                    (Rc::from("x-test"), Value::str("1")),
                    (Rc::from("content-length"), Value::str("999")),
                    (Rc::from("connection"), Value::str("keep-alive")),
                    (Rc::from("transfer-encoding"), Value::str("chunked")),
                ],
            )
            .unwrap();
            let text = String::from_utf8(out).unwrap();
            assert!(text.starts_with("HTTP/1.1 200 OK\r\n"), "{text}");
            assert!(text.contains("x-test: 1\r\n"), "{text}");
            assert!(!text.contains("999"), "{text}");
            assert!(!text.to_lowercase().contains("keep-alive"), "{text}");
            assert!(!text.to_lowercase().contains("transfer-encoding"), "{text}");
            assert!(text.contains("connection: close\r\n"), "{text}");
            assert_eq!(text.matches("content-length:").count(), 1, "{text}");
            assert!(text.contains("content-length: 2\r\n"), "{text}");
            assert!(
                text.contains("content-type: text/plain; charset=utf-8\r\n"),
                "{text}"
            );
            assert!(text.ends_with("\r\n\r\nhi"), "{text}");
        }

        #[test]
        fn response_header_injection_and_bad_names_are_rejected() {
            assert!(respond_once(200.0, "", vec![(Rc::from("x"), Value::str("a\r\nb"))]).is_err());
            assert!(respond_once(200.0, "", vec![(Rc::from("x\r\ny"), Value::str("a"))]).is_err());
            assert!(respond_once(200.0, "", vec![(Rc::from("x y"), Value::str("a"))]).is_err());
            assert!(respond_once(200.0, "", vec![(Rc::from("x:"), Value::str("a"))]).is_err());
            assert!(respond_once(200.0, "", vec![(Rc::from(""), Value::str("a"))]).is_err());
        }

        #[test]
        fn content_type_is_kept_and_status_is_checked() {
            let out = respond_once(
                201.0,
                "{}",
                vec![(Rc::from("Content-Type"), Value::str("application/json"))],
            )
            .unwrap();
            let text = String::from_utf8(out).unwrap();
            assert!(text.starts_with("HTTP/1.1 201 Created\r\n"), "{text}");
            assert_eq!(
                text.to_lowercase().matches("content-type").count(),
                1,
                "{text}"
            );
            assert!(
                text.contains("Content-Type: application/json\r\n"),
                "{text}"
            );
            assert!(!text.contains("text/plain"), "{text}");
            assert!(respond_once(99.0, "", vec![]).is_err());
            assert!(respond_once(600.0, "", vec![]).is_err());
        }

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
            let key = [0x37, 0xfa, 0x21, 0x3d];
            out.extend_from_slice(&key);
            out.extend(payload.iter().zip(key.iter().cycle()).map(|(b, k)| b ^ k));
            out
        }

        fn text_frame(payload: &[u8]) -> Vec<u8> {
            masked(0x81, payload)
        }

        fn parsed(bytes: &[u8]) -> Frame {
            let (frame, used) = parse_frame(bytes, MAX_BODY).unwrap().unwrap();
            assert_eq!(used, bytes.len());
            frame
        }

        #[test]
        fn the_accept_key_matches_rfc_6455() {
            assert_eq!(
                accept_key("dGhlIHNhbXBsZSBub25jZQ=="),
                "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
            );
        }

        #[test]
        fn an_upgrade_needs_every_websocket_header() {
            let headers = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
                pairs
                    .iter()
                    .map(|(n, v)| (n.to_string(), v.to_string()))
                    .collect()
            };
            let good = [
                ("upgrade", "WebSocket"),
                ("connection", "keep-alive, Upgrade"),
                ("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ=="),
                ("sec-websocket-version", "13"),
            ];
            assert_eq!(
                upgrade_key("GET", &headers(&good)).as_deref(),
                Some("dGhlIHNhbXBsZSBub25jZQ==")
            );
            assert!(upgrade_key("POST", &headers(&good)).is_none());
            for missing in 0..good.len() {
                let mut partial = good.to_vec();
                partial.remove(missing);
                assert!(upgrade_key("GET", &headers(&partial)).is_none());
            }
            let mut old = good.to_vec();
            old[3] = ("sec-websocket-version", "8");
            assert!(upgrade_key("GET", &headers(&old)).is_none());
            let mut short = good.to_vec();
            short[2] = ("sec-websocket-key", "abc");
            assert!(upgrade_key("GET", &headers(&short)).is_none());
            let mut plain = good.to_vec();
            plain[1] = ("connection", "keep-alive");
            assert!(upgrade_key("GET", &headers(&plain)).is_none());
        }

        #[test]
        fn a_masked_frame_is_unmasked() {
            let rfc = [
                0x81, 0x85, 0x37, 0xfa, 0x21, 0x3d, 0x7f, 0x9f, 0x4d, 0x51, 0x58,
            ];
            assert_eq!(
                parsed(&rfc),
                Frame {
                    fin: true,
                    opcode: 0x1,
                    payload: b"Hello".to_vec()
                }
            );
            assert_eq!(
                parse_frame(&[0x81, 0x05, b'H', b'e', b'l', b'l', b'o'], MAX_BODY),
                Err(1002)
            );
        }

        #[test]
        fn every_length_form_is_read_and_partial_frames_wait() {
            for len in [0, 125, 126, 300, 0xffff, 0x10000, 70000] {
                let payload = vec![b'x'; len];
                let bytes = text_frame(&payload);
                let header = match len {
                    0..=125 => 6,
                    126..=0xffff => 8,
                    _ => 14,
                };
                assert_eq!(bytes.len(), header + len);
                assert_eq!(parsed(&bytes).payload, payload);
                for cut in [0, 1, 2, 3, header - 1, header, bytes.len() - 1] {
                    if cut < bytes.len() {
                        assert_eq!(
                            parse_frame(&bytes[..cut], MAX_BODY),
                            Ok(None),
                            "{len} {cut}"
                        );
                    }
                }
            }
        }

        #[test]
        fn server_frames_are_unmasked_with_the_shortest_length() {
            for (len, header) in [(5, 2), (126, 4), (0xffff, 4), (0x10000, 10)] {
                let bytes = frame_bytes(0x1, &vec![b'y'; len]);
                assert_eq!(bytes[0], 0x81);
                assert_eq!(bytes[1] & 0x80, 0);
                assert_eq!(bytes.len(), header + len);
            }
        }

        #[test]
        fn frames_the_parser_does_not_understand_are_refused_with_the_right_code() {
            assert_eq!(parse_frame(&masked(0xc1, b"x"), MAX_BODY), Err(1002));
            assert_eq!(parse_frame(&masked(0x91, b"x"), MAX_BODY), Err(1002));
            assert_eq!(parse_frame(&masked(0x83, b"x"), MAX_BODY), Err(1002));
            assert_eq!(parse_frame(&masked(0x8b, b"x"), MAX_BODY), Err(1002));
            assert_eq!(parse_frame(&masked(0x09, b"x"), MAX_BODY), Err(1002));
            assert_eq!(parse_frame(&masked(0x89, &[0; 126]), MAX_BODY), Err(1002));
            let mut huge = vec![0x81, 0x80 | 127];
            huge.extend_from_slice(&(1u64 << 63).to_be_bytes());
            assert_eq!(parse_frame(&huge, MAX_BODY), Err(1002));
            assert_eq!(parse_frame(&text_frame(&[0; 11]), 10), Err(1009));
        }

        fn socket(frames: &[u8], max: usize) -> (Open, TcpStream) {
            let (client, server) = pair();
            client
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            server.set_nonblocking(true).unwrap();
            let fd = server.as_raw_fd();
            let limits = Limits {
                max_body: max,
                ..Limits::default()
            };
            let mut made = None;
            let channel = crate::task::endpoint(|channel| {
                let socket = Arc::new(Socket {
                    stream: Arc::new(Mutex::new(Box::new(Patient {
                        tcp: server,
                        wait: None,
                    }) as Box<dyn Stream>)),
                    fd,
                    writing: Mutex::new(()),
                    closed: AtomicBool::new(false),
                    channel,
                    timeout: Duration::from_secs(5),
                });
                made = Some(Arc::clone(&socket));
                socket
            });
            let open = Open {
                socket: made.unwrap(),
                channel,
                reader: Reader::new(frames.to_vec(), &limits),
            };
            (open, client)
        }

        fn receive(open: &Open) -> Value {
            crate::task::receive(&[Value::Channel(Arc::clone(&open.channel))]).unwrap()
        }

        fn server_frame(client: &mut TcpStream) -> (u8, Vec<u8>) {
            let mut head = [0u8; 2];
            client.read_exact(&mut head).unwrap();
            assert_eq!(head[1] & 0x80, 0, "server frames are not masked");
            let mut payload = vec![0u8; usize::from(head[1] & 0x7f)];
            client.read_exact(&mut payload).unwrap();
            (head[0], payload)
        }

        fn closes_with(frames: &[u8], max: usize, code: u16) {
            let (mut open, mut client) = socket(frames, max);
            let farewell = open.reader.take(&open.socket, &open.channel).unwrap_err();
            open.socket.shut(farewell.as_deref(), false);
            assert!(matches!(receive(&open), Value::None));
            assert_eq!(
                server_frame(&mut client),
                (0x88, code.to_be_bytes().to_vec())
            );
            assert!(matches!(receive(&open), Value::None));
        }

        fn received(value: Value) -> String {
            match value {
                Value::Str(text) => text.to_string(),
                other => panic!("expected a string, got {other:?}"),
            }
        }

        #[test]
        fn fragments_are_reassembled_around_a_ping_that_gets_its_pong() {
            let mut frames = masked(0x01, b"Hel");
            frames.extend(masked(0x89, b"are you there"));
            frames.extend(masked(0x8a, b"unsolicited"));
            frames.extend(masked(0x00, b"lo, "));
            frames.extend(masked(0x80, b"world"));
            frames.extend(masked(0x82, b"binary text"));
            let (mut open, mut client) = socket(&frames, MAX_BODY);
            open.reader.take(&open.socket, &open.channel).unwrap();
            assert_eq!(received(receive(&open)), "Hello, world");
            assert_eq!(server_frame(&mut client), (0x8a, b"are you there".to_vec()));
            assert!(matches!(receive(&open), Value::Bytes(b) if b.as_slice() == b"binary text"));
            open.socket.send(&Value::str("back")).unwrap();
            assert_eq!(server_frame(&mut client), (0x81, b"back".to_vec()));
        }

        #[test]
        fn each_protocol_error_closes_with_its_code() {
            closes_with(&masked(0x81, &[0xff, 0xfe]), MAX_BODY, 1007);
            closes_with(&[0x81, 0x01, b'x'], MAX_BODY, 1002);
            closes_with(&masked(0xc1, b"x"), MAX_BODY, 1002);
            closes_with(&masked(0x83, b"x"), MAX_BODY, 1002);
            closes_with(&masked(0x80, b"x"), MAX_BODY, 1002);
            let mut twice = masked(0x01, b"a");
            twice.extend(masked(0x81, b"b"));
            closes_with(&twice, MAX_BODY, 1002);
            closes_with(&text_frame(&[b'x'; 11]), 10, 1009);
            let mut growing = masked(0x01, &[b'x'; 6]);
            growing.extend(masked(0x80, &[b'x'; 6]));
            closes_with(&growing, 10, 1009);
            closes_with(&masked(0x88, &[0x03]), MAX_BODY, 1002);
            closes_with(&masked(0x88, &999u16.to_be_bytes()), MAX_BODY, 1002);
            let mut bad_reason = 1000u16.to_be_bytes().to_vec();
            bad_reason.push(0xff);
            closes_with(&masked(0x88, &bad_reason), MAX_BODY, 1007);
        }

        #[test]
        fn a_close_from_the_peer_is_echoed_and_receive_returns_none() {
            let mut frames = text_frame(b"last");
            frames.extend(masked(0x88, &[0x03, 0xe8, b'b', b'y', b'e']));
            let (mut open, mut client) = socket(&frames, MAX_BODY);
            let farewell = open.reader.take(&open.socket, &open.channel).unwrap_err();
            assert_eq!(farewell, Some(vec![0x03, 0xe8]));
            open.socket.shut(farewell.as_deref(), false);
            assert_eq!(received(receive(&open)), "last");
            assert!(matches!(receive(&open), Value::None));
            assert_eq!(server_frame(&mut client), (0x88, vec![0x03, 0xe8]));
            let mut rest = Vec::new();
            client.read_to_end(&mut rest).unwrap();
            assert!(rest.is_empty());
            assert_eq!(
                open.socket.send(&Value::str("late")).unwrap_err(),
                "cannot send on a closed websocket"
            );
            assert_eq!(
                open.socket.send(&Value::Number(1.0)).unwrap_err(),
                "a websocket can only send a string or bytes, got number"
            );
        }

        #[test]
        fn a_silent_peer_is_pinged_then_closed_with_1001() {
            let hub = Hub::new(Limits::default()).unwrap();
            let (mut open, mut client) = socket(&[], MAX_BODY);
            open.reader.timeout = Duration::from_millis(100);
            let channel = Arc::clone(&open.channel);
            hub.park(Entry::Socket(open));
            std::thread::sleep(Duration::from_millis(150));
            hub.sweep();
            assert_eq!(server_frame(&mut client), (0x89, Vec::new()));
            std::thread::sleep(Duration::from_millis(150));
            hub.sweep();
            assert_eq!(
                server_frame(&mut client),
                (0x88, 1001u16.to_be_bytes().to_vec())
            );
            assert!(matches!(
                crate::task::receive(&[Value::Channel(channel)]).unwrap(),
                Value::None
            ));
        }

        #[test]
        fn a_peer_that_answers_the_ping_stays_open() {
            let hub = Hub::new(Limits::default()).unwrap();
            let (sender, _jobs) = std::sync::mpsc::sync_channel(1);
            let (mut open, mut client) = socket(&[], MAX_BODY);
            open.reader.timeout = Duration::from_millis(100);
            hub.park(Entry::Socket(open));
            std::thread::sleep(Duration::from_millis(150));
            hub.sweep();
            assert_eq!(server_frame(&mut client), (0x89, Vec::new()));
            client.write_all(&masked(0x8a, b"")).unwrap();
            client.write_all(&text_frame(b"awake")).unwrap();
            std::thread::sleep(Duration::from_millis(20));
            let entry = super::lock(&hub.entries).drain().next().unwrap().1;
            hub.readable(entry, &sender);
            std::thread::sleep(Duration::from_millis(60));
            hub.sweep();
            let Some(Entry::Socket(open)) = super::lock(&hub.entries).drain().next().map(|e| e.1)
            else {
                panic!("the socket was closed");
            };
            assert!(!open.reader.pinged);
            assert_eq!(received(receive(&open)), "awake");
        }
    }
}
