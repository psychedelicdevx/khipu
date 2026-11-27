use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{
    AbiParam, Block, BlockArg, FuncRef, InstBuilder, MemFlagsData, Value, types,
};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module, default_libcall_names};

use crate::kernel::{Arith, Cmp, Expr, Kernel, Kind, Signature, Stmt};

pub const MAX_DEPTH: u32 = 10_000;

#[repr(C)]
#[derive(Default)]
pub struct Context {
    pub depth: u32,
    pub error: u32,
}

pub type Entry = unsafe extern "C" fn(*mut Context, *mut f64, *mut f64) -> f64;

pub struct Compiled {
    pub entry: Entry,
    pub signature: Signature,
}

extern "C" fn remainder(x: f64, y: f64) -> f64 {
    x % y
}

fn block_args(values: &[Value]) -> Vec<BlockArg> {
    values.iter().copied().map(BlockArg::from).collect()
}

fn ty(kind: Kind) -> types::Type {
    match kind {
        Kind::Num => types::F64,
        Kind::Bool => types::I8,
        Kind::Struct(_) => unreachable!("struct values are passed as their fields"),
        Kind::List => types::I64,
    }
}

fn signature(module: &JITModule, kernel: &Kernel) -> cranelift_codegen::ir::Signature {
    let mut signature = module.make_signature();
    signature.params.push(AbiParam::new(types::I64));
    if kernel.loop_vars > 0 {
        signature.params.push(AbiParam::new(types::I64));
    }
    for kind in kernel.flat_params() {
        signature.params.push(AbiParam::new(ty(kind)));
    }
    for kind in kernel.flat_returns() {
        signature.returns.push(AbiParam::new(ty(kind)));
    }
    signature
}

pub fn compile(kernels: &[Kernel]) -> Result<Vec<Compiled>, String> {
    let mut flags = settings::builder();
    flags.set("opt_level", "speed").map_err(|e| e.to_string())?;
    flags.set("is_pic", "false").map_err(|e| e.to_string())?;
    let isa = cranelift_native::builder()
        .map_err(|e| e.to_string())?
        .finish(settings::Flags::new(flags))
        .map_err(|e| e.to_string())?;
    let mut builder = JITBuilder::with_isa(isa, default_libcall_names());
    builder.symbol("khipu_remainder", remainder as *const u8);
    let mut module = JITModule::new(builder);

    let mut remainder_signature = module.make_signature();
    remainder_signature.params.push(AbiParam::new(types::F64));
    remainder_signature.params.push(AbiParam::new(types::F64));
    remainder_signature.returns.push(AbiParam::new(types::F64));
    let remainder_id = module
        .declare_function("khipu_remainder", Linkage::Import, &remainder_signature)
        .map_err(|e| e.to_string())?;

    let mut ids = Vec::with_capacity(kernels.len());
    for (index, kernel) in kernels.iter().enumerate() {
        let sig = signature(&module, kernel);
        let id = module
            .declare_function(&format!("k{index}"), Linkage::Local, &sig)
            .map_err(|e| e.to_string())?;
        ids.push(id);
    }
    let mut entries = Vec::with_capacity(kernels.len());
    for index in 0..kernels.len() {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.returns.push(AbiParam::new(types::F64));
        let id = module
            .declare_function(&format!("e{index}"), Linkage::Local, &sig)
            .map_err(|e| e.to_string())?;
        entries.push(id);
    }

    let mut functions = FunctionBuilderContext::new();
    for (index, kernel) in kernels.iter().enumerate() {
        define_kernel(
            &mut module,
            &mut functions,
            kernel,
            ids[index],
            &ids,
            remainder_id,
        )?;
        define_entry(
            &mut module,
            &mut functions,
            kernel,
            entries[index],
            ids[index],
        )?;
    }
    module.finalize_definitions().map_err(|e| e.to_string())?;

    let compiled = kernels
        .iter()
        .zip(&entries)
        .map(|(kernel, id)| {
            let pointer = module.get_finalized_function(*id);
            // SAFETY: `e{index}` was declared and defined above with exactly the
            // signature `Entry` describes: (i64 context pointer, i64 argument
            // pointer, i64 result pointer) -> f64. The module is leaked below, so the code stays
            // mapped for the rest of the program.
            let entry: Entry = unsafe { std::mem::transmute::<*const u8, Entry>(pointer) };
            Compiled {
                entry,
                signature: kernel.signature.clone(),
            }
        })
        .collect();
    Box::leak(Box::new(module));
    Ok(compiled)
}

fn define_entry(
    module: &mut JITModule,
    functions: &mut FunctionBuilderContext,
    kernel: &Kernel,
    id: FuncId,
    target: FuncId,
) -> Result<(), String> {
    let mut context = module.make_context();
    context
        .func
        .signature
        .params
        .push(AbiParam::new(types::I64));
    context
        .func
        .signature
        .params
        .push(AbiParam::new(types::I64));
    context
        .func
        .signature
        .params
        .push(AbiParam::new(types::I64));
    context
        .func
        .signature
        .returns
        .push(AbiParam::new(types::F64));
    let callee = module.declare_func_in_func(target, &mut context.func);
    let mut b = FunctionBuilder::new(&mut context.func, functions);
    let block = b.create_block();
    b.append_block_params_for_function_params(block);
    b.switch_to_block(block);
    b.seal_block(block);
    let params = b.block_params(block).to_vec();
    let state = params[0];
    let pointer = params[1];
    let out = params[2];
    let mut args = vec![state];
    if kernel.loop_vars > 0 {
        args.push(pointer);
    }
    for (at, kind) in kernel.flat_params().into_iter().enumerate() {
        let raw = b
            .ins()
            .load(types::F64, MemFlagsData::trusted(), pointer, 8 * at as i32);
        args.push(match kind {
            Kind::Num => raw,
            Kind::Bool => {
                let zero = b.ins().f64const(0.0);
                b.ins().fcmp(FloatCC::NotEqual, raw, zero)
            }
            Kind::Struct(_) => unreachable!("struct params are flattened"),
            Kind::List => {
                let mut flags = MemFlagsData::new();
                flags.set_endianness(cranelift_codegen::ir::Endianness::Little);
                b.ins().bitcast(types::I64, flags, raw)
            }
        });
    }
    let call = b.ins().call(callee, &args);
    let results = b.inst_results(call).to_vec();
    let mut first = b.ins().f64const(0.0);
    for (at, value) in results.iter().enumerate() {
        let raw = match kernel.flat_returns()[at] {
            Kind::Num => *value,
            Kind::Bool => {
                let wide = b.ins().uextend(types::I32, *value);
                b.ins().fcvt_from_uint(types::F64, wide)
            }
            Kind::Struct(_) => unreachable!("struct returns are flattened"),
            Kind::List => unreachable!("a list cannot be returned from a kernel"),
        };
        if at == 0 {
            first = raw;
        }
        b.ins()
            .store(MemFlagsData::trusted(), raw, out, 8 * at as i32);
    }
    b.ins().return_(&[first]);
    b.finalize(module.target_config());
    module
        .define_function(id, &mut context)
        .map_err(|e| e.to_string())?;
    module.clear_context(&mut context);
    Ok(())
}

