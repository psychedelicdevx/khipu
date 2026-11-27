use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

use crate::compiler::Proto;
use crate::gc::{self, Tracked};
use crate::parser::Op;
use crate::task::{self, Channel};

pub type NativeFn = fn(&[Value]) -> Result<Value, String>;
pub type Fields = Vec<(Rc<str>, Value)>;
pub type Cell = Rc<RefCell<Value>>;
pub type Shape = Rc<[Rc<str>]>;

pub struct Record {
    pub shape: Shape,
    pub values: Vec<Value>,
}

impl Record {
    pub fn from_fields(fields: Fields) -> Record {
        let (names, values): (Vec<Rc<str>>, Vec<Value>) = fields.into_iter().unzip();
        Record {
            shape: names.into(),
            values,
        }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn position(&self, name: &str) -> Option<usize> {
        self.shape.iter().position(|field| &**field == name)
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        self.values.get(self.position(name)?).cloned()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Rc<str>, &Value)> {
        self.shape.iter().zip(&self.values)
    }
}

#[derive(Default)]
pub struct FieldCache(RefCell<Option<(Shape, u32)>>);

impl FieldCache {
    pub fn position(&self, shape: &Shape, name: &str) -> Option<usize> {
        if let Some((cached, at)) = &*self.0.borrow()
            && Rc::ptr_eq(cached, shape)
            && &*shape[*at as usize] == name
        {
            return Some(*at as usize);
        }
        let at = shape.iter().position(|field| &**field == name)?;
        *self.0.borrow_mut() = Some((Rc::clone(shape), at as u32));
        Some(at)
    }
}

#[derive(Clone)]
pub enum List {
    Values(Vec<Value>),
    Numbers(Vec<f64>),
    Bools(Vec<bool>),
    Range(i64, i64),
}

impl Default for List {
    fn default() -> Self {
        List::Numbers(Vec::new())
    }
}

impl List {
    pub fn from_values(values: Vec<Value>) -> List {
        if values.iter().all(|v| matches!(v, Value::Number(_))) {
            return List::Numbers(
                values
                    .iter()
                    .map(|v| if let Value::Number(n) = v { *n } else { 0.0 })
                    .collect(),
            );
        }
        if !values.is_empty() && values.iter().all(|v| matches!(v, Value::Bool(_))) {
            return List::Bools(
                values
                    .iter()
                    .map(|v| matches!(v, Value::Bool(true)))
                    .collect(),
            );
        }
        List::Values(values)
    }

