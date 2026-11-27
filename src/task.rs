use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

use crate::compiler::{Capture, Code, PatternTest, Proto, StructInfo};
#[cfg(not(target_arch = "wasm32"))]
use crate::error::Error;
use crate::value::{
    Cell, Closure, EnumDef, EnumValue, GlobalStore, Globals, Instance, List, Native, Record,
    StructDef, Value,
};

pub struct Channel {
    state: Mutex<ChannelState>,
    port: Option<Arc<dyn Endpoint>>,
}

pub trait Endpoint: Send + Sync {
    fn send(&self, value: &Value) -> Result<(), String>;
    fn receive(&self) -> Value;
}

pub fn endpoint(port: Arc<dyn Endpoint>) -> Value {
    Value::Channel(Arc::new(Channel {
        state: Mutex::default(),
        port: Some(port),
    }))
}

#[derive(Default)]
struct ChannelState {
    queue: VecDeque<Graph>,
    waiters: VecDeque<Waiter>,
}

struct Waiter {
    armed: Arc<AtomicBool>,
    worker: Option<usize>,
    task: u64,
}

pub enum Slot {
    Number(f64),
    Bool(bool),
    None,
    Str(String),
    Native(&'static Native),
    Channel(Arc<Channel>),
    Node(usize),
}

enum Node {
    Pending,
    List(Vec<Slot>),
    Record(Vec<(String, Slot)>),
    Cell(Slot),
    Globals(Vec<String>, Vec<Option<Slot>>),
    Function(Arc<PortableProto>, Vec<usize>, usize),
    StructDef(String, Vec<String>, Vec<(String, Slot)>),
    Instance(usize, Vec<Slot>),
    EnumDef(PortableEnum),
    Enum(usize, usize, Vec<Slot>),
}

pub struct Graph {
    nodes: Vec<Node>,
    roots: Vec<Slot>,
}

#[derive(Clone)]
pub struct PortableEnum {
    name: String,
    variants: Vec<(String, usize)>,
}

enum PortablePattern {
    Value(u32, u32),
    Variant(u32, Option<String>, String, u32),
}

pub struct PortableProto {
    name: String,
    arity: u32,
    code: Vec<Code>,
    lines: Vec<u32>,
    constants: Vec<Slot>,
    protos: Vec<Arc<PortableProto>>,
    shapes: Vec<Vec<String>>,
    captures: Vec<Capture>,
    structs: Vec<(String, Vec<String>, Vec<String>)>,
    enums: Vec<PortableEnum>,
    patterns: Vec<PortablePattern>,
    builtin: bool,
}

type Packed = HashMap<usize, (Rc<Proto>, Arc<PortableProto>)>;
type Unpacked = HashMap<usize, (Arc<PortableProto>, Rc<Proto>)>;

thread_local! {
    static PACKED: RefCell<Packed> = RefCell::default();
    static UNPACKED: RefCell<Unpacked> = RefCell::default();
}

fn address<T: ?Sized>(rc: &Rc<T>) -> usize {
    Rc::as_ptr(rc) as *const u8 as usize
}

fn strings(names: &[Rc<str>]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
}

fn names(strings: &[String]) -> Rc<[Rc<str>]> {
    strings.iter().map(|name| Rc::from(name.as_str())).collect()
}

fn pack_enum(def: &EnumDef) -> PortableEnum {
    PortableEnum {
        name: def.name.to_string(),
        variants: def
            .variants
            .iter()
            .map(|(name, arity)| (name.to_string(), *arity))
            .collect(),
    }
}

fn unpack_enum(def: &PortableEnum) -> EnumDef {
    EnumDef {
        name: Rc::from(def.name.as_str()),
        variants: def
            .variants
            .iter()
            .map(|(name, arity)| (Rc::from(name.as_str()), *arity))
            .collect(),
    }
}

fn primitive(value: &Value) -> Slot {
    match value {
        Value::Number(n) => Slot::Number(*n),
        Value::Bool(b) => Slot::Bool(*b),
        Value::Str(text) => Slot::Str(text.to_string()),
        _ => Slot::None,
    }
}

fn pack_proto(proto: &Rc<Proto>) -> Arc<PortableProto> {
    let key = address(proto);
    if let Some(found) = PACKED.with(|cache| cache.borrow().get(&key).map(|(_, p)| Arc::clone(p))) {
        return found;
    }
    let packed = Arc::new(PortableProto {
        name: proto.name.to_string(),
        arity: proto.arity,
        code: proto.code.clone(),
        lines: proto.lines.clone(),
        constants: proto.constants.iter().map(primitive).collect(),
        protos: proto.protos.iter().map(pack_proto).collect(),
        shapes: proto.shapes.iter().map(|shape| strings(shape)).collect(),
        captures: proto.captures.clone(),
        builtin: proto.builtin,
        structs: proto
            .structs
            .iter()
            .map(|info| {
                (
                    info.name.to_string(),
                    strings(&info.fields),
                    strings(&info.methods),
                )
            })
            .collect(),
        enums: proto.enums.iter().map(|def| pack_enum(def)).collect(),
        patterns: proto
            .patterns
            .iter()
            .map(|pattern| match pattern {
                PatternTest::Value { slot, constant } => PortablePattern::Value(*slot, *constant),
                PatternTest::Variant {
                    slot,
                    enum_name,
                    variant,
                    arity,
                } => PortablePattern::Variant(
                    *slot,
                    enum_name.as_ref().map(|name| name.to_string()),
                    variant.to_string(),
                    *arity,
                ),
            })
            .collect(),
    });
    PACKED.with(|cache| {
        cache
            .borrow_mut()
            .insert(key, (Rc::clone(proto), Arc::clone(&packed)))
    });
    packed
}

fn unpack_proto(packed: &Arc<PortableProto>) -> Rc<Proto> {
    let key = Arc::as_ptr(packed) as usize;
    if let Some(found) = UNPACKED.with(|cache| cache.borrow().get(&key).map(|(_, p)| Rc::clone(p)))
    {
        return found;
    }
    let constants = packed
        .constants
        .iter()
        .map(|constant| match constant {
            Slot::Number(n) => Value::Number(*n),
            Slot::Bool(b) => Value::Bool(*b),
            Slot::Str(text) => Value::str(text),
            _ => Value::None,
        })
        .collect();
    let mut proto = Proto {
        name: Rc::from(packed.name.as_str()),
        arity: packed.arity,
        code: packed.code.clone(),
        lines: packed.lines.clone(),
        constants,
        protos: packed.protos.iter().map(unpack_proto).collect(),
        shapes: packed.shapes.iter().map(|shape| names(shape)).collect(),
        captures: packed.captures.clone(),
        builtin: packed.builtin,
        source: None,
        top_level: false,
        tier: Default::default(),
        loops: Vec::new(),
        caches: Vec::new(),
        kernel_struct_layouts: Vec::new(),
        structs: packed
            .structs
            .iter()
            .map(|(name, fields, methods)| StructInfo {
                name: Rc::from(name.as_str()),
                fields: names(fields),
                methods: methods.iter().map(|m| Rc::from(m.as_str())).collect(),
            })
            .collect(),
        enums: packed
            .enums
            .iter()
            .map(|def| Rc::new(unpack_enum(def)))
            .collect(),
        patterns: packed
            .patterns
            .iter()
            .map(|pattern| match pattern {
                PortablePattern::Value(slot, constant) => PatternTest::Value {
                    slot: *slot,
                    constant: *constant,
                },
                PortablePattern::Variant(slot, enum_name, variant, arity) => PatternTest::Variant {
                    slot: *slot,
                    enum_name: enum_name.as_deref().map(Rc::from),
                    variant: Rc::from(variant.as_str()),
                    arity: *arity,
                },
            })
            .collect(),
    };
    proto.finish();
    let proto = Rc::new(proto);
    UNPACKED.with(|cache| {
        cache
            .borrow_mut()
            .insert(key, (Arc::clone(packed), Rc::clone(&proto)))
    });
    proto
}

#[derive(Default)]
struct Packer {
    seen: HashMap<usize, usize>,
    nodes: Vec<Node>,
}

pub fn pack(values: &[Value]) -> Graph {
    let mut packer = Packer::default();
    let roots = values.iter().map(|value| packer.slot(value)).collect();
    Graph {
        nodes: packer.nodes,
        roots,
    }
}

impl Packer {
    fn node(&mut self, key: usize, build: impl FnOnce(&mut Self) -> Node) -> usize {
        if let Some(index) = self.seen.get(&key) {
            return *index;
        }
        let index = self.nodes.len();
        self.nodes.push(Node::Pending);
        self.seen.insert(key, index);
        let node = build(self);
        self.nodes[index] = node;
        index
    }