struct Lowering<'a, 'b> {
    b: FunctionBuilder<'b>,
    kernel: &'a Kernel,
    state: Value,
    locals: Vec<Variable>,
    exit: Block,
    propagate: Block,
    loops: Vec<(Block, Block)>,
    callees: Vec<FuncRef>,
    remainder: FuncRef,
    terminated: bool,
}

fn define_kernel(
    module: &mut JITModule,
    functions: &mut FunctionBuilderContext,
    kernel: &Kernel,
    id: FuncId,
    ids: &[FuncId],
    remainder_id: FuncId,
) -> Result<(), String> {
    let mut context = module.make_context();
    context.func.signature = signature(module, kernel);
    let callees: Vec<FuncRef> = ids
        .iter()
        .map(|callee| module.declare_func_in_func(*callee, &mut context.func))
        .collect();
    let remainder = module.declare_func_in_func(remainder_id, &mut context.func);
    let mut b = FunctionBuilder::new(&mut context.func, functions);

    let entry = b.create_block();
    let body = b.create_block();
    let overflow = b.create_block();
    let propagate = b.create_block();
    let exit = b.create_block();
    b.append_block_params_for_function_params(entry);
    let returns = kernel.flat_returns();
    for kind in &returns {
        b.append_block_param(propagate, ty(*kind));
        b.append_block_param(exit, ty(*kind));
    }

    b.switch_to_block(entry);
    b.seal_block(entry);
    let params: Vec<Value> = b.block_params(entry).to_vec();
    let state = params[0];
    let pointer = (kernel.loop_vars > 0).then(|| params[1]);
    let mut locals = Vec::with_capacity(kernel.locals.len());
    for (at, kind) in kernel.locals.iter().enumerate() {
        let variable = b.declare_var(ty(*kind));
        let initial = if let Some(pointer) = pointer
            && at < kernel.loop_vars
        {
            let raw = b
                .ins()
                .load(types::F64, MemFlagsData::trusted(), pointer, 8 * at as i32);
            match kind {
                Kind::Num => raw,
                Kind::Bool => {
                    let zero = b.ins().f64const(0.0);
                    b.ins().fcmp(FloatCC::NotEqual, raw, zero)
                }
                Kind::Struct(_) | Kind::List => unreachable!("loop variables are scalars"),
            }
        } else {
            match kind {
                Kind::Num => b.ins().f64const(0.0),
                Kind::Bool => b.ins().iconst(types::I8, 0),
                Kind::Struct(_) => unreachable!("struct locals are split into fields"),
                Kind::List => b.ins().iconst(types::I64, 0),
            }
        };
        b.def_var(variable, initial);
        locals.push(variable);
    }
    let shift = 1 + usize::from(pointer.is_some());
    let offsets = kernel.param_offsets();
    for (logical, base) in kernel.param_bases.iter().enumerate() {
        match kernel.signature.params[logical] {
            Kind::Struct(index) => {
                for (field, _) in kernel.layouts[index as usize].1.iter().enumerate() {
                    let value = params[shift + offsets[logical] + field];
                    b.def_var(locals[*base as usize + field], value);
                }
            }
            Kind::List => {
                let at = shift + offsets[logical];
                b.def_var(locals[*base as usize], params[at]);
                b.def_var(locals[*base as usize + 1], params[at + 1]);
            }
            _ => b.def_var(locals[*base as usize], params[shift + offsets[logical]]),
        }
    }
    let depth = b.ins().load(types::I32, MemFlagsData::trusted(), state, 0);
    let deeper = b.ins().iadd_imm_s(depth, 1);
    b.ins().store(MemFlagsData::trusted(), deeper, state, 0);
    let too_deep = b
        .ins()
        .icmp_imm_s(IntCC::SignedGreaterThan, deeper, i64::from(MAX_DEPTH));
    b.ins().brif(too_deep, overflow, &[], body, &[]);

    b.switch_to_block(overflow);
    b.seal_block(overflow);
    let one = b.ins().iconst(types::I32, 1);
    b.ins().store(MemFlagsData::trusted(), one, state, 4);
    let failed = failure_values(&mut b, &returns);
    b.ins().jump(propagate, &block_args(&failed));

    b.switch_to_block(body);
    b.seal_block(body);

    let mut lowering = Lowering {
        b,
        kernel,
        state,
        locals,
        exit,
        propagate,
        loops: Vec::new(),
        callees,
        remainder,
        terminated: false,
    };
    lowering.block(&kernel.body);
    if !lowering.terminated {
        let failed = failure_values(&mut lowering.b, &returns);
        lowering.b.ins().jump(exit, &block_args(&failed));
    }

    lowering.b.switch_to_block(propagate);
    lowering.b.seal_block(propagate);
    let carried = lowering.b.block_params(propagate).to_vec();
    lowering.b.ins().jump(exit, &block_args(&carried));

    lowering.b.switch_to_block(exit);
    lowering.b.seal_block(exit);
    let result = lowering.b.block_params(exit).to_vec();
    if let Some(pointer) = pointer {
        for at in 0..kernel.loop_vars {
            let value = lowering.b.use_var(lowering.locals[at]);
            let raw = match kernel.locals[at] {
                Kind::Num => value,
                Kind::Bool => {
                    let wide = lowering.b.ins().uextend(types::I32, value);
                    lowering.b.ins().fcvt_from_uint(types::F64, wide)
                }
                Kind::Struct(_) | Kind::List => unreachable!("loop variables are scalars"),
            };
            lowering
                .b
                .ins()
                .store(MemFlagsData::trusted(), raw, pointer, 8 * at as i32);
        }
    }
    let depth = lowering
        .b
        .ins()
        .load(types::I32, MemFlagsData::trusted(), state, 0);
    let shallower = lowering.b.ins().iadd_imm_s(depth, -1);
    lowering
        .b
        .ins()
        .store(MemFlagsData::trusted(), shallower, state, 0);
    lowering.b.ins().return_(&result);
    lowering.b.finalize(module.target_config());

    module
        .define_function(id, &mut context)
        .map_err(|e| format!("{e:?}"))?;
    module.clear_context(&mut context);
    Ok(())
}

