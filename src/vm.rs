use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::compiler::{Capture, Code, PatternTest, Proto};
use crate::error::Error;
use crate::interpreter::Interpreter;
use crate::lexer::lex;
use crate::parser::{Op, Parser};
use crate::task;
use crate::value::{
    Closure, EnumValue, Fields, GlobalStore, Globals, Instance, StructDef, Value, binary, equal,
    write_number,
};

const MAX_FRAMES: usize = 10_000;
const SLICE: u32 = 10_000;

pub type Modules = Rc<RefCell<HashMap<PathBuf, Rc<RefCell<Fields>>>>>;
pub type Loading = Rc<RefCell<Vec<PathBuf>>>;
pub type Methods = Rc<[HashMap<String, Value>; 4]>;

thread_local! {
    static TASK_METHODS: Methods = crate::interpreter::Interpreter::new().methods();
}

struct Frame {
    closure: Rc<Closure>,
    ip: usize,
    base: usize,
    return_to: usize,
}

pub struct Vm {
    pub globals: GlobalStore,
    stack: Vec<Value>,
    frames: Vec<Frame>,
    dir: PathBuf,
    modules: Modules,
    loading: Loading,
    budget: u32,
    pub preemptive: bool,
    methods: Methods,
}

impl Vm {
    pub fn new(dir: &Path, modules: Modules, loading: Loading) -> Self {
        Vm {
            globals: Globals::default().store(),
            stack: Vec::with_capacity(1024),
            frames: Vec::with_capacity(64),
            dir: dir.to_path_buf(),
            modules,
            loading,
            budget: SLICE,
            preemptive: false,
            methods: Methods::default(),
        }
    }

    pub fn load_methods(&mut self) {
        let mut methods: [HashMap<String, Value>; 4] = Default::default();
        let globals = self.globals.borrow();
        for (name, value) in globals.names.iter().zip(&globals.values) {
            let (Some((kind, method)), Some(value)) = (name.split_once('#'), value) else {
                continue;
            };
            let index = match kind {
                "list" => 0,
                "string" => 1,
                "number" => 2,
                "bool" => 3,
                _ => continue,
            };
            methods[index].insert(method.to_string(), value.clone());
        }
        drop(globals);
        self.methods = Rc::new(methods);
    }

    pub fn methods(&self) -> Methods {
        Rc::clone(&self.methods)
    }

    pub fn task(dir: &Path) -> Self {
        Vm {
            globals: Globals::default().store(),
            stack: Vec::with_capacity(16),
            frames: Vec::with_capacity(4),
            dir: dir.to_path_buf(),
            modules: Modules::default(),
            loading: Loading::default(),
            budget: SLICE,
            preemptive: true,
            methods: TASK_METHODS.with(Rc::clone),
        }
    }

    pub fn sync_globals(&mut self, names: &[String]) {
        let mut globals = self.globals.borrow_mut();
        if names.len() > globals.names.len() {
            let known = globals.names.len();
            globals.names.extend_from_slice(&names[known..]);
            globals.values.resize(names.len(), None);
        }
    }

    pub fn run(&mut self, proto: Rc<Proto>) -> Result<Value, Error> {
        let closure = Rc::new(Closure {
            proto,
            captures: Vec::new(),
            globals: Rc::clone(&self.globals),
        });
        let bottom = self.stack.len();
        self.frames.push(Frame {
            closure,
            ip: 0,
            base: bottom,
            return_to: bottom,
        });
        let result = self.execute();
        if result.is_err() {
            self.frames.clear();
            self.stack.truncate(bottom);
        }
        result
    }

    pub fn call(&mut self, function: Value, args: Vec<Value>) -> Result<Value, Error> {
        if let Value::Native(native) = &function {
            return (native.function)(&args).map_err(|message| Error::new(0, message));
        }
        self.prepare(function, args)?;
        self.resume()
    }

    pub fn prepare(&mut self, function: Value, args: Vec<Value>) -> Result<(), Error> {
        let closure = match &function {
            Value::Function(closure) => Rc::clone(closure),
            other => {
                return Err(Error::new(
                    0,
                    format!("{} is not a function", other.type_name()),
                ));
            }
        };
        let expected = closure.proto.arity as usize;
        if args.len() != expected {
            return Err(Error::new(
                0,
                format!(
                    "{} expects {expected} argument{}, got {}",
                    closure.proto.label(),
                    if expected == 1 { "" } else { "s" },
                    args.len()
                ),
            ));
        }
        let bottom = self.stack.len();
        self.stack.push(function);
        self.stack.extend(args);
        self.frames.push(Frame {
            closure,
            ip: 0,
            base: bottom + 1,
            return_to: bottom,
        });
        Ok(())
    }

    pub fn resume(&mut self) -> Result<Value, Error> {
        let result = self.execute();
        if let Err(error) = &result
            && error.message != task::BLOCKED
            && error.message != task::YIELDED
        {
            self.frames.clear();
            self.stack.clear();
        }
        result
    }