    fn cell(&mut self, cell: &Cell) -> usize {
        self.node(address(cell), |packer| {
            Node::Cell(packer.slot(&cell.borrow()))
        })
    }

    fn slot(&mut self, value: &Value) -> Slot {
        let index = match value {
            Value::Number(n) => return Slot::Number(*n),
            Value::Bool(b) => return Slot::Bool(*b),
            Value::None => return Slot::None,
            Value::Str(text) => return Slot::Str(text.to_string()),
            Value::Native(native) => return Slot::Native(native),
            Value::Channel(channel) => return Slot::Channel(Arc::clone(channel)),
            Value::Cell(cell) => self.cell(cell),
            Value::List(items) => self.node(address(items), |packer| {
                Node::List(items.borrow().iter().map(|v| packer.slot(&v)).collect())
            }),
            Value::Record(fields) => self.node(address(fields), |packer| {
                Node::Record(
                    fields
                        .borrow()
                        .iter()
                        .map(|(name, value)| (name.to_string(), packer.slot(value)))
                        .collect(),
                )
            }),
            Value::Function(closure) => self.node(address(closure), |packer| {
                let captures = closure.captures.iter().map(|c| packer.cell(c)).collect();
                let globals = packer.node(address(&closure.globals), |packer| {
                    let globals = closure.globals.borrow();
                    let values = globals
                        .values
                        .iter()
                        .map(|value| value.as_ref().map(|v| packer.slot(v)))
                        .collect();
                    Node::Globals(globals.names.clone(), values)
                });
                Node::Function(pack_proto(&closure.proto), captures, globals)
            }),
            Value::StructDef(def) => self.node(address(def), |packer| {
                let methods = def
                    .methods
                    .iter()
                    .map(|(name, method)| (name.to_string(), packer.slot(method)))
                    .collect();
                Node::StructDef(def.name.to_string(), strings(&def.fields), methods)
            }),
            Value::Instance(instance) => self.node(address(instance), |packer| {
                let Slot::Node(def) = packer.slot(&Value::StructDef(Rc::clone(&instance.def)))
                else {
                    unreachable!()
                };
                let fields = instance
                    .fields
                    .borrow()
                    .iter()
                    .map(|v| packer.slot(v))
                    .collect();
                Node::Instance(def, fields)
            }),
            Value::EnumDef(def) => self.node(address(def), |_| Node::EnumDef(pack_enum(def))),
            Value::Enum(value) => self.node(address(value), |packer| {
                let Slot::Node(def) = packer.slot(&Value::EnumDef(Rc::clone(&value.def))) else {
                    unreachable!()
                };
                let values = value.values.iter().map(|v| packer.slot(v)).collect();
                Node::Enum(def, value.variant, values)
            }),
        };
        Slot::Node(index)
    }
}

#[derive(Clone)]
enum Made {
    Value(Value),
    Globals(GlobalStore),
}

struct Unpacker {
    nodes: Vec<Node>,
    made: Vec<Option<Made>>,
}

pub fn unpack(graph: Graph) -> Vec<Value> {
    let Graph { nodes, roots } = graph;
    let made = nodes
        .iter()
        .map(|node| match node {
            Node::List(_) => Some(Made::Value(Value::list(Vec::new()))),
            Node::Record(_) => Some(Made::Value(Value::record(Vec::new()))),
            Node::Cell(_) => Some(Made::Value(Value::Cell(Value::cell(Value::None)))),
            Node::Globals(names, _) => Some(Made::Globals(
                Globals {
                    values: Vec::new(),
                    names: names.clone(),
                }
                .store(),
            )),
            _ => None,
        })
        .collect();
    let mut unpacker = Unpacker { nodes, made };
    for index in 0..unpacker.nodes.len() {
        unpacker.build(index);
    }
    for index in 0..unpacker.nodes.len() {
        unpacker.fill(index);
    }
    roots.iter().map(|slot| unpacker.value(slot)).collect()
}

impl Unpacker {
    fn value(&mut self, slot: &Slot) -> Value {
        match slot {
            Slot::Number(n) => Value::Number(*n),
            Slot::Bool(b) => Value::Bool(*b),
            Slot::None => Value::None,
            Slot::Str(text) => Value::str(text),
            Slot::Native(native) => Value::Native(native),
            Slot::Channel(channel) => Value::Channel(Arc::clone(channel)),
            Slot::Node(index) => {
                self.build(*index);
                match &self.made[*index] {
                    Some(Made::Value(value)) => value.clone(),
                    _ => Value::None,
                }
            }
        }
    }