fn failure_values(b: &mut FunctionBuilder, kinds: &[Kind]) -> Vec<Value> {
    kinds
        .iter()
        .map(|kind| match *kind {
            Kind::Num => b.ins().f64const(f64::NAN),
            Kind::Bool => b.ins().iconst(types::I8, 0),
            Kind::Struct(_) => unreachable!("struct returns are flattened"),
            Kind::List => unreachable!("a list cannot be returned from a kernel"),
        })
        .collect()
}

impl Lowering<'_, '_> {
    fn block(&mut self, block: &[Stmt]) {
        for stmt in block {
            if self.terminated {
                return;
            }
            self.statement(stmt);
        }
    }

    fn scalar(&mut self, expr: &Expr) -> Value {
        self.expression(expr)
            .into_iter()
            .next()
            .expect("a scalar expression")
    }

    fn assign(&mut self, slot: u32, expr: &Expr) {
        let values = self.expression(expr);
        if self.kernel.locals.get(slot as usize) == Some(&Kind::List) {
            self.b.def_var(self.locals[slot as usize], values[0]);
            self.b.def_var(self.locals[slot as usize + 1], values[1]);
        } else if self.kernel.struct_fields(slot).is_some() {
            for (at, value) in values.into_iter().enumerate() {
                self.b.def_var(self.locals[slot as usize + at], value);
            }
        } else {
            self.b.def_var(self.locals[slot as usize], values[0]);
        }
    }

