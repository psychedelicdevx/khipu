use std::cell::Cell;
use std::rc::Rc;

use crate::value::{Native, Value, equal};

pub const PRELUDE: &str = include_str!("prelude.kp");

pub static NATIVES: [Native; 45] = [
    native("upper#", upper),
    native("lower#", lower),
    native("trim#", trim),
    native("split#", split),
    native("chars#", chars),
    native("contains#", contains),
    native("starts_with#", starts_with),
    native("ends_with#", ends_with),
    native("replace#", replace),
    native("index_of#", index_of),
    native("slice#", slice),
    native("repeat#", repeat),
    native("to_number#", to_number),
    native("to_string#", to_string),
    native("pop#", pop),
    native("insert#", insert),
    native("remove#", remove),
    native("join#", join),
    native("reversed#", reversed),
    native("sorted#", sorted),
    native("sort_by_keys#", sort_by_keys),
    native("concat#", concat),
    native("math#", math),
    native("math2#", math2),
    native("random#", random),
    native("sum#", sum),
    native("min#", min),
    native("max#", max),
    native("first#", first),
    native("last#", last),
    native("lines#", lines),
    native("read_file#", read_file),
    native("read_bytes#", read_bytes),
    native("bytes_text#", bytes_text),
    native("string_bytes#", string_bytes),
    native("write_file#", write_file),
    native("append_file#", append_file),
    native("file_exists#", file_exists),
    native("delete_file#", delete_file),
    native("list_dir#", list_dir),
    native("input#", input),
    native("args#", args),
    native("env#", env),
    native("exit#", exit),
    native("contract#", contract),
];

static ARGS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

pub fn set_args(args: Vec<String>) {
    let _ = ARGS.set(args);
}

const fn native(name: &'static str, function: fn(&[Value]) -> Result<Value, String>) -> Native {
    Native { name, function }
}

fn text(value: &Value, what: &str) -> Result<Rc<String>, String> {
    match value {
        Value::Str(text) => Ok(Rc::clone(text)),
        other => Err(format!("{what} needs a string, got {}", other.type_name())),
    }
}

fn number(value: &Value, what: &str) -> Result<f64, String> {
    match value {
        Value::Number(n) => Ok(*n),
        other => Err(format!("{what} needs a number, got {}", other.type_name())),
    }
}

fn whole(value: &Value, what: &str) -> Result<i64, String> {
    let n = number(value, what)?;
    if n.fract() != 0.0 {
        return Err(format!("{what} needs a whole number, got {n}"));
    }
    Ok(n as i64)
}

fn items(value: &Value, what: &str) -> Result<Vec<Value>, String> {
    match value {
        Value::List(items) => Ok(items.borrow().to_vec()),
        other => Err(format!("{what} needs a list, got {}", other.type_name())),
    }
}

fn strings(parts: impl Iterator<Item = String>) -> Value {
    Value::list(parts.map(|part| Value::str(&part)).collect())
}

fn upper(args: &[Value]) -> Result<Value, String> {
    Ok(Value::str(&text(&args[0], "upper")?.to_uppercase()))
}

fn lower(args: &[Value]) -> Result<Value, String> {
    Ok(Value::str(&text(&args[0], "lower")?.to_lowercase()))
}

fn trim(args: &[Value]) -> Result<Value, String> {
    Ok(Value::str(text(&args[0], "trim")?.trim()))
}

fn split(args: &[Value]) -> Result<Value, String> {
    let whole = text(&args[0], "split")?;
    let separator = text(&args[1], "split")?;
    if separator.is_empty() {
        return Ok(strings(whole.chars().map(String::from)));
    }
    Ok(strings(whole.split(separator.as_str()).map(String::from)))
}

fn chars(args: &[Value]) -> Result<Value, String> {
    Ok(strings(text(&args[0], "chars")?.chars().map(String::from)))
}

fn contains(args: &[Value]) -> Result<Value, String> {
    match &args[0] {
        Value::Str(whole) => Ok(Value::Bool(
            whole.contains(text(&args[1], "contains")?.as_str()),
        )),
        list => Ok(Value::Bool(
            items(list, "contains")?
                .iter()
                .any(|item| equal(item, &args[1]) == Some(true)),
        )),
    }
}

fn starts_with(args: &[Value]) -> Result<Value, String> {
    let whole = text(&args[0], "starts_with")?;
    Ok(Value::Bool(
        whole.starts_with(text(&args[1], "starts_with")?.as_str()),
    ))
}