    fn cell(&self, index: usize) -> Cell {
        match &self.made[index] {
            Some(Made::Value(Value::Cell(cell))) => Rc::clone(cell),
            _ => Value::cell(Value::None),
        }
    }

    fn build(&mut self, index: usize) {
        if self.made[index].is_some() {
            return;
        }
        let built = match std::mem::replace(&mut self.nodes[index], Node::Pending) {
            Node::Function(proto, captures, globals) => {
                let captures = captures.iter().map(|c| self.cell(*c)).collect();
                let globals = match &self.made[globals] {
                    Some(Made::Globals(store)) => Rc::clone(store),
                    _ => Globals::default().store(),
                };
                let value = Value::closure(Closure {
                    proto: unpack_proto(&proto),
                    captures,
                    globals,
                });
                self.nodes[index] = Node::Function(proto, Vec::new(), 0);
                value
            }
            Node::StructDef(name, fields, methods) => {
                let methods = methods
                    .iter()
                    .map(|(name, method)| (Rc::from(name.as_str()), self.value(method)))
                    .collect();
                Value::struct_def(StructDef {
                    name: Rc::from(name),
                    fields: names(&fields),
                    methods,
                })
            }
            Node::Instance(def, fields) => {
                let Value::StructDef(def_value) = self.value(&Slot::Node(def)) else {
                    return;
                };
                self.nodes[index] = Node::Instance(def, fields);
                Value::instance(Instance {
                    def: def_value,
                    fields: RefCell::new(Vec::new()),
                })
            }
            Node::EnumDef(def) => Value::EnumDef(Rc::new(unpack_enum(&def))),
            Node::Enum(def, variant, values) => {
                let Value::EnumDef(def) = self.value(&Slot::Node(def)) else {
                    return;
                };
                let values = values.iter().map(|v| self.value(v)).collect();
                Value::enumeration(EnumValue {
                    def,
                    variant,
                    values,
                })
            }
            other => {
                self.nodes[index] = other;
                return;
            }
        };
        self.made[index] = Some(Made::Value(built));
    }

