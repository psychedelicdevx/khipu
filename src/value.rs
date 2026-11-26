use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use crate::compiler::Proto;
use crate::parser::Op;

pub type NativeFn = fn(&[Value]) -> Result<Value, String>;
pub type Fields = Vec<(Rc<str>, Value)>;
pub type Cell = Rc<RefCell<Value>>;

#[derive(Clone)]
pub enum Value {
    Number(f64),
    Bool(bool),
    None,
    Str(Rc<String>),
    List(Rc<RefCell<Vec<Value>>>),
    Record(Rc<RefCell<Fields>>),
    Function(Rc<Closure>),
    Native(&'static Native),
    Cell(Cell),
    StructDef(Rc<StructDef>),
    Instance(Rc<Instance>),
    EnumDef(Rc<EnumDef>),
    Enum(Rc<EnumValue>),
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

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Number(_) => "number",
            Value::Bool(_) => "bool",
            Value::None => "none",
            Value::Str(_) => "string",
            Value::List(_) => "list",
            Value::Record(_) => "record",
            Value::Function(_) | Value::Native(..) => "function",
            Value::Cell(_) => "cell",
            Value::StructDef(_) => "struct type",
            Value::Instance(_) => "struct",
            Value::EnumDef(_) => "enum type",
            Value::Enum(_) => "enum",
        }
    }

    pub fn list(items: Vec<Value>) -> Value {
        Value::List(Rc::new(RefCell::new(items)))
    }

    pub fn str(text: &str) -> Value {
        Value::Str(Rc::new(text.to_string()))
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Number(n) => write_number(f, *n),
            Value::Bool(b) => write!(f, "{b}"),
            Value::None => write!(f, "none"),
            Value::Str(s) => write!(f, "{s}"),
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
                        .all(|(a, b)| equal(a, b).unwrap_or(false)),
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
                        y.iter()
                            .find(|(other, _)| other == name)
                            .is_some_and(|(_, b)| equal(a, b).unwrap_or(false))
                    }),
            )
        }
        (Value::Instance(x), Value::Instance(y)) => {
            if Rc::ptr_eq(x, y) {
                return Some(true);
            }
            let (xs, ys) = (x.fields.borrow(), y.fields.borrow());
            Some(
                Rc::ptr_eq(&x.def, &y.def)
                    && xs
                        .iter()
                        .zip(ys.iter())
                        .all(|(a, b)| equal(a, b).unwrap_or(false)),
            )
        }
        (Value::Enum(x), Value::Enum(y)) => Some(
            Rc::ptr_eq(&x.def, &y.def)
                && x.variant == y.variant
                && x.values
                    .iter()
                    .zip(y.values.iter())
                    .all(|(a, b)| equal(a, b).unwrap_or(false)),
        ),
        (Value::None, other) | (other, Value::None) => Some(matches!(other, Value::None)),
        _ => None,
    }
}

pub fn binary(op: Op, a: Value, b: Value) -> Result<Value, String> {
    use Value::{Bool, Number, Str};

    if let Op::Equal | Op::NotEqual = op {
        return match equal(&a, &b) {
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

pub static NATIVES: [Native; 7] = [
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
    if equal(actual, expected) == Some(true) {
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
        [Value::Record(fields)] => Ok(Value::Number(fields.borrow().len() as f64)),
        [other] => Err(format!(
            "len needs a list, a string or a record, got {}",
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
    let items = (start as i64..end as i64)
        .map(|n| Value::Number(n as f64))
        .collect();
    Ok(Value::list(items))
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
