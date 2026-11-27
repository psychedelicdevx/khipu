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
    use std::rc::Rc;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};

    use super::{headers, record, text};
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

    type Incoming = (String, String, Vec<(String, String)>, String);

    fn find_head_end(received: &[u8]) -> Option<(usize, usize)> {
        let find = |needle: &[u8]| received.windows(needle.len()).position(|w| w == needle);
        find(b"\r\n\r\n")
            .map(|at| (at, 4))
            .or_else(|| find(b"\n\n").map(|at| (at, 2)))
    }

    fn read_request(mut stream: &TcpStream) -> Option<Incoming> {
        stream.set_read_timeout(Some(READ_TIMEOUT)).ok()?;
        let deadline = Instant::now() + REQUEST_TIMEOUT;
        let mut buf = [0u8; 4096];
        let mut received = Vec::new();
        let (head_len, term_len) = loop {
            if let Some(found) = find_head_end(&received) {
                break found;
            }
            if received.len() > MAX_REQUEST_HEAD || Instant::now() > deadline {
                return None;
            }
            let read = stream.read(&mut buf).ok()?;
            if read == 0 {
                return None;
            }
            received.extend_from_slice(&buf[..read]);
        };
        let head = std::str::from_utf8(&received[..head_len]).ok()?;
        let mut body = received[head_len + term_len..].to_vec();
        let mut lines = head.split('\n');
        let raw_request_line = lines.next()?;
        let request_line = raw_request_line
            .strip_suffix('\r')
            .unwrap_or(raw_request_line);
        let mut parts = request_line.split_whitespace();
        let method = parts.next()?.to_string();
        let target = parts.next()?.to_string();
        let mut found = Vec::new();
        let mut length = 0usize;
        let mut seen_length = false;
        let mut count = 0usize;
        for raw in lines {
            let line = raw.strip_suffix('\r').unwrap_or(raw);
            if line.is_empty() {
                continue;
            }
            count += 1;
            if count > MAX_HEADERS || line.len() > MAX_HEADER_LINE {
                return None;
            }
            let (name, value) = line.split_once(':')?;
            let name = name.trim().to_lowercase();
            let value = value.trim();
            if name.is_empty() || !valid_header_name(&name) {
                return None;
            }
            if name == "transfer-encoding" && !value.eq_ignore_ascii_case("identity") {
                return None;
            }
            if name == "content-length" {
                if seen_length || value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                seen_length = true;
                length = value.parse().ok()?;
                if length > MAX_BODY {
                    return None;
                }
            }
            found.push((name, value.to_string()));
        }
        while body.len() < length {
            if Instant::now() > deadline {
                return None;
            }
            let read = stream.read(&mut buf).ok()?;
            if read == 0 {
                return None;
            }
            body.extend_from_slice(&buf[..read]);
        }
        body.truncate(length);
        Some((
            method,
            target,
            found,
            String::from_utf8_lossy(&body).into_owned(),
        ))
    }

    struct Conn {
        stream: SharedStream,
        buf: Vec<u8>,
    }

    trait Stream: Read + Write + Send {
        fn set_read_timeout(&self, dur: Option<Duration>) -> std::io::Result<()>;
        fn shutdown(&self, how: std::net::Shutdown) -> std::io::Result<()>;
    }

    impl Stream for TcpStream {
        fn set_read_timeout(&self, dur: Option<Duration>) -> std::io::Result<()> {
            TcpStream::set_read_timeout(self, dur)
        }
        fn shutdown(&self, how: std::net::Shutdown) -> std::io::Result<()> {
            TcpStream::shutdown(self, how)
        }
    }

    impl Stream for rustls::StreamOwned<rustls::ServerConnection, TcpStream> {
        fn set_read_timeout(&self, dur: Option<Duration>) -> std::io::Result<()> {
            self.sock.set_read_timeout(dur)
        }
        fn shutdown(&self, _how: std::net::Shutdown) -> std::io::Result<()> {
            Ok(())
        }
    }

    enum ReadReq {
        Request(Incoming, bool),
        Closed,
        Bad,
    }

    fn read_request_conn(conn: &mut Conn, limits: Limits, first: bool) -> ReadReq {
        let timeout = if first {
            limits.read_timeout
        } else {
            limits.idle_timeout
        };
        let _ = conn
            .stream
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .set_read_timeout(Some(timeout));
        let deadline = Instant::now() + limits.request_timeout;
        let mut scratch = [0u8; 4096];
        let (head_len, term_len) = loop {
            if let Some(found) = find_head_end(&conn.buf) {
                break found;
            }
            if conn.buf.len() > limits.max_request_head || Instant::now() > deadline {
                return ReadReq::Bad;
            }
            match conn
                .stream
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .read(&mut scratch)
            {
                Ok(0) => {
                    return if conn.buf.is_empty() {
                        ReadReq::Closed
                    } else {
                        ReadReq::Bad
                    };
                }
                Ok(read) => conn.buf.extend_from_slice(&scratch[..read]),
                Err(_) => {
                    return if conn.buf.is_empty() {
                        ReadReq::Closed
                    } else {
                        ReadReq::Bad
                    };
                }
            }
        };
        let head = match std::str::from_utf8(&conn.buf[..head_len]) {
            Ok(head) => head.to_string(),
            Err(_) => return ReadReq::Bad,
        };
        let mut lines = head.split('\n');
        let raw_request_line = match lines.next() {
            Some(line) => line,
            None => return ReadReq::Bad,
        };
        let request_line = raw_request_line
            .strip_suffix('\r')
            .unwrap_or(raw_request_line);
        let mut parts = request_line.split_whitespace();
        let method = match parts.next() {
            Some(method) => method.to_string(),
            None => return ReadReq::Bad,
        };
        let target = match parts.next() {
            Some(target) => target.to_string(),
            None => return ReadReq::Bad,
        };
        let version = parts.next().unwrap_or("HTTP/1.1").to_string();
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
                return ReadReq::Bad;
            }
            let Some((name, value)) = line.split_once(':') else {
                return ReadReq::Bad;
            };
            let name = name.trim().to_lowercase();
            let value = value.trim();
            if name.is_empty() || !valid_header_name(&name) {
                return ReadReq::Bad;
            }
            if name == "transfer-encoding" && !value.eq_ignore_ascii_case("identity") {
                return ReadReq::Bad;
            }
            if name == "content-length" {
                if seen_length || value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                    return ReadReq::Bad;
                }
                seen_length = true;
                length = match value.parse() {
                    Ok(length) => length,
                    Err(_) => return ReadReq::Bad,
                };
                if length > limits.max_body {
                    return ReadReq::Bad;
                }
            }
            if name == "connection" {
                connection = value.to_lowercase();
            }
            found.push((name, value.to_string()));
        }
        let body_start = head_len + term_len;
        let total = body_start + length;
        while conn.buf.len() < total {
            if Instant::now() > deadline {
                return ReadReq::Bad;
            }
            match conn
                .stream
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .read(&mut scratch)
            {
                Ok(0) => return ReadReq::Bad,
                Ok(read) => conn.buf.extend_from_slice(&scratch[..read]),
                Err(_) => return ReadReq::Bad,
            }
        }
        let body = conn.buf[body_start..total].to_vec();
        conn.buf.drain(..total);
        let keep_alive = if connection.split(',').any(|c| c.trim() == "close") {
            false
        } else if connection.split(',').any(|c| c.trim() == "keep-alive") {
            true
        } else {
            version != "HTTP/1.0"
        };
        ReadReq::Request(
            (
                method,
                target,
                found,
                String::from_utf8_lossy(&body).into_owned(),
            ),
            keep_alive,
        )
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
                        Value::str(&body),
                    ]));
                }
                None => {
                    let mut stream = stream;
                    let _ = stream.write_all(
                        b"HTTP/1.1 400 Bad Request\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                    );
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
        write_response(&mut stream, status, &body, &args[3], false)?;
        Ok(Value::None)
    }

    fn write_response<S: Write + ?Sized>(
        stream: &mut S,
        status: u16,
        body: &str,
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
        response.extend_from_slice(body.as_bytes());
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
                let _ = stream
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .shutdown(std::net::Shutdown::Both);
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

    fn response_parts(value: &Value) -> Result<(u16, String, Value), String> {
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
        let Some(Value::Str(body)) = field("body") else {
            return Err("a handler must return a Response with a string body".to_string());
        };
        let headers = field("headers").unwrap_or_else(|| Value::record(Vec::new()));
        Ok((status as u16, body.to_string(), headers))
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

    struct Socket {
        stream: SharedStream,
        buf: Mutex<Vec<u8>>,
        closed: std::sync::atomic::AtomicBool,
        max: usize,
        timeout: Duration,
    }

    impl Socket {
        fn write(&self, opcode: u8, payload: &[u8]) -> std::io::Result<()> {
            let mut stream = self.stream.lock().unwrap_or_else(|p| p.into_inner());
            stream.write_all(&frame_bytes(opcode, payload))?;
            stream.flush()
        }

        fn close(&self, payload: &[u8]) {
            if !self.closed.swap(true, Ordering::SeqCst) {
                let _ = self.write(0x8, payload);
            }
            let _ = self
                .stream
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .shutdown(std::net::Shutdown::Both);
        }

        fn fill(&self, buf: &mut Vec<u8>, deadline: Instant) -> Result<(), Option<Vec<u8>>> {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(code(1001));
            }
            let mut stream = self.stream.lock().unwrap_or_else(|p| p.into_inner());
            let _ = stream.set_read_timeout(Some(left));
            let mut scratch = [0u8; 16 * 1024];
            match stream.read(&mut scratch) {
                Ok(0) => Err(None),
                Ok(read) => {
                    buf.extend_from_slice(&scratch[..read]);
                    Ok(())
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    Err(code(1001))
                }
                Err(_) => Err(None),
            }
        }

        fn message(&self, buf: &mut Vec<u8>) -> Result<String, Option<Vec<u8>>> {
            let mut partial: Option<(u8, Vec<u8>)> = None;
            let mut deadline = Instant::now() + self.timeout;
            loop {
                let frame = match parse_frame(buf, self.max) {
                    Err(error) => return Err(code(error)),
                    Ok(None) => {
                        self.fill(buf, deadline)?;
                        continue;
                    }
                    Ok(Some((frame, used))) => {
                        buf.drain(..used);
                        deadline = Instant::now() + self.timeout;
                        frame
                    }
                };
                match frame.opcode {
                    0x8 => return Err(Some(close_reply(&frame.payload).map_err(code)?)),
                    0x9 => self.write(0xa, &frame.payload).map_err(|_| None)?,
                    0xa => {}
                    opcode => {
                        let (kind, mut data) = match (opcode, partial.take()) {
                            (0x0, Some(started)) => started,
                            (0x1 | 0x2, None) => (opcode, Vec::new()),
                            _ => return Err(code(1002)),
                        };
                        if data.len() + frame.payload.len() > self.max {
                            return Err(code(1009));
                        }
                        data.extend_from_slice(&frame.payload);
                        if !frame.fin {
                            partial = Some((kind, data));
                            continue;
                        }
                        return String::from_utf8(data)
                            .map_err(|_| code(if kind == 0x1 { 1007 } else { 1003 }));
                    }
                }
            }
        }
    }

    impl crate::task::Endpoint for Socket {
        fn send(&self, value: &Value) -> Result<(), String> {
            let Value::Str(message) = value else {
                return Err(format!(
                    "a websocket can only send a string, got {}",
                    value.type_name()
                ));
            };
            if self.closed.load(Ordering::SeqCst) || self.write(0x1, message.as_bytes()).is_err() {
                return Err("cannot send on a closed websocket".to_string());
            }
            Ok(())
        }

        fn receive(&self) -> Value {
            if self.closed.load(Ordering::SeqCst) {
                return Value::None;
            }
            let mut buf = self.buf.lock().unwrap_or_else(|p| p.into_inner());
            match self.message(&mut buf) {
                Ok(message) => Value::str(&message),
                Err(Some(payload)) => {
                    self.close(&payload);
                    Value::None
                }
                Err(None) => {
                    self.closed.store(true, Ordering::SeqCst);
                    self.close(&[]);
                    Value::None
                }
            }
        }
    }

    fn websocket(
        stream: SharedStream,
        vm: &mut Vm,
        handler: Value,
        key: &str,
        leftover: Vec<u8>,
        limits: Limits,
    ) {
        let head = format!(
            "HTTP/1.1 101 Switching Protocols\r\nupgrade: websocket\r\nconnection: Upgrade\r\nsec-websocket-accept: {}\r\n\r\n",
            accept_key(key)
        );
        {
            let mut out = stream.lock().unwrap_or_else(|p| p.into_inner());
            if out
                .write_all(head.as_bytes())
                .and_then(|_| out.flush())
                .is_err()
            {
                return;
            }
        }
        let socket = std::sync::Arc::new(Socket {
            stream,
            buf: Mutex::new(leftover),
            closed: std::sync::atomic::AtomicBool::new(false),
            max: limits.max_body,
            timeout: limits.idle_timeout.saturating_mul(12),
        });
        let value = crate::task::endpoint(socket.clone());
        let status: u16 = match vm.call(handler, vec![value]) {
            Ok(_) => 1000,
            Err(error) => {
                eprintln!("{}", crate::task::render(&error));
                1011
            }
        };
        socket.close(&status.to_be_bytes());
    }

    const WORKER_STACK: usize = 16 * 1024 * 1024;
    const WORKERS_PER_CORE: usize = 8;
    const MIN_WORKERS: usize = 16;
    const MAX_WORKERS: usize = 128;
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
        body: &str,
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
            ("body", Value::str(body)),
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
        limits: Limits,
    }

    fn answer(
        vm: &mut Vm,
        handler: &Value,
        request: Value,
        stream: SharedStream,
        keep_alive: bool,
        upgrade: Option<Upgrade>,
    ) {
        let returned = match vm.call(handler.clone(), vec![request]) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("{}", crate::task::render(&error));
                write_response(
                    stream.lock().unwrap_or_else(|p| p.into_inner()).as_mut(),
                    500,
                    INTERNAL_ERROR,
                    &Value::record(Vec::new()),
                    keep_alive,
                )
                .ok();
                return;
            }
        };
        if let Some(socket_handler) = marker(&returned, "khipu_websocket") {
            match upgrade {
                Some(upgrade) => websocket(
                    stream,
                    vm,
                    socket_handler,
                    &upgrade.key,
                    upgrade.leftover,
                    upgrade.limits,
                ),
                None => {
                    let _ = write_response(
                        stream.lock().unwrap_or_else(|p| p.into_inner()).as_mut(),
                        426,
                        "this address only speaks websockets",
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
                (500, INTERNAL_ERROR.to_string(), Value::record(Vec::new()))
            }
        };
        if write_response(
            stream.lock().unwrap_or_else(|p| p.into_inner()).as_mut(),
            status,
            &response_body,
            &response_headers,
            keep_alive,
        )
        .is_err()
        {
            let _ = write_response(
                stream.lock().unwrap_or_else(|p| p.into_inner()).as_mut(),
                500,
                INTERNAL_ERROR,
                &Value::record(Vec::new()),
                keep_alive,
            );
        }
    }

    fn serve_connection(
        stream: Box<dyn Stream>,
        vm: &mut Vm,
        handler: &Value,
        request_def: Option<&Rc<StructDef>>,
        limits: Limits,
    ) {
        let mut conn = Conn {
            stream: std::sync::Arc::new(std::sync::Mutex::new(stream)),
            buf: Vec::new(),
        };
        let mut served = 0usize;
        ACTIVE.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        loop {
            match read_request_conn(&mut conn, limits, served == 0) {
                ReadReq::Closed => break,
                ReadReq::Bad => {
                    let _ = conn.stream.lock().unwrap_or_else(|p| p.into_inner()).write_all(
                        b"HTTP/1.1 400 Bad Request\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                    );
                    break;
                }
                ReadReq::Request((method, target, found, body), keep_alive) => {
                    let upgrade = upgrade_key(&method, &found).map(|key| Upgrade {
                        key,
                        leftover: std::mem::take(&mut conn.buf),
                        limits,
                    });
                    let request = build_request(request_def, &method, &target, found, &body);
                    let more = keep_alive && upgrade.is_none() && served + 1 < limits.max_requests;
                    answer(
                        vm,
                        handler,
                        request,
                        std::sync::Arc::clone(&conn.stream),
                        more,
                        upgrade,
                    );
                    served += 1;
                    if !more {
                        break;
                    }
                }
            }
        }
        ACTIVE.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }

    fn http_worker(
        packed: crate::task::Graph,
        receiver: std::sync::Arc<std::sync::Mutex<std::sync::mpsc::Receiver<Box<dyn Stream>>>>,
        dir: std::path::PathBuf,
        limits: Limits,
    ) {
        let handler = crate::task::unpack(packed).remove(0);
        let handler_globals = match &handler {
            Value::Function(closure) => Some(Rc::clone(&closure.globals)),
            _ => None,
        };
        let mut vm = Vm::task(&dir);
        vm.preemptive = false;
        if let Some(globals) = &handler_globals {
            vm.globals = Rc::clone(globals);
        }
        let request_def = handler_globals
            .as_ref()
            .and_then(|g| struct_def(g, "Request"));
        loop {
            let stream = receiver
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .recv();
            let Ok(stream) = stream else { break };
            serve_connection(stream, &mut vm, &handler, request_def.as_ref(), limits);
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
        tls: Option<std::sync::Arc<rustls::ServerConfig>>,
        port: u16,
        scheme: &str,
    ) -> Result<Value, String> {
        let _ = listener.set_nonblocking(true);
        let flag = shutdown_flag();
        let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, flag.clone());
        let _ = signal_hook::flag::register(signal_hook::consts::SIGTERM, flag.clone());
        println!("listening on {scheme}://localhost:{port}");
        let dir = crate::task::program_dir();
        let workers = std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .saturating_mul(WORKERS_PER_CORE)
            .clamp(MIN_WORKERS, MAX_WORKERS);
        let bound = workers.saturating_mul(2).max(1);
        let (sender, receiver) = std::sync::mpsc::sync_channel::<Box<dyn Stream>>(bound);
        let receiver = std::sync::Arc::new(std::sync::Mutex::new(receiver));
        for _ in 0..workers {
            let packed = crate::task::pack(std::slice::from_ref(&handler));
            let receiver = std::sync::Arc::clone(&receiver);
            let dir = dir.clone();
            std::thread::Builder::new()
                .name("khipu-http".to_string())
                .stack_size(WORKER_STACK)
                .spawn(move || http_worker(packed, receiver, dir, limits))
                .expect("could not start a request worker");
        }
        loop {
            if flag.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
            match listener.accept() {
                Ok((stream, _)) => {
                    let _ = stream.set_nonblocking(false);
                    if flag.load(std::sync::atomic::Ordering::SeqCst) {
                        let _ = stream.shutdown(std::net::Shutdown::Both);
                    } else if let Some(config) = &tls {
                        match rustls::ServerConnection::new(std::sync::Arc::clone(config)) {
                            Ok(conn) => {
                                let _ =
                                    sender.send(Box::new(rustls::StreamOwned::new(conn, stream)));
                            }
                            Err(error) => eprintln!("TLS handshake setup failed: {error}"),
                        }
                    } else {
                        let _ = sender.send(Box::new(stream));
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(error) => {
                    eprintln!("could not accept a connection: {error}");
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }
        drop(sender);
        drain(DRAIN_TIMEOUT);
        Ok(Value::None)
    }

    #[cfg(test)]
    mod tests {
        use std::io::{Read, Write};
        use std::net::{Shutdown, TcpListener, TcpStream};
        use std::rc::Rc;

        use super::Incoming;
        use super::read_request;
        use super::{
            CONNECTIONS, MAX_BODY, MAX_HEADER_LINE, MAX_HEADERS, MAX_REQUEST_HEAD, respond, store,
        };
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
            assert_eq!(body, "hello");
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
                (method.as_str(), target.as_str(), body.as_str()),
                ("GET", "/x", "abc")
            );
        }

        #[test]
        fn accepts_lf_only_line_endings() {
            let raw = b"GET /x HTTP/1.1\nHost: h\nContent-Length: 3\n\nabc";
            let (_, _, _, body) = read_request_bytes(raw).unwrap();
            assert_eq!(body, "abc");
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
            assert_eq!(body, "abc");
        }

        #[test]
        fn body_past_content_length_is_left_aside() {
            let raw = b"POST / HTTP/1.1\r\nContent-Length: 3\r\n\r\nabcEXTRA";
            let (_, _, _, body) = read_request_bytes(raw).unwrap();
            assert_eq!(body, "abc");
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
    }
}