fn ends_with(args: &[Value]) -> Result<Value, String> {
    let whole = text(&args[0], "ends_with")?;
    Ok(Value::Bool(
        whole.ends_with(text(&args[1], "ends_with")?.as_str()),
    ))
}

fn replace(args: &[Value]) -> Result<Value, String> {
    let whole = text(&args[0], "replace")?;
    let from = text(&args[1], "replace")?;
    let to = text(&args[2], "replace")?;
    if from.is_empty() {
        return Ok(Value::Str(whole));
    }
    Ok(Value::str(&whole.replace(from.as_str(), &to)))
}

fn index_of(args: &[Value]) -> Result<Value, String> {
    let found = match &args[0] {
        Value::Str(whole) => {
            let part = text(&args[1], "index_of")?;
            whole
                .find(part.as_str())
                .map(|byte| whole[..byte].chars().count())
        }
        list => items(list, "index_of")?
            .iter()
            .position(|item| equal(item, &args[1]) == Some(true)),
    };
    Ok(found.map_or(Value::None, |at| Value::Number(at as f64)))
}

fn bounds(len: usize, start: &Value, end: &Value) -> Result<(usize, usize), String> {
    let clamp = |n: i64| {
        let n = if n < 0 { len as i64 + n } else { n };
        n.clamp(0, len as i64) as usize
    };
    let start = clamp(whole(start, "slice")?);
    let end = clamp(whole(end, "slice")?);
    Ok((start, end.max(start)))
}

fn slice(args: &[Value]) -> Result<Value, String> {
    match &args[0] {
        Value::Bytes(bytes) => {
            let (start, end) = bounds(bytes.len(), &args[1], &args[2])?;
            Ok(Value::Bytes(Rc::new(bytes[start..end].to_vec())))
        }
        Value::Str(whole) => {
            let chars: Vec<char> = whole.chars().collect();
            let (start, end) = bounds(chars.len(), &args[1], &args[2])?;
            Ok(Value::str(&chars[start..end].iter().collect::<String>()))
        }
        list => {
            let items = items(list, "slice")?;
            let (start, end) = bounds(items.len(), &args[1], &args[2])?;
            Ok(Value::list(items[start..end].to_vec()))
        }
    }
}

fn repeat(args: &[Value]) -> Result<Value, String> {
    let count = whole(&args[1], "repeat")?;
    if count < 0 {
        return Err(format!("repeat needs a count of 0 or more, got {count}"));
    }
    let piece = text(&args[0], "repeat")?;
    if (piece.len() as u128) * (count as u128) > 1 << 30 {
        return Err("repeat would make a string longer than 1 GB".to_string());
    }
    Ok(Value::str(&piece.repeat(count as usize)))
}

fn to_number(args: &[Value]) -> Result<Value, String> {
    let parsed = text(&args[0], "to_number")?.trim().parse::<f64>().ok();
    Ok(parsed
        .filter(|n| n.is_finite())
        .map_or(Value::None, Value::Number))
}

fn to_string(args: &[Value]) -> Result<Value, String> {
    Ok(Value::str(&args[0].to_string()))
}

fn pop(args: &[Value]) -> Result<Value, String> {
    match &args[0] {
        Value::List(items) => Ok(items.borrow_mut().pop().unwrap_or(Value::None)),
        other => Err(format!("pop needs a list, got {}", other.type_name())),
    }
}

fn position(list: &Value, index: &Value, extra: usize, what: &str) -> Result<usize, String> {
    let len = match list {
        Value::List(items) => items.borrow().len(),
        other => return Err(format!("{what} needs a list, got {}", other.type_name())),
    };
    let at = whole(index, what)?;
    if at < 0 || at as usize >= len + extra {
        return Err(format!(
            "{what} index {at} is outside a list of length {len}"
        ));
    }
    Ok(at as usize)
}

fn insert(args: &[Value]) -> Result<Value, String> {
    let at = position(&args[0], &args[1], 1, "insert")?;
    if let Value::List(items) = &args[0] {
        items.borrow_mut().insert(at, args[2].clone());
    }
    Ok(Value::None)
}

fn remove(args: &[Value]) -> Result<Value, String> {
    let at = position(&args[0], &args[1], 0, "remove")?;
    match &args[0] {
        Value::List(items) => Ok(items.borrow_mut().remove(at)),
        _ => Ok(Value::None),
    }
}

fn join(args: &[Value]) -> Result<Value, String> {
    let separator = text(&args[1], "join")?;
    let parts: Vec<String> = items(&args[0], "join")?
        .iter()
        .map(Value::to_string)
        .collect();
    Ok(Value::str(&parts.join(separator.as_str())))
}

