use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use crate::compiler::Proto;
use crate::kernel::{self, Kind, Source};
use crate::value::{Closure, Value};

const CALLS_BEFORE_COMPILING: u32 = 64;
pub const TOO_DEEP: &str = "too much recursion: more than 10000 calls deep";

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

thread_local! {
    static GENERATION: Cell<u32> = const { Cell::new(0) };
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
    if native.is_none() {
        proto.tier.set(Tier::Rejected(generation));
    }
    native
}

fn attempt(closure: &Closure, generation: u32) -> Option<Native> {
    let root = closure.proto.source.as_ref()?;
    let protos: RefCell<HashMap<String, Rc<Proto>>> = RefCell::new(HashMap::new());
    protos
        .borrow_mut()
        .insert(root.name.clone(), Rc::clone(&closure.proto));
    let globals = closure.globals.borrow();
    let find = |name: &str| -> Option<Rc<Source>> {
        if let Some(proto) = protos.borrow().get(name) {
            return proto.source.clone();
        }
        let slot = globals.names.iter().position(|n| n == name)?;
        let Some(Value::Function(callee)) = &globals.values[slot] else {
            return None;
        };
        if !callee.captures.is_empty() || !callee.proto.top_level {
            return None;
        }
        let source = callee.proto.source.clone()?;
        protos
            .borrow_mut()
            .insert(name.to_string(), Rc::clone(&callee.proto));
        Some(source)
    };
    let kernels = kernel::group(&root.name, &find).ok()?;
    drop(globals);
    let targets = targets(&kernels)?;
    let protos = protos.into_inner();
    let mut result = None;
    for (kernel, target) in kernels.iter().zip(targets) {
        let native = Native {
            target,
            arity: kernel.signature.params.len() as u8,
            bool_params: kernel
                .signature
                .params
                .iter()
                .enumerate()
                .fold(0, |mask, (at, kind)| {
                    mask | (u16::from(*kind == Kind::Bool) << at)
                }),
            returns_bool: kernel.signature.returns == Kind::Bool,
            generation,
        };
        if let Some(proto) = protos.get(&kernel.name) {
            proto.tier.set(Tier::Native(native));
        }
        if kernel.name == root.name {
            result = Some(native);
        }
    }
    result
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

    use super::{Native, TOO_DEEP, arguments, result};
    use crate::jit::{self, Context};
    use crate::kernel::Kernel;
    use crate::value::Value;

    thread_local! {
        static CONTEXT: RefCell<Context> = RefCell::new(Context::default());
    }

    pub fn targets(kernels: &[Kernel]) -> Option<Vec<super::Target>> {
        let compiled = jit::compile(kernels).ok()?;
        Some(compiled.into_iter().map(|c| c.entry).collect())
    }

    pub fn call(native: &Native, args: &[Value]) -> Option<Result<Value, String>> {
        let raw = arguments(native, args)?;
        let outcome = CONTEXT.with(|context| {
            let mut context = context.borrow_mut();
            context.error = 0;
            // SAFETY: `native.target` was produced by `jit::compile` for a kernel
            // with `native.arity` parameters, `raw` holds at least that many
            // f64 values, and the context lives for the whole call.
            let value = unsafe { (native.target)(&mut *context, raw.as_ptr()) };
            if context.error != 0 {
                context.error = 0;
                context.depth = 0;
                return Err(TOO_DEEP.to_string());
            }
            Ok(value)
        });
        Some(outcome.map(|raw| result(native, raw)))
    }
}

#[cfg(target_arch = "wasm32")]
mod backend {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::{Native, TOO_DEEP, arguments, result};
    use crate::kernel::Kernel;
    use crate::value::Value;

    unsafe extern "C" {
        fn native_call(index: u32, argc: u32, args: *const f64) -> f64;
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
                .map(|kernel| {
                    known
                        .get(&(kernel.name.clone(), kernel.signature.params.len()))
                        .copied()
                })
                .collect()
        })
    }

    pub fn call(native: &Native, args: &[Value]) -> Option<Result<Value, String>> {
        let raw = arguments(native, args)?;
        // SAFETY: both imports are provided by the page that instantiated this
        // module; `raw` holds `native.arity` f64 values at the given pointer.
        let (value, failed) = unsafe {
            let value = native_call(native.target, u32::from(native.arity), raw.as_ptr());
            (value, native_failed())
        };
        if failed != 0 {
            return Some(Err(TOO_DEEP.to_string()));
        }
        Some(Ok(result(native, value)))
    }
}

#[cfg(target_arch = "wasm32")]
pub use backend::KERNELS;
pub use backend::{call, targets};
