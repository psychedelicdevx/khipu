use std::rc::Rc;

use serde_json::Value as Json;

use crate::value::{Fields, Native, Value};

pub static NATIVES: [Native; 6] = [
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
        Value::Record(fields) => fields
            .borrow()
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect(),
        _ => Vec::new(),
    }
}

fn to_json(value: &Value) -> Result<Json, String> {
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

#[cfg(not(target_arch = "wasm32"))]
pub use server::{accept, fetch, listen, respond};

#[cfg(not(target_arch = "wasm32"))]
mod server {
    use std::collections::HashMap;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use super::{headers, record, text};
    use crate::value::Value;

    const MAX_BODY: usize = 10 * 1024 * 1024;

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

    fn read_request(stream: &TcpStream) -> Option<Incoming> {
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .ok()?;
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        let mut parts = line.split_whitespace();
        let method = parts.next()?.to_string();
        let target = parts.next()?.to_string();
        let mut found = Vec::new();
        let mut length = 0;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).ok()?;
            let header = header.trim_end();
            if header.is_empty() {
                break;
            }
            let (name, value) = header.split_once(':')?;
            let (name, value) = (name.trim().to_lowercase(), value.trim().to_string());
            if name == "content-length" {
                length = value.parse().ok()?;
            }
            found.push((name, value));
        }
        if length > MAX_BODY {
            return None;
        }
        let mut body = vec![0; length];
        reader.read_exact(&mut body).ok()?;
        Some((
            method,
            target,
            found,
            String::from_utf8_lossy(&body).into_owned(),
        ))
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
        let mut head = format!("HTTP/1.1 {status} {}\r\n", reason(status));
        let mut typed = false;
        for (name, value) in headers(&args[3]) {
            if name.contains(['\r', '\n']) || value.contains(['\r', '\n']) {
                return Err(format!("header '{}' contains a line break", name.trim()));
            }
            typed |= name.eq_ignore_ascii_case("content-type");
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        if !typed {
            head.push_str("content-type: text/plain; charset=utf-8\r\n");
        }
        head.push_str(&format!(
            "content-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        ));
        let _ = stream
            .write_all(head.as_bytes())
            .and_then(|_| stream.write_all(body.as_bytes()));
        Ok(Value::None)
    }
}