fn reversed(args: &[Value]) -> Result<Value, String> {
    let mut items = items(&args[0], "reversed")?;
    items.reverse();
    Ok(Value::list(items))
}

fn order(a: &Value, b: &Value) -> Result<std::cmp::Ordering, String> {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => Ok(x.total_cmp(y)),
        (Value::Str(x), Value::Str(y)) => Ok(x.cmp(y)),
        (x, y) => Err(format!(
            "can only sort numbers or strings, got {} and {}",
            x.type_name(),
            y.type_name()
        )),
    }
}

fn sortable(values: &[Value]) -> Result<(), String> {
    if let Some(first) = values.first() {
        for value in &values[1..] {
            order(first, value)?;
        }
    }
    Ok(())
}

fn sort_values(values: &mut [Value]) -> Result<(), String> {
    sortable(values)?;
    values.sort_by(|a, b| order(a, b).unwrap_or(std::cmp::Ordering::Equal));
    Ok(())
}

fn sorted(args: &[Value]) -> Result<Value, String> {
    let mut items = items(&args[0], "sorted")?;
    sort_values(&mut items)?;
    Ok(Value::list(items))
}

fn sort_by_keys(args: &[Value]) -> Result<Value, String> {
    let items = items(&args[0], "sort_by")?;
    let keys = self::items(&args[1], "sort_by")?;
    sortable(&keys)?;
    let mut order_of: Vec<usize> = (0..items.len()).collect();
    order_of.sort_by(|&a, &b| order(&keys[a], &keys[b]).unwrap_or(std::cmp::Ordering::Equal));
    Ok(Value::list(
        order_of.into_iter().map(|at| items[at].clone()).collect(),
    ))
}

fn concat(args: &[Value]) -> Result<Value, String> {
    let mut joined = items(&args[0], "concat")?;
    joined.extend(items(&args[1], "concat")?);
    Ok(Value::list(joined))
}

fn math(args: &[Value]) -> Result<Value, String> {
    let name = text(&args[0], "math")?;
    let x = number(&args[1], &name)?;
    let result = match name.as_str() {
        "sqrt" if x < 0.0 => return Err(format!("sqrt needs a number of 0 or more, got {x}")),
        "sqrt" => x.sqrt(),
        "floor" => x.floor(),
        "ceil" => x.ceil(),
        "round" => x.round(),
        "abs" => x.abs(),
        "sin" => x.sin(),
        "cos" => x.cos(),
        "tan" => x.tan(),
        "log" if x <= 0.0 => return Err(format!("log needs a number above 0, got {x}")),
        "log" => x.ln(),
        "exp" => x.exp(),
        other => return Err(format!("unknown math function '{other}'")),
    };
    Ok(Value::Number(result))
}

fn math2(args: &[Value]) -> Result<Value, String> {
    let name = text(&args[0], "math")?;
    let x = number(&args[1], &name)?;
    let y = number(&args[2], &name)?;
    let result = match name.as_str() {
        "pow" => x.powf(y),
        "atan2" => x.atan2(y),
        other => return Err(format!("unknown math function '{other}'")),
    };
    Ok(Value::Number(result))
}

thread_local! {
    static SEED: Cell<u64> = Cell::new({
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |time| time.as_nanos() as u64);
        let here = &nanos as *const u64 as u64;
        (nanos ^ here.rotate_left(32)) | 1
    });
}

fn random(_: &[Value]) -> Result<Value, String> {
    let next = SEED.with(|seed| {
        let mut x = seed.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        seed.set(x);
        x
    });
    Ok(Value::Number((next >> 11) as f64 / (1u64 << 53) as f64))
}

fn numbers(value: &Value, what: &str) -> Result<Vec<f64>, String> {
    items(value, what)?
        .iter()
        .map(|item| number(item, what))
        .collect()
}

fn sum(args: &[Value]) -> Result<Value, String> {
    Ok(Value::Number(numbers(&args[0], "sum")?.iter().sum()))
}

fn min(args: &[Value]) -> Result<Value, String> {
    let found = numbers(&args[0], "min")?.into_iter().reduce(f64::min);
    Ok(found.map_or(Value::None, Value::Number))
}

fn max(args: &[Value]) -> Result<Value, String> {
    let found = numbers(&args[0], "max")?.into_iter().reduce(f64::max);
    Ok(found.map_or(Value::None, Value::Number))
}

fn first(args: &[Value]) -> Result<Value, String> {
    Ok(items(&args[0], "first")?
        .first()
        .cloned()
        .unwrap_or(Value::None))
}