    fn fill(&mut self, index: usize) {
        let node = std::mem::replace(&mut self.nodes[index], Node::Pending);
        match (&node, self.made[index].clone()) {
            (Node::List(items), Some(Made::Value(Value::List(list)))) => {
                let items = items.iter().map(|v| self.value(v)).collect();
                *list.borrow_mut() = List::from_values(items);
            }
            (Node::Record(fields), Some(Made::Value(Value::Record(record)))) => {
                let fields = fields
                    .iter()
                    .map(|(name, v)| (Rc::from(name.as_str()), self.value(v)))
                    .collect();
                *record.borrow_mut() = Record::from_fields(fields);
            }
            (Node::Cell(slot), Some(Made::Value(Value::Cell(cell)))) => {
                let value = self.value(slot);
                *cell.borrow_mut() = value;
            }
            (Node::Globals(_, values), Some(Made::Globals(store))) => {
                let values = values
                    .iter()
                    .map(|value| value.as_ref().map(|v| self.value(v)))
                    .collect();
                store.borrow_mut().values = values;
            }
            (Node::Instance(_, fields), Some(Made::Value(Value::Instance(instance)))) => {
                let fields = fields.iter().map(|v| self.value(v)).collect();
                *instance.fields.borrow_mut() = fields;
            }
            _ => {}
        }
        self.nodes[index] = node;
    }
}

pub const BLOCKED: &str = "\u{0}blocked";
pub const YIELDED: &str = "\u{0}yielded";

struct World {
    live: usize,
    blocked: usize,
    next: u64,
}

static WORLD: Mutex<World> = Mutex::new(World {
    live: 1,
    blocked: 0,
    next: 0,
});
static WAKE: Condvar = Condvar::new();
static PROGRAM: OnceLock<(String, String)> = OnceLock::new();

thread_local! {
    static CURRENT: std::cell::Cell<Option<(usize, u64)>> = const { std::cell::Cell::new(None) };
}

pub fn set_program(path: &str, source: &str) {
    let _ = PROGRAM.set((path.to_string(), source.to_string()));
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn world() -> std::sync::MutexGuard<'static, World> {
    lock(&WORLD)
}

fn take_or_wait(
    channels: &[Arc<Channel>],
    armed: &Arc<AtomicBool>,
    worker: Option<usize>,
    task: u64,
) -> Option<(usize, Graph)> {
    for (index, channel) in channels.iter().enumerate() {
        let mut state = lock(&channel.state);
        if let Some(value) = state.queue.pop_front() {
            return Some((index, value));
        }
        state
            .waiters
            .retain(|waiter| waiter.armed.load(Ordering::Acquire));
        state.waiters.push_back(Waiter {
            armed: Arc::clone(armed),
            worker,
            task,
        });
    }
    None
}

fn received(index: usize, graph: Graph) -> (usize, Value) {
    (index, unpack(graph).remove(0))
}

pub fn wait_for(channels: &[Arc<Channel>]) -> Result<(usize, Value), String> {
    if let Some((worker, task)) = CURRENT.get() {
        let armed = Arc::new(AtomicBool::new(true));
        let mut world = world();
        world.blocked += 1;
        if let Some((index, value)) = take_or_wait(channels, &armed, Some(worker), task) {
            if armed.swap(false, Ordering::AcqRel) {
                world.blocked -= 1;
            }
            drop(world);
            return Ok(received(index, value));
        }
        if world.blocked >= world.live {
            WAKE.notify_all();
        }
        return Err(BLOCKED.to_string());
    }
    loop {
        let mut world = world();
        world.next += 1;
        let me = world.next;
        let armed = Arc::new(AtomicBool::new(true));
        world.blocked += 1;
        if let Some((index, value)) = take_or_wait(channels, &armed, None, me) {
            if armed.swap(false, Ordering::AcqRel) {
                world.blocked -= 1;
            }
            drop(world);
            return Ok(received(index, value));
        }
        while armed.load(Ordering::Acquire) && world.blocked < world.live {
            world = WAKE
                .wait(world)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        if armed.swap(false, Ordering::AcqRel) {
            world.blocked -= 1;
            return Err(
                "deadlock: every task is waiting on a channel that will never get a value"
                    .to_string(),
            );
        }
    }
}

pub fn channel(args: &[Value]) -> Result<Value, String> {
    if !args.is_empty() {
        return Err(format!("channel expects 0 arguments, got {}", args.len()));
    }
    Ok(Value::Channel(Arc::new(Channel {
        state: Mutex::default(),
        port: None,
    })))
}

pub fn send(args: &[Value]) -> Result<Value, String> {
    if let [Value::Channel(channel), value] = args
        && let Some(port) = &channel.port
    {
        return port.send(value).map(|_| Value::None);
    }
    let [Value::Channel(channel), value] = args else {
        return match args {
            [other, _] => Err(format!("send needs a channel, got {}", other.type_name())),
            _ => Err(format!("send expects 2 arguments, got {}", args.len())),
        };
    };
    let packed = pack(std::slice::from_ref(value));
    let mut world = world();
    let woken = {
        let mut state = lock(&channel.state);
        state.queue.push_back(packed);
        loop {
            match state.waiters.pop_front() {
                Some(waiter) if waiter.armed.swap(false, Ordering::AcqRel) => break Some(waiter),
                Some(_) => continue,
                None => break None,
            }
        }
    };
    if let Some(waiter) = woken {
        world.blocked -= 1;
        drop(world);
        match waiter.worker {
            Some(worker) => wake(worker, waiter.task),
            None => WAKE.notify_all(),
        }
    }
    Ok(Value::None)
}

pub fn select(args: &[Value]) -> Result<Value, String> {
    let [Value::List(items)] = args else {
        return Err("select needs a list of channels".to_string());
    };
    let channels = items
        .borrow()
        .iter()
        .map(|item| match item {
            Value::Channel(channel) if channel.port.is_some() => {
                Err("select cannot wait on a websocket".to_string())
            }
            Value::Channel(channel) => Ok(Arc::clone(&channel)),
            other => Err(format!(
                "select can only receive from a channel, got {}",
                other.type_name()
            )),
        })
        .collect::<Result<Vec<_>, String>>()?;
    let (index, value) = wait_for(&channels)?;
    Ok(Value::list(vec![Value::Number(index as f64), value]))
}

pub fn receive(args: &[Value]) -> Result<Value, String> {
    if let [Value::Channel(channel)] = args
        && let Some(port) = &channel.port
    {
        return Ok(port.receive());
    }
    match args {
        [Value::Channel(channel)] => wait_for(std::slice::from_ref(channel)).map(|(_, v)| v),
        [other] => Err(format!(
            "receive needs a channel, got {}",
            other.type_name()
        )),
        _ => Err(format!("receive expects 1 argument, got {}", args.len())),
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct Job {
    task: u64,
    values: Graph,
    dir: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct Pool {
    jobs: VecDeque<Job>,
    idle: Vec<usize>,
}

#[cfg(not(target_arch = "wasm32"))]
static POOL: Mutex<Pool> = Mutex::new(Pool {
    jobs: VecDeque::new(),
    idle: Vec::new(),
});

#[derive(Default)]
struct Inbox {
    woken: Vec<u64>,
    #[cfg(not(target_arch = "wasm32"))]
    poked: bool,
}

#[derive(Default)]
struct Worker {
    inbox: Mutex<Inbox>,
    signal: Condvar,
}

static WORKERS: OnceLock<Box<[Worker]>> = OnceLock::new();

fn wake(worker: usize, task: u64) {
    if let Some(workers) = WORKERS.get() {
        lock(&workers[worker].inbox).woken.push(task);
        workers[worker].signal.notify_one();
    }
}

#[cfg(target_arch = "wasm32")]
pub fn spawn(_: &[Value]) -> Result<Value, String> {
    Err("spawn is not available in the playground yet".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn workers() -> &'static [Worker] {
    WORKERS.get_or_init(|| {
        let count = std::thread::available_parallelism().map_or(4, |n| n.get());
        for index in 0..count {
            std::thread::Builder::new()
                .name(format!("khipu-worker-{index}"))
                .stack_size(16 * 1024 * 1024)
                .spawn(move || run_worker(index))
                .expect("could not start a worker thread");
        }
        (0..count).map(|_| Worker::default()).collect()
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn(args: &[Value]) -> Result<Value, String> {
    let Some(function) = args.first() else {
        return Err("spawn needs a function to run".to_string());
    };
    if !matches!(function, Value::Function(_) | Value::Native(_)) {
        return Err(format!(
            "spawn needs a function, got {}",
            function.type_name()
        ));
    }
    let values = pack(args);
    let dir = match PROGRAM.get() {
        Some((path, _)) => std::path::Path::new(path)
            .parent()
            .map_or_else(|| PathBuf::from("."), PathBuf::from),
        None => PathBuf::from("."),
    };
    let task = {
        let mut world = world();
        world.live += 1;
        world.next += 1;
        world.next
    };
    let workers = workers();
    let idle = {
        let mut pool = lock(&POOL);
        pool.jobs.push_back(Job { task, values, dir });
        pool.idle.pop()
    };
    if let Some(index) = idle {
        lock(&workers[index].inbox).poked = true;
        workers[index].signal.notify_one();
    }
    Ok(Value::None)
}

#[cfg(not(target_arch = "wasm32"))]
fn start(job: Job, tasks: &mut HashMap<u64, crate::vm::Vm>, ready: &mut VecDeque<u64>) {
    let mut values = unpack(job.values);
    let function = values.remove(0);
    let mut vm = crate::vm::Vm::task(&job.dir);
    if let Err(error) = vm.prepare(function, values) {
        report(error);
    }
    tasks.insert(job.task, vm);
    ready.push_back(job.task);
}

#[cfg(not(target_arch = "wasm32"))]
fn run_worker(index: usize) {
    use std::collections::HashSet;

    let me = &workers()[index];
    let mut tasks: HashMap<u64, crate::vm::Vm> = HashMap::new();
    let mut parked: HashSet<u64> = HashSet::new();
    let mut ready: VecDeque<u64> = VecDeque::new();
    loop {
        {
            let mut inbox = lock(&me.inbox);
            inbox.poked = false;
            for task in inbox.woken.drain(..) {
                if parked.remove(&task) {
                    ready.push_back(task);
                }
            }
            if ready.is_empty() {
                let mut pool = lock(&POOL);
                if let Some(job) = pool.jobs.pop_front() {
                    drop(pool);
                    drop(inbox);
                    start(job, &mut tasks, &mut ready);
                    continue;
                }
                pool.idle.push(index);
                drop(pool);
                while inbox.woken.is_empty() && !inbox.poked {
                    inbox = me
                        .signal
                        .wait(inbox)
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                }
                lock(&POOL).idle.retain(|worker| *worker != index);
                continue;
            }
        }
        let Some(task) = ready.pop_front() else {
            continue;
        };
        let Some(vm) = tasks.get_mut(&task) else {
            continue;
        };
        CURRENT.set(Some((index, task)));
        let result = vm.resume();
        CURRENT.set(None);
        match result {
            Ok(_) => {
                tasks.remove(&task);
                world().live -= 1;
                WAKE.notify_all();
            }
            Err(error) if error.message == BLOCKED => {
                parked.insert(task);
            }
            Err(error) if error.message == YIELDED => ready.push_back(task),
            Err(error) => report(error),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn report(mut error: Error) {
    error.message = format!("in a spawned task: {}", error.message);
    eprintln!("{}", render(&error));
    std::process::exit(1);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn render(error: &Error) -> String {
    match PROGRAM.get() {
        Some((path, source)) => error.render(path, source),
        None => format!("error: {}", error.message),
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn program_dir() -> PathBuf {
    PROGRAM
        .get()
        .and_then(|(path, _)| std::path::Path::new(path).parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}