    fn execute(&mut self) -> Result<Value, Error> {
        let first = self.frames.pop().expect("run pushes the first frame");
        let mut closure = first.closure;
        let mut ip = first.ip;
        let mut base = first.base;
        let mut return_to = first.return_to;

        'frames: loop {
            let proto: &Proto = &closure.proto;
            let code: &[Code] = &proto.code;

            macro_rules! fail {
                ($message:expr) => {{
                    let message: String = $message.into();
                    let blame_caller = message.starts_with('\u{1}');
                    let line = if proto.builtin || blame_caller {
                        self.frames
                            .iter()
                            .rev()
                            .find(|frame| !frame.closure.proto.builtin)
                            .map_or(proto.lines[ip - 1] as usize, |frame| {
                                frame.closure.proto.lines[frame.ip - 1] as usize
                            })
                    } else {
                        proto.lines[ip - 1] as usize
                    };
                    return Err(Error::new(line, message.trim_start_matches('\u{1}')));
                }};
            }

            macro_rules! jump_unless {
                ($op:expr, $fast:expr, $target:expr) => {{
                    let b = self.stack.pop().expect("operand");
                    let a = self.stack.pop().expect("operand");
                    let holds = match (&a, &b) {
                        (Value::Number(x), Value::Number(y)) => $fast(*x, *y),
                        _ => match binary($op, a, b) {
                            Ok(Value::Bool(holds)) => holds,
                            Ok(_) => fail!("internal error: comparison did not give true or false"),
                            Err(message) => fail!(message),
                        },
                    };
                    if !holds {
                        ip = $target as usize;
                    }
                }};
            }

            macro_rules! enter {
                ($function:expr, $argc:expr, $base:expr, $return_to:expr) => {{
                    let function: Rc<Closure> = $function;
                    let expected = function.proto.arity as usize;
                    if $argc != expected {
                        fail!(format!(
                            "{} expects {expected} argument{}, got {}",
                            function.proto.label(),
                            if expected == 1 { "" } else { "s" },
                            $argc
                        ));
                    }
                    if self.frames.len() + 1 >= MAX_FRAMES {
                        fail!(format!(
                            "too much recursion: more than {MAX_FRAMES} calls deep"
                        ));
                    }
                    self.frames.push(Frame {
                        closure: std::mem::replace(&mut closure, function),
                        ip,
                        base,
                        return_to,
                    });
                    ip = 0;
                    base = $base;
                    return_to = $return_to;
                    continue 'frames;
                }};
            }

            macro_rules! call_at {
                ($callee_at:expr, $argc:expr) => {{
                    let callee_at: usize = $callee_at;
                    let argc: usize = $argc;
                    match &self.stack[callee_at] {
                        Value::Function(function) => {
                            let function = Rc::clone(function);
                            enter!(function, argc, callee_at + 1, callee_at)
                        }
                        Value::Native(native) => {
                            let function = native.function;
                            match function(&self.stack[callee_at + 1..]) {
                                Ok(value) => {
                                    self.stack.truncate(callee_at);
                                    self.stack.push(value);
                                }
                                Err(message) if message == task::BLOCKED => {
                                    self.frames.push(Frame {
                                        closure: Rc::clone(&closure),
                                        ip: ip - 1,
                                        base,
                                        return_to,
                                    });
                                    return Err(Error::new(0, message));
                                }
                                Err(message) => fail!(message),
                            }
                        }
                        other => fail!(format!("{} is not a function", other.type_name())),
                    }
                }};
            }

            macro_rules! numbers_const {
                ($op:expr, $fast:expr, $index:expr) => {{
                    let constant = &proto.constants[$index as usize];
                    let top = self.stack.last_mut().expect("operand");
                    if let (Value::Number(x), Value::Number(y)) = (&*top, constant) {
                        *top = $fast(*x, *y);
                    } else {
                        let a = self.stack.pop().expect("operand");
                        match binary($op, a, constant.clone()) {
                            Ok(value) => self.stack.push(value),
                            Err(message) => fail!(message),
                        }
                    }
                }};
            }

            macro_rules! jump_unless_const {
                ($op:expr, $fast:expr, $index:expr, $target:expr) => {{
                    let constant = &proto.constants[$index as usize];
                    let a = self.stack.pop().expect("operand");
                    let holds = match (&a, constant) {
                        (Value::Number(x), Value::Number(y)) => $fast(*x, *y),
                        _ => match binary($op, a, constant.clone()) {
                            Ok(Value::Bool(holds)) => holds,
                            Ok(_) => fail!("internal error: comparison did not give true or false"),
                            Err(message) => fail!(message),
                        },
                    };
                    if !holds {
                        ip = $target as usize;
                    }
                }};
            }

            macro_rules! update {
                ($place:expr, $op:expr, $value:expr) => {{
                    let value: Value = $value;
                    let place: &mut Value = $place;
                    match (&mut *place, &value) {
                        (Value::Number(x), Value::Number(y)) => {
                            if let Op::Add = $op {
                                *x += *y;
                            } else {
                                *x -= *y;
                            }
                        }
                        (Value::Str(x), Value::Str(y)) if matches!($op, Op::Add) => {
                            Rc::make_mut(x).push_str(y);
                        }
                        _ => match binary($op, place.clone(), value) {
                            Ok(result) => *place = result,
                            Err(message) => fail!(message),
                        },
                    }
                }};
            }

            macro_rules! numbers {
                ($op:expr, $fast:expr) => {{
                    let b = self.stack.pop().expect("operand");
                    let top = self.stack.last_mut().expect("operand");
                    if let (Value::Number(x), Value::Number(y)) = (&*top, &b) {
                        *top = $fast(*x, *y);
                    } else {
                        let a = self.stack.pop().expect("operand");
                        match binary($op, a, b) {
                            Ok(value) => self.stack.push(value),
                            Err(message) => fail!(message),
                        }
                    }
                }};
            }

            loop {
                let op_code = code[ip];
                ip += 1;
                match op_code {
                    Code::Constant(index) => {
                        self.stack.push(proto.constants[index as usize].clone())
                    }
                    Code::None => self.stack.push(Value::None),
                    Code::True => self.stack.push(Value::Bool(true)),
                    Code::False => self.stack.push(Value::Bool(false)),
                    Code::Pop => {
                        self.stack.pop();
                    }
                    Code::PopN(count) => {
                        let len = self.stack.len();
                        self.stack.truncate(len - count as usize);
                    }
                    Code::GetLocal(slot) => {
                        let value = self.stack[base + slot as usize].clone();
                        self.stack.push(value);
                    }
                    Code::SetLocal(slot) => {
                        let value = self.stack.pop().expect("value to store");
                        self.stack[base + slot as usize] = value;
                    }
                    Code::MakeCell(slot) => {
                        let place = &mut self.stack[base + slot as usize];
                        let value = std::mem::replace(place, Value::None);
                        *place = Value::Cell(Value::cell(value));
                    }
                    Code::GetCell(slot) => {
                        let value = match &self.stack[base + slot as usize] {
                            Value::Cell(cell) => cell.borrow().clone(),
                            other => other.clone(),
                        };
                        self.stack.push(value);
                    }
                    Code::SetCell(slot) => {
                        let value = self.stack.pop().expect("value to store");
                        match &self.stack[base + slot as usize] {
                            Value::Cell(cell) => *cell.borrow_mut() = value,
                            _ => self.stack[base + slot as usize] = value,
                        }
                    }
                    Code::GetCaptured(index) => {
                        let value = closure.captures[index as usize].borrow().clone();
                        self.stack.push(value);
                    }
                    Code::SetCaptured(index) => {
                        let value = self.stack.pop().expect("value to store");
                        *closure.captures[index as usize].borrow_mut() = value;
                    }
                    Code::GetGlobal(slot) => {
                        let globals = closure.globals.borrow();
                        match &globals.values[slot as usize] {
                            Some(value) => {
                                let value = value.clone();
                                drop(globals);
                                self.stack.push(value);
                            }
                            None => {
                                fail!(format!("'{}' is not defined", globals.names[slot as usize]))
                            }
                        }
                    }
                    Code::SetGlobal(slot) => {
                        let value = self.stack.pop().expect("value to store");
                        let mut globals = closure.globals.borrow_mut();
                        if globals.values[slot as usize].is_none() {
                            let name = &globals.names[slot as usize];
                            fail!(format!(
                                "'{name}' is not defined, use 'let {name} = ...' first"
                            ));
                        }
                        globals.values[slot as usize] = Some(value);
                    }
                    Code::DefineGlobal(slot) => {
                        let value = self.stack.pop().expect("value to store");
                        closure.globals.borrow_mut().values[slot as usize] = Some(value);
                    }
                    Code::Add => numbers!(Op::Add, |x, y| Value::Number(x + y)),
                    Code::AddConst(index) => {
                        numbers_const!(Op::Add, |x, y| Value::Number(x + y), index)
                    }
                    Code::SubConst(index) => {
                        numbers_const!(Op::Sub, |x, y| Value::Number(x - y), index)
                    }
                    Code::UpdateLocal(slot, op) => {
                        let value = self.stack.pop().expect("value");
                        update!(&mut self.stack[base + slot as usize], op, value)
                    }
                    Code::UpdateLocalConst(slot, op, c) => {
                        let value = proto.constants[c as usize].clone();
                        update!(&mut self.stack[base + slot as usize], op, value)
                    }
                    Code::UpdateGlobal(slot, op) | Code::UpdateGlobalConst(slot, op, _) => {
                        let value = match op_code_constant(&op_code) {
                            Some(c) => proto.constants[c as usize].clone(),
                            None => self.stack.pop().expect("value"),
                        };
                        let mut globals = closure.globals.borrow_mut();
                        let globals = &mut *globals;
                        let Some(place) = globals.values[slot as usize].as_mut() else {
                            let name = &globals.names[slot as usize];
                            fail!(format!(
                                "'{name}' is not defined, use 'let {name} = ...' first"
                            ));
                        };
                        update!(place, op, value)
                    }
                    Code::JumpUnlessLessConst(c, target) => {
                        jump_unless_const!(Op::Less, |x, y| x < y, c, target)
                    }
                    Code::JumpUnlessLessEqualConst(c, target) => {
                        jump_unless_const!(Op::LessEqual, |x, y| x <= y, c, target)
                    }
                    Code::JumpUnlessGreaterConst(c, target) => {
                        jump_unless_const!(Op::Greater, |x, y| x > y, c, target)
                    }
                    Code::JumpUnlessGreaterEqualConst(c, target) => {
                        jump_unless_const!(Op::GreaterEqual, |x, y| x >= y, c, target)
                    }
                    Code::JumpUnlessEqualConst(c, target) => {
                        jump_unless_const!(Op::Equal, |x, y| x == y, c, target)
                    }
                    Code::JumpUnlessNotEqualConst(c, target) => {
                        jump_unless_const!(Op::NotEqual, |x, y| x != y, c, target)
                    }
                    Code::CallGlobal(slot, argc) => {
                        let argc = argc as usize;
                        let args_at = self.stack.len() - argc;
                        let callee = closure.globals.borrow().values[slot as usize].clone();
                        match callee {
                            Some(Value::Function(function)) => {
                                enter!(function, argc, args_at, args_at)
                            }
                            Some(Value::Native(native)) => {
                                let function = native.function;
                                match function(&self.stack[args_at..]) {
                                    Ok(value) => {
                                        self.stack.truncate(args_at);
                                        self.stack.push(value);
                                    }
                                    Err(message) if message == task::BLOCKED => {
                                        self.frames.push(Frame {
                                            closure: Rc::clone(&closure),
                                            ip: ip - 1,
                                            base,
                                            return_to,
                                        });
                                        return Err(Error::new(0, message));
                                    }
                                    Err(message) => fail!(message),
                                }
                            }
                            Some(other) => {
                                fail!(format!("{} is not a function", other.type_name()))
                            }
                            None => fail!(format!(
                                "'{}' is not defined",
                                closure.globals.borrow().names[slot as usize]
                            )),
                        }
                    }
                    Code::Sub => numbers!(Op::Sub, |x, y| Value::Number(x - y)),
                    Code::Mul => numbers!(Op::Mul, |x, y| Value::Number(x * y)),
                    Code::Div => numbers!(Op::Div, |x, y| Value::Number(x / y)),
                    Code::Rem => numbers!(Op::Rem, |x: f64, y| Value::Number(x % y)),
                    Code::Less => numbers!(Op::Less, |x, y| Value::Bool(x < y)),
                    Code::LessEqual => numbers!(Op::LessEqual, |x, y| Value::Bool(x <= y)),
                    Code::Greater => numbers!(Op::Greater, |x, y| Value::Bool(x > y)),
                    Code::GreaterEqual => numbers!(Op::GreaterEqual, |x, y| Value::Bool(x >= y)),
                    Code::Equal => numbers!(Op::Equal, |x, y| Value::Bool(x == y)),
                    Code::NotEqual => numbers!(Op::NotEqual, |x, y| Value::Bool(x != y)),
                    Code::Negate => match self.stack.pop().expect("operand") {
                        Value::Number(n) => self.stack.push(Value::Number(-n)),
                        other => fail!(format!("'-' needs a number, got {}", other.type_name())),
                    },
                    Code::Not => match self.stack.pop().expect("operand") {
                        Value::Bool(b) => self.stack.push(Value::Bool(!b)),
                        other => fail!(format!(
                            "'!' needs true or false, got {}",
                            other.type_name()
                        )),
                    },
                    Code::Jump(target) => {
                        if (target as usize) < ip {
                            self.budget -= 1;
                            if self.budget == 0 {
                                self.budget = SLICE;
                                if self.preemptive {
                                    self.frames.push(Frame {
                                        closure: Rc::clone(&closure),
                                        ip: target as usize,
                                        base,
                                        return_to,
                                    });
                                    return Err(Error::new(0, task::YIELDED));
                                }
                            }
                        }
                        ip = target as usize
                    }
                    Code::JumpUnlessLess(target) => jump_unless!(Op::Less, |x, y| x < y, target),
                    Code::JumpUnlessLessEqual(target) => {
                        jump_unless!(Op::LessEqual, |x, y| x <= y, target)
                    }
                    Code::JumpUnlessGreater(target) => {
                        jump_unless!(Op::Greater, |x, y| x > y, target)
                    }
                    Code::JumpUnlessGreaterEqual(target) => {
                        jump_unless!(Op::GreaterEqual, |x, y| x >= y, target)
                    }
                    Code::JumpUnlessEqual(target) => jump_unless!(Op::Equal, |x, y| x == y, target),
                    Code::JumpUnlessNotEqual(target) => {
                        jump_unless!(Op::NotEqual, |x, y| x != y, target)
                    }
                    Code::JumpIfFalse(target, check) => {
                        match self.stack.pop().expect("condition") {
                            Value::Bool(true) => {}
                            Value::Bool(false) => ip = target as usize,
                            other => fail!(format!(
                                "{} needs true or false, got {}",
                                check.label(),
                                other.type_name()
                            )),
                        }
                    }
                    Code::JumpIfFalseKeep(target, check) => match self.stack.last() {
                        Some(Value::Bool(false)) => ip = target as usize,
                        Some(Value::Bool(true)) => {
                            self.stack.pop();
                        }
                        other => fail!(format!(
                            "{} needs true or false, got {}",
                            check.label(),
                            other.map_or("nothing", Value::type_name)
                        )),
                    },
                    Code::JumpIfTrueKeep(target, check) => match self.stack.last() {
                        Some(Value::Bool(true)) => ip = target as usize,
                        Some(Value::Bool(false)) => {
                            self.stack.pop();
                        }
                        other => fail!(format!(
                            "{} needs true or false, got {}",
                            check.label(),
                            other.map_or("nothing", Value::type_name)
                        )),
                    },
                    Code::CheckBool(check) => {
                        if let Some(other) =
                            self.stack.last().filter(|v| !matches!(v, Value::Bool(_)))
                        {
                            fail!(format!(
                                "{} needs true or false, got {}",
                                check.label(),
                                other.type_name()
                            ));
                        }
                    }
                    Code::JumpIfNotNoneKeep(target) => {
                        if matches!(self.stack.last(), Some(Value::None)) {
                            self.stack.pop();
                        } else {
                            ip = target as usize;
                        }
                    }
                    Code::JumpIfNoneKeep(target) => {
                        if matches!(self.stack.last(), Some(Value::None)) {
                            ip = target as usize;
                        }
                    }
                    Code::Call(argc) => {
                        let argc = argc as usize;
                        call_at!(self.stack.len() - argc - 1, argc)
                    }
                    Code::Invoke(name, argc) => {
                        let Value::Str(name) = &proto.constants[name as usize] else {
                            fail!("internal error: method name is not a string");
                        };
                        let argc = argc as usize;
                        let target_at = self.stack.len() - argc - 1;
                        let target = self.stack[target_at].clone();
                        match &target {
                            Value::Instance(instance) => {
                                let method = instance
                                    .def
                                    .methods
                                    .iter()
                                    .find(|(method, _)| **method == ***name)
                                    .map(|(_, value)| value.clone());
                                if let Some(Value::Function(function)) = method {
                                    let expected = function.proto.arity as usize;
                                    if expected != argc + 1 {
                                        fail!(format!(
                                            "{}.{name} expects {} argument{}, got {argc}",
                                            instance.def.name,
                                            expected.saturating_sub(1),
                                            if expected == 2 { "" } else { "s" }
                                        ));
                                    }
                                    enter!(function, argc + 1, target_at, target_at)
                                }
                                let field = instance
                                    .def
                                    .fields
                                    .iter()
                                    .position(|field| **field == ***name)
                                    .map(|at| instance.fields.borrow()[at].clone());
                                match field {
                                    Some(value) => {
                                        self.stack[target_at] = value;
                                        call_at!(target_at, argc)
                                    }
                                    None => fail!(format!(
                                        "{} has no method '{name}'",
                                        instance.def.name
                                    )),
                                }
                            }
                            Value::StructDef(def) => {
                                match def.methods.iter().find(|(method, _)| **method == ***name) {
                                    Some((_, method)) => {
                                        self.stack[target_at] = method.clone();
                                        call_at!(target_at, argc)
                                    }
                                    None => fail!(format!("{} has no method '{name}'", def.name)),
                                }
                            }
                            Value::EnumDef(def) => {
                                let Some(variant) =
                                    def.variants.iter().position(|(v, _)| **v == ***name)
                                else {
                                    fail!(format!("{} has no variant '{name}'", def.name));
                                };
                                let arity = def.variants[variant].1;
                                if arity != argc {
                                    fail!(format!(
                                        "{}.{name} holds {arity} value{}, got {argc}",
                                        def.name,
                                        if arity == 1 { "" } else { "s" }
                                    ));
                                }
                                let values = self.stack.split_off(target_at + 1);
                                self.stack[target_at] = Value::enumeration(EnumValue {
                                    def: Rc::clone(def),
                                    variant,
                                    values,
                                });
                            }
                            Value::Record(fields) => match get_field(fields, name) {
                                Ok(value) => {
                                    self.stack[target_at] = value;
                                    call_at!(target_at, argc)
                                }
                                Err(message) => fail!(message),
                            },
                            Value::List(_) | Value::Str(_) | Value::Number(_) | Value::Bool(_) => {
                                let kind = match &target {
                                    Value::List(_) => 0,
                                    Value::Str(_) => 1,
                                    Value::Number(_) => 2,
                                    _ => 3,
                                };
                                let Some(method) = self.methods[kind].get(name.as_str()).cloned()
                                else {
                                    fail!(format!("{} has no method '{name}'", target.type_name()));
                                };
                                let Value::Function(function) = method else {
                                    fail!("internal error: built-in method is not a function");
                                };
                                let expected = function.proto.arity as usize;
                                if expected != argc + 1 {
                                    fail!(format!(
                                        "{}.{name} expects {} argument{}, got {argc}",
                                        target.type_name(),
                                        expected.saturating_sub(1),
                                        if expected == 2 { "" } else { "s" }
                                    ));
                                }
                                enter!(function, argc + 1, target_at, target_at)
                            }
                            Value::None => fail!(format!(
                                "cannot read field '{name}' of none, use '?.{name}' if it might be none"
                            )),
                            other => fail!(format!(
                                "cannot read field '{name}' of {}",
                                other.type_name()
                            )),
                        }
                    }
                    Code::Return => {
                        let result = self.stack.pop().expect("return value");
                        self.stack.truncate(return_to);
                        let Some(caller) = self.frames.pop() else {
                            return Ok(result);
                        };
                        self.stack.push(result);
                        closure = caller.closure;
                        ip = caller.ip;
                        base = caller.base;
                        return_to = caller.return_to;
                        continue 'frames;
                    }
                    Code::Closure(index) => {
                        let inner = Rc::clone(&proto.protos[index as usize]);
                        let mut captures = Vec::with_capacity(inner.captures.len());
                        for capture in &inner.captures {
                            match *capture {
                                Capture::Local(slot) => match &self.stack[base + slot as usize] {
                                    Value::Cell(cell) => captures.push(Rc::clone(cell)),
                                    _ => fail!("internal error: captured variable is not a cell"),
                                },
                                Capture::Outer(outer) => {
                                    captures.push(Rc::clone(&closure.captures[outer as usize]))
                                }
                            }
                        }
                        self.stack.push(Value::closure(Closure {
                            proto: inner,
                            captures,
                            globals: Rc::clone(&closure.globals),
                        }));
                    }
                    Code::List(count) => {
                        let items = self.stack.split_off(self.stack.len() - count as usize);
                        self.stack.push(Value::list(items));
                    }
                    Code::Record(shape) => {
                        let shape = &proto.shapes[shape as usize];
                        let values = self.stack.split_off(self.stack.len() - shape.len());
                        let fields: Fields = shape.iter().cloned().zip(values).collect();
                        self.stack.push(Value::record(fields));
                    }
                    Code::Index => {
                        let index = self.stack.pop().expect("index");
                        let target = self.stack.pop().expect("target");
                        let value = match &target {
                            Value::List(items) => {
                                let items = items.borrow();
                                match position(&index, items.len()) {
                                    Ok(at) => items.get(at).unwrap_or(Value::None),
                                    Err(message) => fail!(message),
                                }
                            }
                            Value::Str(text) => {
                                let chars: Vec<char> = text.chars().collect();
                                match position(&index, chars.len()) {
                                    Ok(at) => Value::str(&chars[at].to_string()),
                                    Err(message) => fail!(message),
                                }
                            }
                            Value::Record(fields) => match &index {
                                Value::Str(name) => match get_field(fields, name) {
                                    Ok(value) => value,
                                    Err(message) => fail!(message),
                                },
                                other => fail!(format!(
                                    "record fields are named with strings, got {}",
                                    other.type_name()
                                )),
                            },
                            other => fail!(format!("cannot index into {}", other.type_name())),
                        };
                        self.stack.push(value);
                    }
                    Code::SetIndex => {
                        let value = self.stack.pop().expect("value");
                        let index = self.stack.pop().expect("index");
                        let target = self.stack.pop().expect("target");
                        match &target {
                            Value::List(items) => {
                                let len = items.borrow().len();
                                match position(&index, len) {
                                    Ok(at) => items.borrow_mut().set(at, value),
                                    Err(message) => fail!(message),
                                }
                            }
                            Value::Record(fields) => match &index {
                                Value::Str(name) => {
                                    if let Err(message) = set_field(fields, name, value) {
                                        fail!(message);
                                    }
                                }
                                other => fail!(format!(
                                    "record fields are named with strings, got {}",
                                    other.type_name()
                                )),
                            },
                            Value::Str(_) => fail!("strings can't be changed in place"),
                            other => fail!(format!("cannot index into {}", other.type_name())),
                        }
                    }
                    Code::GetField(name) => {
                        let Value::Str(name) = &proto.constants[name as usize] else {
                            fail!("internal error: field name is not a string");
                        };
                        let target = self.stack.pop().expect("target");
                        let value = match &target {
                            Value::Record(fields) => match get_field(fields, name) {
                                Ok(value) => value,
                                Err(message) => fail!(message),
                            },
                            Value::Instance(instance) => {
                                match instance
                                    .def
                                    .fields
                                    .iter()
                                    .position(|field| **field == ***name)
                                {
                                    Some(at) => instance.fields.borrow()[at].clone(),
                                    None if instance
                                        .def
                                        .methods
                                        .iter()
                                        .any(|(m, _)| **m == ***name) =>
                                    {
                                        fail!(format!(
                                            "'{name}' is a method of {}, call it like value.{name}(...)",
                                            instance.def.name
                                        ))
                                    }
                                    None => fail!(format!(
                                        "{} has no field '{name}'",
                                        instance.def.name
                                    )),
                                }
                            }
                            Value::StructDef(def) => {
                                match def.methods.iter().find(|(m, _)| **m == ***name) {
                                    Some((_, method)) => method.clone(),
                                    None => fail!(format!("{} has no method '{name}'", def.name)),
                                }
                            }
                            Value::EnumDef(def) => {
                                match def.variants.iter().position(|(v, _)| **v == ***name) {
                                    Some(variant) if def.variants[variant].1 == 0 => {
                                        Value::enumeration(EnumValue {
                                            def: Rc::clone(def),
                                            variant,
                                            values: Vec::new(),
                                        })
                                    }
                                    Some(variant) => {
                                        let arity = def.variants[variant].1;
                                        fail!(format!(
                                            "{}.{name} holds {arity} value{}, write {}.{name}(...)",
                                            def.name,
                                            if arity == 1 { "" } else { "s" },
                                            def.name
                                        ))
                                    }
                                    None => fail!(format!("{} has no variant '{name}'", def.name)),
                                }
                            }
                            Value::None => fail!(format!(
                                "cannot read field '{name}' of none, use '?.{name}' if it might be none"
                            )),
                            other => fail!(format!(
                                "cannot read field '{name}' of {}",
                                other.type_name()
                            )),
                        };
                        self.stack.push(value);
                    }
                    Code::SetField(name) => {
                        let Value::Str(name) = &proto.constants[name as usize] else {
                            fail!("internal error: field name is not a string");
                        };
                        let value = self.stack.pop().expect("value");
                        let target = self.stack.pop().expect("target");
                        match &target {
                            Value::Record(fields) => {
                                if let Err(message) = set_field(fields, name, value) {
                                    fail!(message);
                                }
                            }
                            Value::Instance(instance) => {
                                match instance
                                    .def
                                    .fields
                                    .iter()
                                    .position(|field| **field == ***name)
                                {
                                    Some(at) => instance.fields.borrow_mut()[at] = value,
                                    None => fail!(format!(
                                        "{} has no field '{name}'",
                                        instance.def.name
                                    )),
                                }
                            }
                            other => fail!(format!(
                                "cannot set field '{name}' on {}",
                                other.type_name()
                            )),
                        }
                    }
                    Code::MakeStructDef(index) => {
                        let info = &proto.structs[index as usize];
                        let methods = self.stack.split_off(self.stack.len() - info.methods.len());
                        let def = StructDef {
                            name: Rc::clone(&info.name),
                            fields: Rc::clone(&info.fields),
                            methods: info.methods.iter().cloned().zip(methods).collect(),
                        };
                        self.stack.push(Value::struct_def(def));
                    }
                    Code::MakeEnumDef(index) => {
                        self.stack
                            .push(Value::EnumDef(Rc::clone(&proto.enums[index as usize])));
                    }
                    Code::MakeStruct(shape) => {
                        let names = &proto.shapes[shape as usize];
                        let mut given = self.stack.split_off(self.stack.len() - names.len());
                        let Value::StructDef(def) = self.stack.pop().expect("struct definition")
                        else {
                            fail!("only a struct can be built with { ... }");
                        };
                        if let Some(extra) = names.iter().find(|name| !def.fields.contains(name)) {
                            fail!(format!("{} has no field '{extra}'", def.name));
                        }
                        let mut fields = Vec::with_capacity(def.fields.len());
                        for field in def.fields.iter() {
                            match names.iter().position(|name| name == field) {
                                Some(at) => {
                                    fields.push(std::mem::replace(&mut given[at], Value::None))
                                }
                                None => fail!(format!("{} is missing field '{field}'", def.name)),
                            }
                        }
                        self.stack.push(Value::instance(Instance {
                            def,
                            fields: RefCell::new(fields),
                        }));
                    }
                    Code::Test(pattern, fail_at) => match &proto.patterns[pattern as usize] {
                        PatternTest::Value { slot, constant } => {
                            let subject = &self.stack[base + *slot as usize];
                            if !equal(subject, &proto.constants[*constant as usize])
                                .unwrap_or(false)
                            {
                                ip = fail_at as usize;
                            }
                        }
                        PatternTest::Variant {
                            slot,
                            enum_name,
                            variant,
                            arity,
                        } => {
                            let matched = match &self.stack[base + *slot as usize] {
                                Value::Enum(value)
                                    if enum_name.as_ref().is_none_or(|n| *n == value.def.name)
                                        && value.def.variants[value.variant].0 == *variant =>
                                {
                                    Some(Rc::clone(value))
                                }
                                _ => None,
                            };
                            match matched {
                                Some(value) => {
                                    if value.values.len() != *arity as usize {
                                        fail!(format!(
                                            "{}.{variant} holds {} values, but the pattern names {arity}",
                                            value.def.name,
                                            value.values.len()
                                        ));
                                    }
                                    self.stack.extend(value.values.iter().cloned());
                                }
                                None => ip = fail_at as usize,
                            }
                        }
                    },
                    Code::Squash(count) => {
                        let value = self.stack.pop().expect("match value");
                        let len = self.stack.len();
                        self.stack.truncate(len - count as usize);
                        self.stack.push(value);
                    }
                    Code::MatchFail(slot) => {
                        fail!(format!(
                            "no match arm for {}",
                            self.stack[base + slot as usize]
                        ));
                    }
                    Code::Stringify => {
                        let top = self.stack.last_mut().expect("value to show");
                        if !matches!(top, Value::Str(_)) {
                            let mut text = String::new();
                            match &*top {
                                Value::Number(n) => {
                                    let _ = write_number(&mut text, *n);
                                }
                                other => {
                                    let _ = write!(text, "{other}");
                                }
                            }
                            *top = Value::Str(Rc::new(text));
                        }
                    }
                    Code::Concat(count) => {
                        let parts = self.stack.split_off(self.stack.len() - count as usize);
                        let mut text = String::new();
                        for part in &parts {
                            match part {
                                Value::Str(s) => text.push_str(s),
                                Value::Number(n) => {
                                    let _ = write_number(&mut text, *n);
                                }
                                other => {
                                    let _ = write!(text, "{other}");
                                }
                            }
                        }
                        self.stack.push(Value::Str(Rc::new(text)));
                    }
                    Code::IterInit => {
                        let items = match self.stack.pop().expect("iterable") {
                            list @ Value::List(_) => list,
                            Value::Str(text) => Value::list(
                                text.chars().map(|c| Value::str(&c.to_string())).collect(),
                            ),
                            Value::Record(_) => fail!(
                                "for needs a list or a string, got record, try 'for key in keys(record)'"
                            ),
                            other => fail!(format!(
                                "for needs a list or a string, got {}",
                                other.type_name()
                            )),
                        };
                        self.stack.push(items);
                    }
                    Code::ForNext(slot, exit) => {
                        let at = base + slot as usize;
                        let Value::Number(count) = self.stack[at + 1] else {
                            fail!("internal error: loop counter is not a number");
                        };
                        let next = match &self.stack[at] {
                            Value::List(items) => items.borrow().get(count as usize),
                            _ => fail!("internal error: loop items are not a list"),
                        };
                        match next {
                            Some(item) => {
                                self.stack[at + 1] = Value::Number(count + 1.0);
                                self.stack.push(item);
                            }
                            None => ip = exit as usize,
                        }
                    }
                    Code::Import(path) => {
                        let Value::Str(path) = &proto.constants[path as usize] else {
                            fail!("internal error: import path is not a string");
                        };
                        match self.load_module(path) {
                            Ok(module) => self.stack.push(Value::Record(module)),
                            Err(error) if error.file.is_some() => return Err(error),
                            Err(error) => fail!(error.message),
                        }
                    }
                    Code::ImportName(path, name) => {
                        let (Value::Str(path), Value::Str(name)) = (
                            &proto.constants[path as usize],
                            &proto.constants[name as usize],
                        ) else {
                            fail!("internal error: import names are not strings");
                        };
                        let Value::Record(module) = self.stack.pop().expect("module") else {
                            fail!("internal error: module is not a record");
                        };
                        let found = module
                            .borrow()
                            .iter()
                            .find(|(field, _)| **field == ***name)
                            .map(|(_, value)| value.clone());
                        match found {
                            Some(value) => self.stack.push(value),
                            None => fail!(format!("{path} has no '{name}'")),
                        }
                    }
                }
            }
        }
    }

    fn load_module(&mut self, path: &str) -> Result<Rc<RefCell<Fields>>, Error> {
        let file = if path.ends_with(".kp") {
            self.dir
                .join(path)
                .canonicalize()
                .map_err(|_| Error::new(0, format!("cannot find {path}")))?
        } else {
            crate::package::locate(&self.dir, path).map_err(|message| Error::new(0, message))?
        };

        if let Some(module) = self.modules.borrow().get(&file) {
            return Ok(Rc::clone(module));
        }
        if self.loading.borrow().contains(&file) {
            return Err(Error::new(0, format!("{path} is part of an import loop")));
        }

        let source = fs::read_to_string(&file)
            .map_err(|error| Error::new(0, format!("cannot read {path}: {error}")))?;
        self.loading.borrow_mut().push(file.clone());
        let result = self.run_module(&file, &source);
        self.loading.borrow_mut().pop();

        let fields = result.map_err(|mut error| {
            error.file.get_or_insert(file.clone());
            error
        })?;
        let Value::Record(module) = Value::record(fields) else {
            unreachable!("record makes a record")
        };
        self.modules.borrow_mut().insert(file, Rc::clone(&module));
        Ok(module)
    }

    fn run_module(&self, file: &Path, source: &str) -> Result<Fields, Error> {
        let program = Parser::new(lex(source)?).parse_program()?;
        let dir = file.parent().unwrap_or(Path::new("."));
        let mut module =
            Interpreter::with_modules(dir, Rc::clone(&self.modules), Rc::clone(&self.loading));
        module.run(&program)?;
        Ok(module.exports())
    }
}