fn last(args: &[Value]) -> Result<Value, String> {
    Ok(items(&args[0], "last")?
        .last()
        .cloned()
        .unwrap_or(Value::None))
}

fn lines(args: &[Value]) -> Result<Value, String> {
    Ok(strings(text(&args[0], "lines")?.lines().map(String::from)))
}

fn failed(action: &str, path: &str, error: std::io::Error) -> String {
    let reason = match error.kind() {
        std::io::ErrorKind::NotFound => "it does not exist".to_string(),
        std::io::ErrorKind::PermissionDenied => "permission denied".to_string(),
        std::io::ErrorKind::Unsupported => "files are not available here".to_string(),
        _ => error.to_string(),
    };
    format!("cannot {action} {path}: {reason}")
}

fn read_file(args: &[Value]) -> Result<Value, String> {
    let path = text(&args[0], "File.read")?;
    std::fs::read_to_string(path.as_str())
        .map(|contents| Value::str(&contents))
        .map_err(|error| failed("read", &path, error))
}

fn read_bytes(args: &[Value]) -> Result<Value, String> {
    let path = text(&args[0], "File.read_bytes")?;
    std::fs::read(path.as_str())
        .map(|contents| Value::Bytes(Rc::new(contents)))
        .map_err(|error| failed("read", &path, error))
}

fn bytes_text(args: &[Value]) -> Result<Value, String> {
    match &args[0] {
        Value::Bytes(bytes) => Ok(std::str::from_utf8(bytes).map_or(Value::None, Value::str)),
        other => Err(format!("to_string needs bytes, got {}", other.type_name())),
    }
}

fn string_bytes(args: &[Value]) -> Result<Value, String> {
    let text = text(&args[0], "to_bytes")?;
    Ok(Value::Bytes(Rc::new(text.as_bytes().to_vec())))
}

fn write_file(args: &[Value]) -> Result<Value, String> {
    let path = text(&args[0], "File.write")?;
    let contents = text(&args[1], "File.write")?;
    std::fs::write(path.as_str(), contents.as_bytes())
        .map(|_| Value::None)
        .map_err(|error| failed("write", &path, error))
}

fn append_file(args: &[Value]) -> Result<Value, String> {
    use std::io::Write;
    let path = text(&args[0], "File.append")?;
    let contents = text(&args[1], "File.append")?;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path.as_str())
        .and_then(|mut file| file.write_all(contents.as_bytes()))
        .map(|_| Value::None)
        .map_err(|error| failed("append to", &path, error))
}

fn file_exists(args: &[Value]) -> Result<Value, String> {
    let path = text(&args[0], "File.exists")?;
    Ok(Value::Bool(std::path::Path::new(path.as_str()).exists()))
}

fn delete_file(args: &[Value]) -> Result<Value, String> {
    let path = text(&args[0], "File.delete")?;
    std::fs::remove_file(path.as_str())
        .map(|_| Value::None)
        .map_err(|error| failed("delete", &path, error))
}

fn list_dir(args: &[Value]) -> Result<Value, String> {
    let path = text(&args[0], "File.list")?;
    let entries = std::fs::read_dir(path.as_str()).map_err(|error| failed("list", &path, error))?;
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    Ok(strings(names.into_iter()))
}

fn input(args: &[Value]) -> Result<Value, String> {
    use std::io::Write;
    let prompt = text(&args[0], "input")?;
    print!("{prompt}");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    match std::io::stdin().read_line(&mut line) {
        Ok(0) | Err(_) => Ok(Value::None),
        Ok(_) => Ok(Value::str(line.trim_end_matches(['\n', '\r']))),
    }
}

fn args(_: &[Value]) -> Result<Value, String> {
    Ok(strings(ARGS.get().cloned().unwrap_or_default().into_iter()))
}

fn env(args: &[Value]) -> Result<Value, String> {
    let name = text(&args[0], "Process.env")?;
    Ok(std::env::var(name.as_str()).map_or(Value::None, |value| Value::str(&value)))
}

#[cfg(target_arch = "wasm32")]
fn exit(_: &[Value]) -> Result<Value, String> {
    Err("Process.exit is not available in the playground".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn exit(args: &[Value]) -> Result<Value, String> {
    use std::io::Write;
    let code = whole(&args[0], "Process.exit")?;
    let _ = std::io::stdout().flush();
    crate::http::begin_shutdown();
    crate::http::drain(std::time::Duration::from_secs(10));
    std::process::exit(code.clamp(0, 255) as i32)
}

fn contract(args: &[Value]) -> Result<Value, String> {
    Err(text(&args[0], "contract")?.to_string())
}
