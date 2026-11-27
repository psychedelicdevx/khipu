use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use crate::compiler::Proto;
use crate::kernel::{self, Kind, LoopSource, Source};
use crate::value::{Closure, Value};

const CALLS_BEFORE_COMPILING: u32 = 64;

#[derive(Clone, Copy)]
pub struct Native {
    pub target: Target,
    pub arity: u8,
    pub bool_params: u16,
    pub returns_bool: bool,
    pub generation: u32,
}

#[cfg(not(target_arch = "wasm32"))]
pub type Target = crate::jit::Entry;
#[cfg(target_arch = "wasm32")]
pub type Target = u32;

#[derive(Clone, Copy)]
pub enum Tier {
    Counting(u32),
    Rejected(u32),
    Native(Native),
}

impl Default for Tier {
    fn default() -> Self {
        Tier::Counting(0)
    }
}

const MAX_COMPILES: u32 = 2_000;

thread_local! {
    static GENERATION: Cell<u32> = const { Cell::new(0) };
    static COMPILES: Cell<u32> = const { Cell::new(0) };
}

pub fn invalidate() {
    GENERATION.set(GENERATION.get() + 1);
}

pub fn entry(closure: &Closure) -> Option<Native> {
    let proto = &closure.proto;
    let generation = GENERATION.get();
    let count = match proto.tier.get() {
        Tier::Native(native) if native.generation == generation => return Some(native),
        Tier::Rejected(when) if when == generation => return None,
        Tier::Counting(count) => count + 1,
        Tier::Native(_) | Tier::Rejected(_) => 1,
    };
    let source = proto.source.as_ref()?;
    if count < CALLS_BEFORE_COMPILING && !source.looping {
        proto.tier.set(Tier::Counting(count));
        return None;
    }
    if !closure.captures.is_empty() {
        proto.tier.set(Tier::Rejected(generation));
        return None;
    }
    let native = attempt(closure, generation);
    proto.tier.set(match native {
        Some(native) => Tier::Native(native),
        None => Tier::Rejected(generation),
    });
    native
}

fn range_is_builtin(globals: &crate::value::Globals) -> bool {
    globals
        .names
        .iter()
        .position(|name| name == "range")
        .is_some_and(|slot| {
            matches!(&globals.values[slot], Some(Value::Native(native)) if native.name == "range")
        })
}

struct Lookup<'a> {
    store: &'a crate::value::GlobalStore,
    globals: std::cell::Ref<'a, crate::value::Globals>,
    protos: RefCell<HashMap<String, Rc<Proto>>>,
}

impl Lookup<'_> {
    fn find(&self, name: &str) -> Option<Rc<Source>> {
        let slot = self.globals.names.iter().position(|n| n == name)?;
        let Some(Value::Function(callee)) = &self.globals.values[slot] else {
            return None;
        };
        if !callee.captures.is_empty()
            || !callee.proto.top_level
            || !Rc::ptr_eq(&callee.globals, self.store)
        {
            return None;
        }
        let source = callee.proto.source.clone()?;
        self.protos
            .borrow_mut()
            .insert(name.to_string(), Rc::clone(&callee.proto));
        Some(source)
    }
}

fn install(
    kernels: &[kernel::Kernel],
    protos: &HashMap<String, Rc<Proto>>,
    generation: u32,
) -> Option<Vec<Native>> {
    if COMPILES.get() >= MAX_COMPILES {
        return None;
    }
    COMPILES.set(COMPILES.get() + 1);
    let targets = targets(kernels)?;
    let mut natives = Vec::with_capacity(kernels.len());
    for (kernel, target) in kernels.iter().zip(targets) {
        let kinds: Vec<Kind> = if kernel.loop_vars > 0 {
            kernel.locals[..kernel.loop_vars].to_vec()
        } else {
            kernel.signature.params.clone()
        };
        let native = Native {
            target,
            arity: kinds.len() as u8,
            bool_params: kinds.iter().enumerate().fold(0, |mask, (at, kind)| {
                mask | (u16::from(*kind == Kind::Bool) << at)
            }),
            returns_bool: kernel.signature.returns == Kind::Bool,
            generation,
        };
        if let Some(proto) = protos.get(&kernel.name)
            && !natives.is_empty()
        {
            proto.tier.set(Tier::Native(native));
        }
        natives.push(native);
    }
    Some(natives)
}

fn attempt(closure: &Closure, generation: u32) -> Option<Native> {
    let root = closure.proto.source.as_ref()?;
    let lookup = Lookup {
        store: &closure.globals,
        globals: closure.globals.borrow(),
        protos: RefCell::new(HashMap::new()),
    };
    let builtin = range_is_builtin(&lookup.globals);
    let root_source = Rc::clone(root);
    let root_proto = Rc::clone(&closure.proto);
    lookup
        .protos
        .borrow_mut()
        .insert(root.name.clone(), root_proto);
    let kernels = kernel::group(
        &root.name,
        &|name| {
            if name == root.name {
                Some(Rc::clone(&root_source))
            } else {
                lookup.find(name)
            }
        },
        &closure.proto.kernel_struct_layouts,
        builtin,
    )
    .ok()?;
    let protos = lookup.protos.into_inner();
    install(&kernels, &protos, generation)?.into_iter().next()
}