    fn statement(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Set(slot, value) => self.assign(*slot, value),
            Stmt::MakeStruct(base, fields) => {
                for (at, field) in fields.iter().enumerate() {
                    let value = self.scalar(field);
                    self.b.def_var(self.locals[*base as usize + at], value);
                }
            }
            Stmt::SetField(base, offset, value) => {
                let value = self.scalar(value);
                self.b
                    .def_var(self.locals[*base as usize + *offset as usize], value);
            }
            Stmt::SetIndexList(base, index, value) => {
                let index = self.scalar(index);
                let value = self.scalar(value);
                let address = self.list_address(*base, index);
                self.b
                    .ins()
                    .store(MemFlagsData::trusted(), value, address, 0);
            }
            Stmt::Discard(value) => {
                self.expression(value);
            }
            Stmt::Return(value) => {
                let values = self.expression(value);
                self.b.ins().jump(self.exit, &block_args(&values));
                self.terminated = true;
            }
            Stmt::If(condition, then, otherwise) => {
                let condition = self.scalar(condition);
                let then_block = self.b.create_block();
                let else_block = self.b.create_block();
                let merge = self.b.create_block();
                self.b
                    .ins()
                    .brif(condition, then_block, &[], else_block, &[]);
                let mut merged = false;
                for (block, body) in [(then_block, then), (else_block, otherwise)] {
                    self.b.switch_to_block(block);
                    self.b.seal_block(block);
                    self.terminated = false;
                    self.block(body);
                    if !self.terminated {
                        self.b.ins().jump(merge, &[]);
                        merged = true;
                    }
                }
                self.b.switch_to_block(merge);
                self.b.seal_block(merge);
                self.terminated = !merged;
                if !merged {
                    let failed = failure_values(&mut self.b, &self.kernel.flat_returns());
                    self.b.ins().jump(self.exit, &block_args(&failed));
                }
            }
            Stmt::While(condition, body) => {
                let header = self.b.create_block();
                let inner = self.b.create_block();
                let after = self.b.create_block();
                self.b.ins().jump(header, &[]);
                self.b.switch_to_block(header);
                let condition = self.scalar(condition);
                self.b.ins().brif(condition, inner, &[], after, &[]);
                self.b.switch_to_block(inner);
                self.b.seal_block(inner);
                self.loops.push((header, after));
                self.terminated = false;
                self.block(body);
                if !self.terminated {
                    self.b.ins().jump(header, &[]);
                }
                self.loops.pop();
                self.b.seal_block(header);
                self.b.switch_to_block(after);
                self.b.seal_block(after);
                self.terminated = false;
            }
            Stmt::Whole(slots) => {
                let mut fine = self.b.ins().iconst(types::I8, 1);
                let infinity = self.b.ins().f64const(f64::INFINITY);
                for slot in slots {
                    let value = self.b.use_var(self.locals[*slot as usize]);
                    let whole = self.b.ins().trunc(value);
                    let integral = self.b.ins().fcmp(FloatCC::Equal, value, whole);
                    let magnitude = self.b.ins().fabs(value);
                    let finite = self.b.ins().fcmp(FloatCC::LessThan, magnitude, infinity);
                    let both = self.b.ins().band(integral, finite);
                    fine = self.b.ins().band(fine, both);
                }
                let bad = self.b.create_block();
                let good = self.b.create_block();
                self.b.ins().brif(fine, good, &[], bad, &[]);
                self.b.switch_to_block(bad);
                self.b.seal_block(bad);
                let one = self.b.ins().iconst(types::I32, 1);
                self.b
                    .ins()
                    .store(MemFlagsData::trusted(), one, self.state, 4);
                let failed = failure_values(&mut self.b, &self.kernel.flat_returns());
                self.b.ins().jump(self.propagate, &block_args(&failed));
                self.b.switch_to_block(good);
                self.b.seal_block(good);
            }
            Stmt::For {
                counter,
                limit,
                var,
                body,
            } => {
                let header = self.b.create_block();
                let inner = self.b.create_block();
                let latch = self.b.create_block();
                let after = self.b.create_block();
                self.b.ins().jump(header, &[]);
                self.b.switch_to_block(header);
                let current = self.b.use_var(self.locals[*counter as usize]);
                let end = self.b.use_var(self.locals[*limit as usize]);
                let more = self.b.ins().fcmp(FloatCC::LessThan, current, end);
                self.b.ins().brif(more, inner, &[], after, &[]);
                self.b.switch_to_block(inner);
                self.b.seal_block(inner);
                let current = self.b.use_var(self.locals[*counter as usize]);
                self.b.def_var(self.locals[*var as usize], current);
                self.loops.push((latch, after));
                self.terminated = false;
                self.block(body);
                if !self.terminated {
                    self.b.ins().jump(latch, &[]);
                }
                self.loops.pop();
                self.b.switch_to_block(latch);
                self.b.seal_block(latch);
                let current = self.b.use_var(self.locals[*counter as usize]);
                let one = self.b.ins().f64const(1.0);
                let next = self.b.ins().fadd(current, one);
                self.b.def_var(self.locals[*counter as usize], next);
                self.b.ins().jump(header, &[]);
                self.b.seal_block(header);
                self.b.switch_to_block(after);
                self.b.seal_block(after);
                self.terminated = false;
            }
            Stmt::Break => {
                let (_, after) = self.loops.last().expect("break inside a loop");
                self.b.ins().jump(*after, &[]);
                self.terminated = true;
            }
            Stmt::Continue => {
                let (header, _) = self.loops.last().expect("continue inside a loop");
                self.b.ins().jump(*header, &[]);
                self.terminated = true;
            }
        }
    }

    fn list_address(&mut self, base: u32, index: Value) -> Value {
        let len = self.b.use_var(self.locals[base as usize + 1]);
        let zero = self.b.ins().f64const(0.0);
        let truncated = self.b.ins().trunc(index);
        let whole = self.b.ins().fcmp(FloatCC::Equal, index, truncated);
        let nonnegative = self.b.ins().fcmp(FloatCC::GreaterThanOrEqual, index, zero);
        let in_range = self.b.ins().fcmp(FloatCC::LessThan, index, len);
        let both = self.b.ins().band(whole, nonnegative);
        let fine = self.b.ins().band(both, in_range);
        let bad = self.b.create_block();
        let good = self.b.create_block();
        self.b.ins().brif(fine, good, &[], bad, &[]);
        self.b.switch_to_block(bad);
        self.b.seal_block(bad);
        let one = self.b.ins().iconst(types::I32, 1);
        self.b
            .ins()
            .store(MemFlagsData::trusted(), one, self.state, 4);
        let failed = failure_values(&mut self.b, &self.kernel.flat_returns());
        self.b.ins().jump(self.propagate, &block_args(&failed));
        self.b.switch_to_block(good);
        self.b.seal_block(good);
        let whole = self.b.ins().fcvt_to_sint_sat(types::I64, index);
        let bytes = self.b.ins().imul_imm_s(whole, 8);
        let pointer = self.b.use_var(self.locals[base as usize]);
        self.b.ins().iadd(pointer, bytes)
    }

    fn remainder(&mut self, left: Value, right: Value) -> Value {
        let whole_left = self.b.ins().trunc(left);
        let whole_right = self.b.ins().trunc(right);
        let integral_left = self.b.ins().fcmp(FloatCC::Equal, left, whole_left);
        let integral_right = self.b.ins().fcmp(FloatCC::Equal, right, whole_right);
        let limit = self.b.ins().f64const(9007199254740992.0);
        let abs_left = self.b.ins().fabs(left);
        let abs_right = self.b.ins().fabs(right);
        let small_left = self.b.ins().fcmp(FloatCC::LessThan, abs_left, limit);
        let small_right = self.b.ins().fcmp(FloatCC::LessThan, abs_right, limit);
        let zero = self.b.ins().f64const(0.0);
        let nonzero = self.b.ins().fcmp(FloatCC::NotEqual, right, zero);
        let integral = self.b.ins().band(integral_left, integral_right);
        let small = self.b.ins().band(small_left, small_right);
        let fits = self.b.ins().band(integral, small);
        let fast = self.b.ins().band(fits, nonzero);
        let quick = self.b.create_block();
        let slow = self.b.create_block();
        let merge = self.b.create_block();
        self.b.append_block_param(merge, types::F64);
        self.b.ins().brif(fast, quick, &[], slow, &[]);
        self.b.switch_to_block(quick);
        self.b.seal_block(quick);
        let left_int = self.b.ins().fcvt_to_sint_sat(types::I64, left);
        let right_int = self.b.ins().fcvt_to_sint_sat(types::I64, right);
        let rem = self.b.ins().srem(left_int, right_int);
        let rem = self.b.ins().fcvt_from_sint(types::F64, rem);
        let signed = self.b.ins().fcopysign(rem, left);
        self.b.ins().jump(merge, &[signed.into()]);
        self.b.switch_to_block(slow);
        self.b.seal_block(slow);
        let call = self.b.ins().call(self.remainder, &[left, right]);
        let exact = self.b.inst_results(call)[0];
        self.b.ins().jump(merge, &[exact.into()]);
        self.b.switch_to_block(merge);
        self.b.seal_block(merge);
        self.b.block_params(merge)[0]
    }

    fn expression(&mut self, expr: &Expr) -> Vec<Value> {
        let value = match expr {
            Expr::Num(n) => self.b.ins().f64const(*n),
            Expr::Bool(flag) => self.b.ins().iconst(types::I8, i64::from(*flag)),
            Expr::Local(slot) => {
                let pointer = self.b.use_var(self.locals[*slot as usize]);
                if self.kernel.locals.get(*slot as usize) == Some(&Kind::List) {
                    return vec![pointer, self.b.use_var(self.locals[*slot as usize + 1])];
                }
                pointer
            }
            Expr::Struct(base, _) => {
                let fields = self
                    .kernel
                    .struct_fields(*base)
                    .expect("struct base was validated above")
                    .len();
                return (0..fields)
                    .map(|at| self.b.use_var(self.locals[*base as usize + at]))
                    .collect();
            }
            Expr::GetField(base, offset) => self
                .b
                .use_var(self.locals[*base as usize + *offset as usize]),
            Expr::ListLen(base) => self.b.use_var(self.locals[*base as usize + 1]),
            Expr::IndexList(base, index) => {
                let index = self.scalar(index);
                let address = self.list_address(*base, index);
                self.b
                    .ins()
                    .load(types::F64, MemFlagsData::trusted(), address, 0)
            }
            Expr::Neg(inner) => {
                let inner = self.scalar(inner);
                self.b.ins().fneg(inner)
            }
            Expr::Not(inner) => {
                let inner = self.scalar(inner);
                self.b.ins().bxor_imm_u(inner, 1)
            }
            Expr::Arith(op, left, right) => {
                let left = self.scalar(left);
                let right = self.scalar(right);
                match op {
                    Arith::Add => self.b.ins().fadd(left, right),
                    Arith::Sub => self.b.ins().fsub(left, right),
                    Arith::Mul => self.b.ins().fmul(left, right),
                    Arith::Div => self.b.ins().fdiv(left, right),
                    Arith::Rem => self.remainder(left, right),
                }
            }
            Expr::Compare(cmp, Kind::Num, left, right) => {
                let left = self.scalar(left);
                let right = self.scalar(right);
                let cc = match cmp {
                    Cmp::Eq => FloatCC::Equal,
                    Cmp::Ne => FloatCC::NotEqual,
                    Cmp::Lt => FloatCC::LessThan,
                    Cmp::Le => FloatCC::LessThanOrEqual,
                    Cmp::Gt => FloatCC::GreaterThan,
                    Cmp::Ge => FloatCC::GreaterThanOrEqual,
                };
                self.b.ins().fcmp(cc, left, right)
            }
            Expr::Compare(cmp, Kind::Bool, left, right) => {
                let left = self.scalar(left);
                let right = self.scalar(right);
                let cc = match cmp {
                    Cmp::Eq => IntCC::Equal,
                    Cmp::Ne => IntCC::NotEqual,
                    Cmp::Lt => IntCC::UnsignedLessThan,
                    Cmp::Le => IntCC::UnsignedLessThanOrEqual,
                    Cmp::Gt => IntCC::UnsignedGreaterThan,
                    Cmp::Ge => IntCC::UnsignedGreaterThanOrEqual,
                };
                self.b.ins().icmp(cc, left, right)
            }
            Expr::Compare(_, Kind::Struct(_) | Kind::List, _, _) => {
                unreachable!("struct and list comparison is not lowered")
            }
            Expr::And(left, right) | Expr::Or(left, right) => {
                let is_and = matches!(expr, Expr::And(..));
                let left = self.scalar(left);
                let rest = self.b.create_block();
                let merge = self.b.create_block();
                self.b.append_block_param(merge, types::I8);
                if is_and {
                    self.b.ins().brif(left, rest, &[], merge, &[left.into()]);
                } else {
                    self.b.ins().brif(left, merge, &[left.into()], rest, &[]);
                }
                self.b.switch_to_block(rest);
                self.b.seal_block(rest);
                let right = self.scalar(right);
                self.b.ins().jump(merge, &[right.into()]);
                self.b.switch_to_block(merge);
                self.b.seal_block(merge);
                self.b.block_params(merge)[0]
            }
            Expr::Call(index, args) => {
                let mut values = vec![self.state];
                for arg in args {
                    values.extend(self.expression(arg));
                }
                let call = self.b.ins().call(self.callees[*index as usize], &values);
                let results = self.b.inst_results(call).to_vec();
                let error = self
                    .b
                    .ins()
                    .load(types::I32, MemFlagsData::trusted(), self.state, 4);
                let fine = self.b.create_block();
                let failed = failure_values(&mut self.b, &self.kernel.flat_returns());
                self.b
                    .ins()
                    .brif(error, self.propagate, &block_args(&failed), fine, &[]);
                self.b.switch_to_block(fine);
                self.b.seal_block(fine);
                return results;
            }
        };
        vec![value]
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;
    use crate::interpreter::{Interpreter, Value};
    use crate::kernel::{Source, group};
    use crate::lexer::lex;
    use crate::parser::{Expr as Ast, Parser, Stmt as AstStmt};

    fn sources(program: &str) -> Vec<Rc<Source>> {
        let block = Parser::new(lex(program).unwrap()).parse_program().unwrap();
        block
            .into_iter()
            .filter_map(|(_, stmt)| match stmt {
                AstStmt::Let(
                    name,
                    _,
                    Ast::Function {
                        params,
                        param_types,
                        returns,
                        body,
                        ..
                    },
                ) => Some(Rc::new(Source {
                    name,
                    params,
                    param_types,
                    returns,
                    looping: crate::kernel::has_loop(&body),
                    body,
                })),
                _ => None,
            })
            .collect()
    }

    fn native(program: &str, root: &str, args: &[f64]) -> (f64, Context) {
        let block = Parser::new(lex(program).unwrap()).parse_program().unwrap();
        let structs = crate::kernel::struct_layouts(&block);
        let sources = sources(program);
        let kernels = group(
            root,
            &|name| sources.iter().find(|s| s.name == name).cloned(),
            &structs,
            true,
            true,
        )
        .unwrap();
        let compiled = compile(&kernels).unwrap();
        let mut context = Context::default();
        let mut args = args.to_vec();
        let mut out = [0.0; 16];
        // SAFETY: the entry was produced by `compile` for `kernels[0]`, `args`
        // holds one f64 per flat parameter, and `out` holds 16 f64 slots.
        let result =
            unsafe { (compiled[0].entry)(&mut context, args.as_mut_ptr(), out.as_mut_ptr()) };
        (result, context)
    }

    fn native_value(program: &str, root: &str, args: Vec<Value>) -> Option<f64> {
        let block = Parser::new(lex(program).unwrap()).parse_program().unwrap();
        let structs = crate::kernel::struct_layouts(&block);
        let sources = sources(program);
        let kernels = group(
            root,
            &|name| sources.iter().find(|s| s.name == name).cloned(),
            &structs,
            true,
            true,
        )
        .unwrap();
        let compiled = compile(&kernels).unwrap();
        let native = crate::native::Native {
            target: compiled[0].entry,
            kernel: Box::leak(Box::new(kernels[0].clone())),
            arity: kernels[0].arity() as u8,
            bool_params: 0,
            generation: 0,
        };
        let globals = crate::value::Globals::default().store();
        crate::native::call(&native, &args, &globals, 0).map(|value| match value {
            Value::Number(n) => n,
            other => panic!("unexpected {other:?}"),
        })
    }

    fn interpreted(program: &str, call: &str) -> f64 {
        let source = format!("{program}\n{call}");
        let block = Parser::new(lex(&source).unwrap()).parse_program().unwrap();
        match Interpreter::new().run_main(&block).unwrap() {
            Some(Value::Number(n)) => n,
            Some(Value::Bool(b)) => f64::from(b),
            other => panic!("unexpected {other:?}"),
        }
    }

    fn agree(program: &str, root: &str, args: &[f64]) {
        let sources = sources(program);
        let source = sources.iter().find(|s| s.name == root).unwrap();
        let shown: Vec<String> = args
            .iter()
            .zip(&source.param_types)
            .map(|(a, ty)| match ty {
                Some(crate::parser::Type::Bool) => (*a != 0.0).to_string(),
                _ if a.is_nan() => "0 / 0".to_string(),
                _ if a.is_infinite() => "1 / 0".to_string(),
                _ => a.to_string(),
            })
            .collect();
        let call = format!("{root}({})", shown.join(", "));
        let (got, context) = native(program, root, args);
        assert_eq!(context.error, 0, "{call} failed natively");
        assert_eq!(context.depth, 0, "{call} left the depth counter unbalanced");
        let want = interpreted(program, &call);
        assert!(
            got == want || (got.is_nan() && want.is_nan()),
            "{call}: native {got}, interpreter {want}"
        );
    }

    fn native_struct(program: &str, root: &str, args: &[f64], count: usize) -> Vec<f64> {
        let block = Parser::new(lex(program).unwrap()).parse_program().unwrap();
        let structs = crate::kernel::struct_layouts(&block);
        let sources = sources(program);
        let kernels = group(
            root,
            &|name| sources.iter().find(|s| s.name == name).cloned(),
            &structs,
            true,
            true,
        )
        .unwrap();
        let compiled = compile(&kernels).unwrap();
        let mut context = Context::default();
        let mut args = args.to_vec();
        let mut out = [0.0; 16];
        // SAFETY: the entry was produced by `compile` for `kernels[0]`, `args`
        // holds one f64 per flat parameter, and `out` holds 16 f64 slots.
        unsafe {
            (compiled[0].entry)(&mut context, args.as_mut_ptr(), out.as_mut_ptr());
        }
        assert_eq!(context.error, 0);
        assert_eq!(context.depth, 0);
        out[..count].to_vec()
    }

    fn interpreted_values(program: &str, call: &str) -> Vec<f64> {
        let source = format!("{program}\n{call}");
        let block = Parser::new(lex(&source).unwrap()).parse_program().unwrap();
        match Interpreter::new().run_main(&block).unwrap() {
            Some(Value::Instance(instance)) => instance
                .fields
                .borrow()
                .iter()
                .map(|field| match field {
                    Value::Number(n) => *n,
                    Value::Bool(b) => f64::from(*b),
                    other => panic!("unexpected field {other:?}"),
                })
                .collect(),
            other => panic!("unexpected {other:?}"),
        }
    }

    fn agree_struct(program: &str, root: &str, flat_args: &[f64], call: &str) {
        let (got, context) = native(program, root, flat_args);
        assert_eq!(context.error, 0, "{call} failed natively");
        assert_eq!(context.depth, 0, "{call} left the depth counter unbalanced");
        let want = interpreted(program, call);
        assert!(got == want, "{call}: native {got}, interpreter {want}");
    }

    const FIB: &str = "fn fib(n) {\n    if n < 2 {\n        return n\n    }\n    return fib(n - 1) + fib(n - 2)\n}";

    #[test]
    fn recursion_and_arithmetic_match_the_interpreter() {
        agree(FIB, "fib", &[20.0]);
        let work = "fn work(n) {\n    let a = 0\n    let b = 1\n    let i = 0\n    while i < n {\n        a = a + b\n        b = b + 1\n        i = i + 1\n    }\n    return a % 1000\n}";
        agree(work, "work", &[1000.0]);
        let remainder = "fn rem(a, b) {\n    return a % b\n}";
        for (a, b) in [
            (7.0, 3.0),
            (-7.0, 3.0),
            (7.5, 2.0),
            (1e300, 7.0),
            (5.0, 0.0),
            (-4.0, 2.0),
            (0.0, -3.0),
            (-0.0, 3.0),
            (9007199254740993.0, 10.0),
            (f64::INFINITY, 7.0),
            (4.0, f64::INFINITY),
            (f64::NAN, 2.0),
        ] {
            agree(remainder, "rem", &[a, b]);
        }
        let ranged = "fn total(n) {\n    let t = 0\n    for i in range(n) {\n        t = t + i\n    }\n    return t\n}";
        agree(ranged, "total", &[4.0]);
        for bad in [2.5, f64::NAN, f64::INFINITY] {
            let (result, context) = native(ranged, "total", &[bad]);
            assert!(
                result.is_nan() && context.error == 1,
                "range({bad}) must fail natively"
            );
        }
        let division = "fn div(a, b) {\n    return a / b + -a\n}";
        agree(division, "div", &[1.0, 0.0]);
        agree(division, "div", &[9.0, 4.0]);
    }

    #[test]
    fn bools_loops_and_short_circuits_match_the_interpreter() {
        let program = "fn is_even(n: number) -> bool {\n    if n == 0 {\n        return true\n    }\n    return is_odd(n - 1)\n}\nfn is_odd(n: number) -> bool {\n    if n == 0 {\n        return false\n    }\n    return is_even(n - 1)\n}\nfn pick(flag: bool, a: number, b: number) -> number {\n    if flag && a > b || !flag && a < b {\n        return a\n    }\n    return b\n}\nfn count(limit: number) -> number {\n    let total = 0\n    let i = 0\n    while true {\n        i = i + 1\n        if i > limit {\n            break\n        }\n        if i % 2 == 0 {\n            continue\n        }\n        total = total + i\n    }\n    return total\n}\nfn same(a: bool, b: bool) -> bool {\n    return a == b && !(a != b)\n}";
        agree(program, "is_even", &[11.0]);
        agree(program, "is_odd", &[11.0]);
        agree(program, "pick", &[1.0, 3.0, 2.0]);
        agree(program, "pick", &[0.0, 3.0, 2.0]);
        agree(program, "count", &[10.0]);
        agree(program, "same", &[1.0, 1.0]);
        agree(program, "same", &[1.0, 0.0]);
    }

    #[test]
    fn runaway_recursion_is_an_error_not_a_crash() {
        let forever = "fn forever(n) {\n    return forever(n + 1)\n}";
        let (result, context) = native(forever, "forever", &[0.0]);
        assert!(result.is_nan());
        assert_eq!(context.error, 1);
        assert_eq!(context.depth, 0);
    }

    #[test]
    fn flat_struct_field_reads_match_the_interpreter() {
        let program = "struct Point {\n    x: number,\n    y: number,\n}\nfn norm(n: number) -> number {\n    let p = Point { x: n, y: 2 }\n    return p.x * p.y + p.x - p.y\n}";
        agree(program, "norm", &[3.0]);
        agree(program, "norm", &[7.5]);
    }

    #[test]
    fn flat_struct_writes_and_bool_fields_match_the_interpreter() {
        let program = "struct Point {\n    x: number,\n    y: number,\n}\nstruct Flags {\n    on: bool,\n    extra: bool,\n}\nfn bump(n: number) -> number {\n    let p = Point { x: n, y: 1 }\n    p.x = p.x + 10\n    p.y = p.y * 2\n    return p.x * p.y\n}\nfn pick(flag: bool) -> number {\n    let f = Flags { on: flag, extra: !flag }\n    if f.on && !f.extra {\n        return 1\n    }\n    return 0\n}";
        agree(program, "bump", &[3.0]);
        agree(program, "pick", &[1.0]);
        agree(program, "pick", &[0.0]);
    }

    #[test]
    fn flat_struct_literals_out_of_declaration_order_match_the_interpreter() {
        let program = "struct Point {\n    x: number,\n    y: number,\n}\nfn swapped(n: number) -> number {\n    let p = Point { y: 100, x: n }\n    return p.x - p.y\n}";
        agree(program, "swapped", &[1.0]);
        agree(program, "swapped", &[250.0]);
    }

    #[test]
    fn lists_of_numbers_match_the_interpreter() {
        let program = "fn total(items: list<number>) -> number {\n    let sum = 0\n    let i = 0\n    let n = len(items)\n    while i < n {\n        sum = sum + items[i]\n        i = i + 1\n    }\n    items[0] = items[0] + 1\n    return sum + items[len(items) - 1]\n}\nfn at(items: list<number>, i: number) -> number {\n    return items[i]\n}";
        for values in [vec![10.0, 20.0, 30.0], vec![0.0], vec![-1.5, 2.5]] {
            let shown = values
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let got = native_value(
                program,
                "total",
                vec![Value::list(values.into_iter().map(Value::Number).collect())],
            )
            .unwrap();
            let want = interpreted(program, &format!("total([{shown}])"));
            assert_eq!(got, want, "total([{shown}])");
        }
        let values = vec![Value::Number(1.0), Value::Number(2.0)];
        assert_eq!(
            native_value(program, "at", vec![Value::list(values), Value::Number(1.0)]),
            Some(2.0)
        );
        assert_eq!(
            native_value(
                program,
                "at",
                vec![Value::list(vec![Value::Number(1.0)]), Value::Number(1.0)]
            ),
            None,
            "an out-of-range index must fail the native call"
        );
        assert_eq!(
            native_value(
                program,
                "at",
                vec![Value::list(vec![Value::Number(1.0)]), Value::Number(0.5)]
            ),
            None,
            "a fractional index must fail the native call"
        );
        let source = format!("{program}\nat([10, 20, 30], 4)");
        let block = Parser::new(lex(&source).unwrap()).parse_program().unwrap();
        let error = Interpreter::new().run_main(&block).unwrap_err();
        assert_eq!(error.message, "index 4 is out of range, the length is 3");
    }

    #[test]
    fn list_kernels_reject_everything_but_indexing_assignment_and_length() {
        let program = "fn created(n: number) -> number {\n    let xs = [n, n + 1]\n    return xs[0]\n}\nfn appended(n: number) -> number {\n    let xs = [n]\n    push(xs, n + 1)\n    return xs[0]\n}\nfn strings(xs: list<string>) -> number {\n    return len(xs)\n}\nfn returned(n: number) -> list<number> {\n    return [n]\n}\nfn looped(xs: list<number>) -> number {\n    let total = 0\n    for x in xs {\n        total = total + x\n    }\n    return total\n}\nfn written(xs: list<number>) -> number {\n    xs[0] = \"a\"\n    return len(xs)\n}";
        let block = Parser::new(lex(program).unwrap()).parse_program().unwrap();
        let structs = crate::kernel::struct_layouts(&block);
        let sources = sources(program);
        let find = |name: &str| sources.iter().find(|s| s.name == name).cloned();
        for root in [
            "created", "appended", "strings", "returned", "looped", "written",
        ] {
            assert!(
                group(root, &find, &structs, true, true).is_err(),
                "{root} must be rejected"
            );
        }
    }

    #[test]
    fn returned_structs_match_the_interpreter() {
        let program = "struct Point {\n    x: number,\n    y: number,\n}\nfn escape(n: number) -> Point {\n    return Point { y: n, x: n + 1 }\n}\nfn swapped(n: number) -> Point {\n    return Point { x: n, y: -n }\n}";
        for (root, argument) in [("escape", 3.0), ("escape", -7.5), ("swapped", 4.0)] {
            let got = native_struct(program, root, &[argument], 2);
            let want = interpreted_values(program, &format!("{root}({argument})"));
            assert_eq!(got, want, "{root}({argument})");
        }
    }

    #[test]
    fn structs_cross_call_boundaries_match_the_interpreter() {
        let program = "struct Point {\n    x: number,\n    y: number,\n}\nfn make(x: number, y: number) -> Point {\n    return Point { x: x, y: y }\n}\nfn area(p: Point) -> number {\n    return p.x * p.y\n}\nfn work(n: number) -> number {\n    let point = make(3, 4)\n    let total = 0\n    let i = 0\n    while i < n {\n        total = total + area(point)\n        i = i + 1\n    }\n    return total\n}\nfn via(p: Point) -> number {\n    let q = make(p.x + 1, p.y - 1)\n    return area(q)\n}";
        agree(program, "work", &[1.0]);
        agree(program, "work", &[100.0]);
        agree_struct(program, "via", &[3.0, 5.0], "via(Point { x: 3, y: 5 })");
        agree_struct(program, "via", &[-2.5, 0.0], "via(Point { x: -2.5, y: 0 })");
    }

    #[test]
    fn struct_calls_with_out_of_order_literals_match_the_interpreter() {
        let program = "struct Point {\n    x: number,\n    y: number,\n}\nfn swapped(n: number) -> Point {\n    return Point { y: 100, x: n }\n}\nfn read(p: Point) -> number {\n    return p.x - p.y\n}\nfn both(n: number) -> number {\n    let p = swapped(n)\n    return read(p)\n}";
        agree(program, "both", &[1.0]);
        agree(program, "both", &[250.0]);
    }

    #[test]
    fn flat_struct_kernels_reject_structs_that_escape() {
        let program = "struct Point {\n    x: number,\n    y: number,\n}\nfn take(p: Point) -> number {\n    return p.x\n}\nfn escape(n: number) -> Point {\n    return Point { x: n, y: 1 }\n}\nfn in_list(n: number) -> number {\n    let xs = [Point { x: n, y: 1 }]\n    return xs[0].x\n}\nfn wrong_field(n: number) -> number {\n    let p = Point { x: n, y: 1 }\n    return p.nope\n}\nfn captured(n: number) -> number {\n    let p = Point { x: n, y: 1 }\n    let f = fn() -> number { return p.x }\n    return f()\n}\nfn non_kernel(n: number) -> number {\n    let p = Point { x: n, y: 1 }\n    let sink = fn(q: Point) -> number { return q.x }\n    return sink(p)\n}\nfn wrong_struct(n: number) -> number {\n    let p = Pair { a: n, b: n + 1 }\n    return take(p)\n}\nstruct Pair {\n    a: number,\n    b: number,\n}";
        let block = Parser::new(lex(program).unwrap()).parse_program().unwrap();
        let structs = crate::kernel::struct_layouts(&block);
        let sources = sources(program);
        let find = |name: &str| sources.iter().find(|s| s.name == name).cloned();
        for root in [
            "in_list",
            "wrong_field",
            "captured",
            "non_kernel",
            "wrong_struct",
        ] {
            assert!(
                group(root, &find, &structs, true, true).is_err(),
                "{root} must be rejected"
            );
        }
        for root in ["take", "escape"] {
            assert!(
                group(root, &find, &structs, true, true).is_ok(),
                "{root} must compile"
            );
        }
        let wrong = "struct Point {\n    x: number,\n    y: number,\n}\nstruct Pair {\n    a: number,\n    b: number,\n}\nfn take(p: Point) -> number {\n    return p.x\n}\nfn wrong(n: number) -> number {\n    let p = Pair { a: n, b: n + 1 }\n    return take(p)\n}\nwrong(1)";
        let block = Parser::new(lex(wrong).unwrap()).parse_program().unwrap();
        let error = Interpreter::new().run_main(&block).unwrap_err();
        assert_eq!(
            error.message,
            "argument 1 of 'take' should be Point, got Pair"
        );
    }
}