fn position(index: &Value, len: usize) -> Result<usize, String> {
    match index {
        Value::Number(n) if n.fract() != 0.0 => {
            Err(format!("index must be a whole number, got {n}"))
        }
        Value::Number(n) if *n >= 0.0 && (*n as usize) < len => Ok(*n as usize),
        Value::Number(n) => Err(format!("index {n} is out of range, the length is {len}")),
        other => Err(format!("index must be a number, got {}", other.type_name())),
    }
}

fn get_field(fields: &RefCell<Fields>, name: &str) -> Result<Value, String> {
    fields
        .borrow()
        .iter()
        .find(|(field, _)| &**field == name)
        .map(|(_, value)| value.clone())
        .ok_or_else(|| format!("record has no field '{name}'"))
}

fn set_field(fields: &RefCell<Fields>, name: &str, value: Value) -> Result<(), String> {
    match fields
        .borrow_mut()
        .iter_mut()
        .find(|(field, _)| &**field == name)
    {
        Some((_, slot)) => {
            *slot = value;
            Ok(())
        }
        None => Err(format!(
            "record has no field '{name}', fields can only be added when the record is created"
        )),
    }
}

fn op_code_constant(code: &Code) -> Option<u32> {
    match code {
        Code::UpdateGlobalConst(_, _, c) => Some(*c),
        _ => None,
    }
}