pub fn loop_entry(closure: &Closure, source: &LoopSource, values: &[Value]) -> Option<Native> {
    let generation = GENERATION.get();
    match source.tier.get() {
        Tier::Native(native) if native.generation == generation => return Some(native),
        Tier::Rejected(when) if when == generation => return None,
        Tier::Native(_) | Tier::Rejected(_) | Tier::Counting(_) => {}
    }
    let kinds: Option<Vec<Kind>> = values
        .iter()
        .map(|value| match value {
            Value::Number(_) => Some(Kind::Num),
            Value::Bool(_) => Some(Kind::Bool),
            _ => None,
        })
        .collect();
    let native = kinds.and_then(|kinds| {
        let lookup = Lookup {
            store: &closure.globals,
            globals: closure.globals.borrow(),
            protos: RefCell::new(HashMap::new()),
        };
        let builtin = range_is_builtin(&lookup.globals);
        let kernels = kernel::group_loop(
            source,
            &kinds,
            &|name| lookup.find(name),
            &closure.proto.kernel_struct_layouts,
            builtin,
        )
        .ok()?;
        let protos = lookup.protos.into_inner();
        install(&kernels, &protos, generation)?.into_iter().next()
    });
    source.tier.set(match native {
        Some(native) => Tier::Native(native),
        None => Tier::Rejected(generation),
    });
    native
}

pub fn run_loop(native: &Native, values: &mut [Value], depth: u32) -> Option<()> {
    let mut raw = arguments(native, values)?;
    run(native, &mut raw, depth)?;
    for (at, value) in values.iter_mut().enumerate() {
        *value = if native.bool_params & (1 << at) != 0 {
            Value::Bool(raw[at] != 0.0)
        } else {
            Value::Number(raw[at])
        };
    }
    Some(())
}

pub fn call(native: &Native, args: &[Value], depth: u32) -> Option<Value> {
    let mut raw = arguments(native, args)?;
    let value = run(native, &mut raw, depth)?;
    Some(result(native, value))
}

pub fn arguments(native: &Native, args: &[Value]) -> Option<[f64; 16]> {
    if args.len() != native.arity as usize {
        return None;
    }
    let mut raw = [0.0; 16];
    for (at, arg) in args.iter().enumerate() {
        let wants_bool = native.bool_params & (1 << at) != 0;
        raw[at] = match (arg, wants_bool) {
            (Value::Number(n), false) => *n,
            (Value::Bool(b), true) => f64::from(*b),
            _ => return None,
        };
    }
    Some(raw)
}

pub fn result(native: &Native, raw: f64) -> Value {
    if native.returns_bool {
        Value::Bool(raw != 0.0)
    } else {
        Value::Number(raw)
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod backend {
    use std::cell::RefCell;

    use super::Native;
    use crate::jit::{self, Context};
    use crate::kernel::Kernel;

    thread_local! {
        static CONTEXT: RefCell<Context> = RefCell::new(Context::default());
    }

    pub fn targets(kernels: &[Kernel]) -> Option<Vec<super::Target>> {
        let compiled = jit::compile(kernels).ok()?;
        Some(compiled.into_iter().map(|c| c.entry).collect())
    }

    pub fn run(native: &Native, raw: &mut [f64; 16], depth: u32) -> Option<f64> {
        CONTEXT.with(|context| {
            let mut context = context.borrow_mut();
            context.error = 0;
            context.depth = depth;
            // SAFETY: `native.target` was produced by `jit::compile` for a kernel
            // with `native.arity` values, `raw` holds 16 f64 slots, and the
            // context lives for the whole call.
            let value = unsafe { (native.target)(&mut *context, raw.as_mut_ptr()) };
            if context.error != 0 {
                context.error = 0;
                context.depth = 0;
                return None;
            }
            Some(value)
        })
    }
}

#[cfg(target_arch = "wasm32")]
mod backend {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::Native;
    use crate::kernel::Kernel;

    unsafe extern "C" {
        fn native_call(index: u32, argc: u32, args: *mut f64, depth: u32) -> f64;
        fn native_failed() -> u32;
    }

    thread_local! {
        pub static KERNELS: RefCell<HashMap<(String, usize), u32>> = RefCell::new(HashMap::new());
    }

    pub fn targets(kernels: &[Kernel]) -> Option<Vec<super::Target>> {
        KERNELS.with(|known| {
            let known = known.borrow();
            kernels
                .iter()
                .map(|kernel| known.get(&(kernel.name.clone(), kernel.arity())).copied())
                .collect()
        })
    }

    pub fn run(native: &Native, raw: &mut [f64; 16], depth: u32) -> Option<f64> {
        // SAFETY: both imports are provided by the page that instantiated this
        // module; `raw` holds 16 f64 slots at the given pointer.
        let (value, failed) = unsafe {
            let value = native_call(
                native.target,
                u32::from(native.arity),
                raw.as_mut_ptr(),
                depth,
            );
            (value, native_failed())
        };
        if failed != 0 {
            return None;
        }
        Some(value)
    }
}

#[cfg(target_arch = "wasm32")]
pub use backend::KERNELS;
pub use backend::{run, targets};