    pub fn len(&self) -> usize {
        match self {
            List::Values(items) => items.len(),
            List::Numbers(items) => items.len(),
            List::Bools(items) => items.len(),
            List::Range(start, end) => end.saturating_sub(*start).max(0) as usize,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, at: usize) -> Option<Value> {
        match self {
            List::Values(items) => items.get(at).cloned(),
            List::Numbers(items) => items.get(at).map(|n| Value::Number(*n)),
            List::Bools(items) => items.get(at).map(|b| Value::Bool(*b)),
            List::Range(start, _) => {
                (at < self.len()).then(|| Value::Number((start + at as i64) as f64))
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = Value> + '_ {
        (0..self.len()).map(move |at| self.get(at).unwrap_or(Value::None))
    }

    pub fn to_vec(&self) -> Vec<Value> {
        self.iter().collect()
    }

    pub fn visit(&self, visit: &mut impl FnMut(&Value)) {
        if let List::Values(items) = self {
            items.iter().for_each(visit);
        }
    }

    fn widen(&mut self) -> &mut Vec<Value> {
        if !matches!(self, List::Values(_)) {
            *self = List::Values(self.to_vec());
        }
        match self {
            List::Values(items) => items,
            _ => unreachable!("widened above"),
        }
    }

    pub fn push(&mut self, value: Value) {
        match (&mut *self, &value) {
            (List::Numbers(items), Value::Number(n)) => items.push(*n),
            (List::Bools(items), Value::Bool(b)) => items.push(*b),
            (List::Numbers(items), Value::Bool(b)) if items.is_empty() => {
                *self = List::Bools(vec![*b]);
            }
            (List::Values(items), _) => items.push(value),
            _ => self.widen().push(value),
        }
    }

    pub fn set(&mut self, at: usize, value: Value) {
        match (&mut *self, &value) {
            (List::Numbers(items), Value::Number(n)) => items[at] = *n,
            (List::Bools(items), Value::Bool(b)) => items[at] = *b,
            (List::Values(items), _) => items[at] = value,
            _ => self.widen()[at] = value,
        }
    }

    pub fn pop(&mut self) -> Option<Value> {
        match self {
            List::Values(items) => items.pop(),
            List::Numbers(items) => items.pop().map(Value::Number),
            List::Bools(items) => items.pop().map(Value::Bool),
            List::Range(..) => self.widen().pop(),
        }
    }

    pub fn insert(&mut self, at: usize, value: Value) {
        match (&mut *self, &value) {
            (List::Numbers(items), Value::Number(n)) => items.insert(at, *n),
            (List::Bools(items), Value::Bool(b)) => items.insert(at, *b),
            (List::Values(items), _) => items.insert(at, value),
            _ => self.widen().insert(at, value),
        }
    }

    pub fn remove(&mut self, at: usize) -> Value {
        match self {
            List::Values(items) => items.remove(at),
            List::Numbers(items) => Value::Number(items.remove(at)),
            List::Bools(items) => Value::Bool(items.remove(at)),
            List::Range(..) => self.widen().remove(at),
        }
    }

    pub fn numbers(&self) -> Option<Vec<f64>> {
        match self {
            List::Numbers(items) => Some(items.clone()),
            List::Range(start, end) => {
                Some((*start..(*end).max(*start)).map(|n| n as f64).collect())
            }
            _ => None,
        }
    }
}

#[derive(Clone)]
pub enum Value {
    Number(f64),
    Bool(bool),
    None,
    Str(Rc<String>),
    Bytes(Rc<Vec<u8>>),
    List(Rc<RefCell<List>>),
    Record(Rc<RefCell<Record>>),
    Function(Rc<Closure>),
    Native(&'static Native),
    Cell(Cell),
    StructDef(Rc<StructDef>),
    Instance(Rc<Instance>),
    EnumDef(Rc<EnumDef>),
    Enum(Rc<EnumValue>),
    Channel(Arc<Channel>),
}

pub struct Native {
    pub name: &'static str,
    pub function: NativeFn,
}

pub struct StructDef {
    pub name: Rc<str>,
    pub fields: Rc<[Rc<str>]>,
    pub methods: Vec<(Rc<str>, Value)>,
}

pub struct Instance {
    pub def: Rc<StructDef>,
    pub fields: RefCell<Vec<Value>>,
}

pub struct EnumDef {
    pub name: Rc<str>,
    pub variants: Vec<(Rc<str>, usize)>,
}

pub struct EnumValue {
    pub def: Rc<EnumDef>,
    pub variant: usize,
    pub values: Vec<Value>,
}

pub struct Closure {
    pub proto: Rc<Proto>,
    pub captures: Vec<Cell>,
    pub globals: GlobalStore,
}

#[derive(Default)]
pub struct Globals {
    pub values: Vec<Option<Value>>,
    pub names: Vec<String>,
}

pub type GlobalStore = Rc<RefCell<Globals>>;

impl Globals {
    pub fn store(self) -> GlobalStore {
        let store = Rc::new(RefCell::new(self));
        gc::track(Tracked::Globals(Rc::downgrade(&store)));
        store
    }
}

impl Value {
    pub fn record(fields: Fields) -> Value {
        Value::shaped(Record::from_fields(fields))
    }

    pub fn shaped(record: Record) -> Value {
        let record = Rc::new(RefCell::new(record));
        gc::track(Tracked::Record(Rc::downgrade(&record)));
        Value::Record(record)
    }

    pub fn closure(closure: Closure) -> Value {
        let closure = Rc::new(closure);
        gc::track(Tracked::Closure(Rc::downgrade(&closure)));
        Value::Function(closure)
    }

    pub fn instance(instance: Instance) -> Value {
        let instance = Rc::new(instance);
        gc::track(Tracked::Instance(Rc::downgrade(&instance)));
        Value::Instance(instance)
    }

    pub fn enumeration(value: EnumValue) -> Value {
        let value = Rc::new(value);
        gc::track(Tracked::Enum(Rc::downgrade(&value)));
        Value::Enum(value)
    }

    pub fn struct_def(def: StructDef) -> Value {
        let def = Rc::new(def);
        gc::track(Tracked::Struct(Rc::downgrade(&def)));
        Value::StructDef(def)
    }

    pub fn cell(value: Value) -> Cell {
        let cell = Rc::new(RefCell::new(value));
        gc::track(Tracked::Cell(Rc::downgrade(&cell)));
        cell
    }

    pub fn range(start: i64, end: i64) -> Value {
        Value::from_list(List::Range(start, end))
    }

    pub fn from_list(list: List) -> Value {
        let list = Rc::new(RefCell::new(list));
        gc::track(Tracked::List(Rc::downgrade(&list)));
        Value::List(list)
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Number(_) => "number",
            Value::Bool(_) => "bool",
            Value::None => "none",
            Value::Str(_) => "string",
            Value::Bytes(_) => "bytes",
            Value::List(_) => "list",
            Value::Record(_) => "record",
            Value::Function(_) | Value::Native(..) => "function",
            Value::Cell(_) => "cell",
            Value::StructDef(_) => "struct type",
            Value::Instance(_) => "struct",
            Value::EnumDef(_) => "enum type",
            Value::Enum(_) => "enum",
            Value::Channel(_) => "channel",
        }
    }

    pub fn list(items: Vec<Value>) -> Value {
        Value::from_list(List::from_values(items))
    }

    pub fn str(text: &str) -> Value {
        Value::Str(Rc::new(text.to_string()))
    }
}

pub const MAX_DEPTH: usize = 200;

thread_local! {
    static DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static TOO_DEEP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static SHOWING: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}

struct Showing;

impl Showing {
    fn enter(address: usize) -> Option<Showing> {
        SHOWING.with(|showing| {
            let mut showing = showing.borrow_mut();
            if showing.contains(&address) {
                return None;
            }
            showing.push(address);
            Some(Showing)
        })
    }
}

impl Drop for Showing {
    fn drop(&mut self) {
        SHOWING.with(|showing| {
            showing.borrow_mut().pop();
        });
    }
}

fn address_of(value: &Value) -> Option<usize> {
    Some(match value {
        Value::List(rc) => Rc::as_ptr(rc) as *const u8 as usize,
        Value::Record(rc) => Rc::as_ptr(rc) as *const u8 as usize,
        Value::Instance(rc) => Rc::as_ptr(rc) as *const u8 as usize,
        _ => return None,
    })
}

pub fn compare_equal(a: &Value, b: &Value) -> Result<Option<bool>, String> {
    TOO_DEEP.set(false);
    let same = equal(a, b);
    if TOO_DEEP.replace(false) {
        return Err(format!(
            "cannot compare values nested more than {MAX_DEPTH} levels deep, does one contain itself?"
        ));
    }
    Ok(same)
}

pub struct Nesting;

impl Nesting {
    pub fn enter() -> Option<Nesting> {
        let depth = DEPTH.get();
        if depth >= MAX_DEPTH {
            TOO_DEEP.set(true);
            return None;
        }
        DEPTH.set(depth + 1);
        Some(Nesting)
    }
}

impl Drop for Nesting {
    fn drop(&mut self) {
        DEPTH.set(DEPTH.get().saturating_sub(1));
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let nested = matches!(
            self,
            Value::List(_) | Value::Record(_) | Value::Instance(_) | Value::Enum(_)
        );
        let guard = Nesting::enter();
        if guard.is_none() && nested {
            return write!(f, "...");
        }
        let showing = address_of(self).map(Showing::enter);
        if matches!(showing, Some(None)) {
            return write!(f, "...");
        }
        match self {
            Value::Number(n) => write_number(f, *n),
            Value::Bool(b) => write!(f, "{b}"),
            Value::None => write!(f, "none"),
            Value::Str(s) => write!(f, "{s}"),
            Value::Bytes(bytes) => write!(f, "<{} bytes>", bytes.len()),
            Value::List(items) => {
                let items: Vec<String> = items
                    .borrow()
                    .iter()
                    .map(|item| format!("{item:?}"))
                    .collect();
                write!(f, "[{}]", items.join(", "))
            }
            Value::Record(fields) => {
                let fields: Vec<String> = fields
                    .borrow()
                    .iter()
                    .map(|(name, value)| format!("{name}: {value:?}"))
                    .collect();
                write!(f, "{{{}}}", fields.join(", "))
            }
            Value::Function(closure) => write!(f, "<fn {}>", closure.proto.label()),
            Value::Native(native) => write!(f, "<fn {}>", native.name),
            Value::Cell(cell) => write!(f, "{}", cell.borrow()),
            Value::StructDef(def) => write!(f, "<struct {}>", def.name),
            Value::EnumDef(def) => write!(f, "<enum {}>", def.name),
            Value::Channel(_) => write!(f, "<channel>"),
            Value::Instance(instance) => {
                let values = instance.fields.borrow();
                let fields: Vec<String> = instance
                    .def
                    .fields
                    .iter()
                    .zip(values.iter())
                    .map(|(name, value)| format!("{name}: {value:?}"))
                    .collect();
                write!(f, "{} {{ {} }}", instance.def.name, fields.join(", "))
            }
            Value::Enum(value) => {
                let (variant, _) = &value.def.variants[value.variant];
                write!(f, "{}.{variant}", value.def.name)?;
                if !value.values.is_empty() {
                    let values: Vec<String> =
                        value.values.iter().map(|v| format!("{v:?}")).collect();
                    write!(f, "({})", values.join(", "))?;
                }
                Ok(())
            }
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Str(s) => write!(f, "{s:?}"),
            other => write!(f, "{other}"),
        }
    }
}

pub fn write_number(out: &mut impl fmt::Write, n: f64) -> fmt::Result {
    if n.fract() != 0.0 || n.abs() >= 1e15 || (n == 0.0 && n.is_sign_negative()) {
        return write!(out, "{n}");
    }
    let mut digits = [0u8; 20];
    let mut at = digits.len();
    let mut rest = (n as i64).unsigned_abs();
    loop {
        at -= 1;
        digits[at] = b'0' + (rest % 10) as u8;
        rest /= 10;
        if rest == 0 {
            break;
        }
    }
    if n < 0.0 {
        at -= 1;
        digits[at] = b'-';
    }
    out.write_str(std::str::from_utf8(&digits[at..]).expect("digits are ASCII"))
}

pub fn equal(a: &Value, b: &Value) -> Option<bool> {
    let _guard = Nesting::enter()?;
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => Some(x == y),
        (Value::Bool(x), Value::Bool(y)) => Some(x == y),
        (Value::Str(x), Value::Str(y)) => Some(x == y),
        (Value::List(x), Value::List(y)) => {
            if Rc::ptr_eq(x, y) {
                return Some(true);
            }
            let (x, y) = (x.borrow(), y.borrow());
            Some(
                x.len() == y.len()
                    && x.iter()
                        .zip(y.iter())
                        .all(|(a, b)| equal(&a, &b).unwrap_or(false)),
            )
        }
        (Value::Record(x), Value::Record(y)) => {
            if Rc::ptr_eq(x, y) {
                return Some(true);
            }
            let (x, y) = (x.borrow(), y.borrow());
            Some(
                x.len() == y.len()
                    && x.iter().all(|(name, a)| {
                        y.get(name).is_some_and(|b| equal(a, &b).unwrap_or(false))
                    }),
            )
        }
        (Value::Instance(x), Value::Instance(y)) => {
            if Rc::ptr_eq(x, y) {
                return Some(true);
            }
            let (xs, ys) = (x.fields.borrow(), y.fields.borrow());
            Some(
                (Rc::ptr_eq(&x.def, &y.def) || x.def.name == y.def.name)
                    && xs
                        .iter()
                        .zip(ys.iter())
                        .all(|(a, b)| equal(a, b).unwrap_or(false)),
            )
        }
        (Value::Enum(x), Value::Enum(y)) => Some(
            (Rc::ptr_eq(&x.def, &y.def) || x.def.name == y.def.name)
                && x.variant == y.variant
                && x.values
                    .iter()
                    .zip(y.values.iter())
                    .all(|(a, b)| equal(a, b).unwrap_or(false)),
        ),
        (Value::Bytes(x), Value::Bytes(y)) => Some(x == y),
        (Value::Channel(x), Value::Channel(y)) => Some(Arc::ptr_eq(x, y)),
        (Value::None, other) | (other, Value::None) => Some(matches!(other, Value::None)),
        _ => None,
    }
}

pub fn binary(op: Op, a: Value, b: Value) -> Result<Value, String> {
    use Value::{Bool, Number, Str};

    if let Op::Equal | Op::NotEqual = op {
        return match compare_equal(&a, &b)? {
            Some(same) => Ok(Bool(same == matches!(op, Op::Equal))),
            None => Err(mismatch(op, &a, &b)),
        };
    }

    match (op, a, b) {
        (Op::Add, Number(x), Number(y)) => Ok(Number(x + y)),
        (Op::Add, Str(mut x), Str(y)) => {
            Rc::make_mut(&mut x).push_str(&y);
            Ok(Str(x))
        }
        (Op::Add, a @ Str(_), b) | (Op::Add, a, b @ Str(_)) => Err(format!(
            "cannot use '+' on {} and {}, use \"${{...}}\" to put values inside a string",
            a.type_name(),
            b.type_name()
        )),
        (Op::Sub, Number(x), Number(y)) => Ok(Number(x - y)),
        (Op::Mul, Number(x), Number(y)) => Ok(Number(x * y)),
        (Op::Div, Number(x), Number(y)) => Ok(Number(x / y)),
        (Op::Rem, Number(x), Number(y)) => Ok(Number(x % y)),
        (Op::Less, Number(x), Number(y)) => Ok(Bool(x < y)),
        (Op::LessEqual, Number(x), Number(y)) => Ok(Bool(x <= y)),
        (Op::Greater, Number(x), Number(y)) => Ok(Bool(x > y)),
        (Op::GreaterEqual, Number(x), Number(y)) => Ok(Bool(x >= y)),
        (op, a, b) => Err(mismatch(op, &a, &b)),
    }
}

fn mismatch(op: Op, a: &Value, b: &Value) -> String {
    format!(
        "cannot use '{}' on {} and {}",
        symbol(op),
        a.type_name(),
        b.type_name()
    )
}

pub fn symbol(op: Op) -> &'static str {
    match op {
        Op::Add => "+",
        Op::Sub => "-",
        Op::Mul => "*",
        Op::Div => "/",
        Op::Rem => "%",
        Op::Equal => "==",
        Op::NotEqual => "!=",
        Op::Less => "<",
        Op::LessEqual => "<=",
        Op::Greater => ">",
        Op::GreaterEqual => ">=",
        Op::And => "&&",
        Op::Or => "||",
        Op::Coalesce => "??",
    }
}

pub static NATIVES: [Native; 13] = [
    Native {
        name: "print",
        function: print,
    },
    Native {
        name: "len",
        function: len,
    },
    Native {
        name: "push",
        function: push,
    },
    Native {
        name: "range",
        function: range,
    },
    Native {
        name: "keys",
        function: keys,
    },
    Native {
        name: "assert",
        function: assert,
    },
    Native {
        name: "assert_eq",
        function: assert_eq,
    },
    Native {
        name: "spawn",
        function: task::spawn,
    },
    Native {
        name: "channel",
        function: task::channel,
    },
    Native {
        name: "send",
        function: task::send,
    },
    Native {
        name: "receive",
        function: task::receive,
    },
    Native {
        name: "select#",
        function: task::select,
    },
    Native {
        name: "close",
        function: task::close,
    },
];

fn assert(args: &[Value]) -> Result<Value, String> {
    match args {
        [Value::Bool(true)] | [Value::Bool(true), _] => Ok(Value::None),
        [Value::Bool(false)] => Err("assertion failed".to_string()),
        [Value::Bool(false), message] => Err(format!("assertion failed: {message}")),
        [other] | [other, _] => Err(format!("assert needs a bool, got {}", other.type_name())),
        _ => Err("assert expects a condition and an optional message".to_string()),
    }
}

fn assert_eq(args: &[Value]) -> Result<Value, String> {
    let [actual, expected] = arguments::<2>("assert_eq", args)?;
    if compare_equal(actual, expected)? == Some(true) {
        return Ok(Value::None);
    }
    let shown = |value: &Value| match value {
        Value::Str(text) => format!("\"{text}\""),
        other => other.to_string(),
    };
    Err(format!(
        "expected {}, got {}",
        shown(expected),
        shown(actual)
    ))
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    pub static OUTPUT: RefCell<String> = const { RefCell::new(String::new()) };
}

fn print(args: &[Value]) -> Result<Value, String> {
    let parts: Vec<String> = args.iter().map(Value::to_string).collect();
    #[cfg(not(target_arch = "wasm32"))]
    println!("{}", parts.join(" "));
    #[cfg(target_arch = "wasm32")]
    OUTPUT.with(|output| {
        let mut output = output.borrow_mut();
        output.push_str(&parts.join(" "));
        output.push('\n');
    });
    Ok(Value::None)
}

fn arguments<'a, const N: usize>(name: &str, args: &'a [Value]) -> Result<&'a [Value; N], String> {
    args.try_into().map_err(|_| {
        format!(
            "{name} expects {N} argument{}, got {}",
            if N == 1 { "" } else { "s" },
            args.len()
        )
    })
}

