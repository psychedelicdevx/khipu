use std::rc::Rc;

use serde_json::Value as Json;

use crate::value::{Fields, Native, Value};

pub static NATIVES: [Native; 7] = [
    Native {
        name: "serve#",
        function: serve,
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

#[cfg(not(target_arch = "wasm32"))]
pub use server::{accept, fetch, listen, respond, serve};

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
    use crate::value::{GlobalStore, Instance, StructDef, Value};
    use crate::vm::Vm;

    const MAX_BODY: usize = 10 * 1024 * 1024;
    const MAX_REQUEST_HEAD: usize = 16 * 1024;
    const MAX_HEADER_LINE: usize = 8 * 1024;
    const MAX_HEADERS: usize = 100;
    const READ_TIMEOUT: Duration = Duration::from_secs(10);
    const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

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
        stream: TcpStream,
        buf: Vec<u8>,
    }

    enum ReadReq {
        Request(Incoming, bool),
        Closed,
        Bad,
    }

    fn read_request_conn(conn: &mut Conn) -> ReadReq {
        let _ = conn.stream.set_read_timeout(Some(READ_TIMEOUT));
        let deadline = Instant::now() + REQUEST_TIMEOUT;
        let mut scratch = [0u8; 4096];
        let (head_len, term_len) = loop {
            if let Some(found) = find_head_end(&conn.buf) {
                break found;
            }
            if conn.buf.len() > MAX_REQUEST_HEAD || Instant::now() > deadline {
                return ReadReq::Bad;
            }
            match conn.stream.read(&mut scratch) {
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
            if count > MAX_HEADERS || line.len() > MAX_HEADER_LINE {
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
                if length > MAX_BODY {
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
            match conn.stream.read(&mut scratch) {
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

    fn write_response(
        stream: &mut TcpStream,
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

    const WORKER_STACK: usize = 16 * 1024 * 1024;

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

    fn answer(
        vm: &mut Vm,
        handler: &Value,
        request: Value,
        stream: &mut TcpStream,
        keep_alive: bool,
    ) {
        let (status, response_body, response_headers) =
            match vm.call(handler.clone(), vec![request]) {
                Ok(value) => match response_parts(&value) {
                    Ok(response) => response,
                    Err(message) => {
                        eprintln!("in a request handler: {message}");
                        (500, INTERNAL_ERROR.to_string(), Value::record(Vec::new()))
                    }
                },
                Err(error) => {
                    eprintln!("{}", crate::task::render(&error));
                    (500, INTERNAL_ERROR.to_string(), Value::record(Vec::new()))
                }
            };
        if write_response(
            stream,
            status,
            &response_body,
            &response_headers,
            keep_alive,
        )
        .is_err()
        {
            let _ = write_response(
                stream,
                500,
                INTERNAL_ERROR,
                &Value::record(Vec::new()),
                keep_alive,
            );
        }
    }

    fn serve_connection(
        stream: TcpStream,
        vm: &mut Vm,
        handler: &Value,
        request_def: Option<&Rc<StructDef>>,
    ) {
        let mut conn = Conn {
            stream,
            buf: Vec::new(),
        };
        loop {
            match read_request_conn(&mut conn) {
                ReadReq::Closed => break,
                ReadReq::Bad => {
                    let _ = conn.stream.write_all(
                        b"HTTP/1.1 400 Bad Request\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                    );
                    break;
                }
                ReadReq::Request((method, target, found, body), keep_alive) => {
                    let request = build_request(request_def, &method, &target, found, &body);
                    answer(vm, handler, request, &mut conn.stream, keep_alive);
                    if !keep_alive {
                        break;
                    }
                }
            }
        }
    }

    fn http_worker(
        packed: crate::task::Graph,
        receiver: std::sync::Arc<std::sync::Mutex<std::sync::mpsc::Receiver<TcpStream>>>,
        dir: std::path::PathBuf,
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
            serve_connection(stream, &mut vm, &handler, request_def.as_ref());
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
        println!("listening on http://localhost:{port}");
        let dir = crate::task::program_dir();
        let workers = std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .max(1);
        let bound = workers.saturating_mul(2).max(1);
        let (sender, receiver) = std::sync::mpsc::sync_channel::<TcpStream>(bound);
        let receiver = std::sync::Arc::new(std::sync::Mutex::new(receiver));
        for _ in 0..workers {
            let packed = crate::task::pack(std::slice::from_ref(&handler));
            let receiver = std::sync::Arc::clone(&receiver);
            let dir = dir.clone();
            std::thread::Builder::new()
                .name("khipu-http".to_string())
                .stack_size(WORKER_STACK)
                .spawn(move || http_worker(packed, receiver, dir))
                .expect("could not start a request worker");
        }
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    let _ = sender.send(stream);
                }
                Err(error) => {
                    eprintln!("could not accept a connection: {error}");
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }
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