fn len(args: &[Value]) -> Result<Value, String> {
    match arguments::<1>("len", args)? {
        [Value::List(items)] => Ok(Value::Number(items.borrow().len() as f64)),
        [Value::Str(s)] => Ok(Value::Number(s.chars().count() as f64)),
        [Value::Bytes(bytes)] => Ok(Value::Number(bytes.len() as f64)),
        [Value::Record(fields)] => Ok(Value::Number(fields.borrow().len() as f64)),
        [other] => Err(format!(
            "len needs a list, a string, bytes or a record, got {}",
            other.type_name()
        )),
    }
}

fn push(args: &[Value]) -> Result<Value, String> {
    match arguments::<2>("push", args)? {
        [Value::List(items), value] => {
            items.borrow_mut().push(value.clone());
            Ok(Value::None)
        }
        [other, _] => Err(format!("push needs a list, got {}", other.type_name())),
    }
}

fn range(args: &[Value]) -> Result<Value, String> {
    let (start, end) = match args {
        [Value::Number(end)] => (0.0, *end),
        [Value::Number(start), Value::Number(end)] => (*start, *end),
        _ => {
            return Err(
                "range expects one or two numbers: range(end) or range(start, end)".to_string(),
            );
        }
    };
    if start.fract() != 0.0 || end.fract() != 0.0 {
        return Err("range needs whole numbers".to_string());
    }
    Ok(Value::range(start as i64, end as i64))
}

fn keys(args: &[Value]) -> Result<Value, String> {
    match arguments::<1>("keys", args)? {
        [Value::Record(fields)] => {
            let names = fields
                .borrow()
                .iter()
                .map(|(name, _)| Value::str(name))
                .collect();
            Ok(Value::list(names))
        }
        [other] => Err(format!("keys needs a record, got {}", other.type_name())),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn values_stay_small() {
        assert_eq!(std::mem::size_of::<super::Value>(), 16);
    }
}
