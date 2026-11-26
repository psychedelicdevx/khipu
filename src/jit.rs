use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{AbiParam, Block, FuncRef, InstBuilder, MemFlagsData, Value, types};
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

pub type Entry = unsafe extern "C" fn(*mut Context, *mut f64) -> f64;

pub struct Compiled {
    pub entry: Entry,
    pub signature: Signature,
}

extern "C" fn remainder(x: f64, y: f64) -> f64 {
    x % y
}

fn ty(kind: Kind) -> types::Type {
    match kind {
        Kind::Num => types::F64,
        Kind::Bool => types::I8,
    }
}

fn signature(module: &JITModule, kernel: &Kernel) -> cranelift_codegen::ir::Signature {
    let mut signature = module.make_signature();
    signature.params.push(AbiParam::new(types::I64));
    if kernel.loop_vars > 0 {
        signature.params.push(AbiParam::new(types::I64));
    }
    for kind in &kernel.signature.params {
        signature.params.push(AbiParam::new(ty(*kind)));
    }
    signature
        .returns
        .push(AbiParam::new(ty(kernel.signature.returns)));
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
            // pointer) -> f64. The module is leaked below, so the code stays
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
        .returns
        .push(AbiParam::new(types::F64));
    let callee = module.declare_func_in_func(target, &mut context.func);
    let mut b = FunctionBuilder::new(&mut context.func, functions);
    let block = b.create_block();
    b.append_block_params_for_function_params(block);
    b.switch_to_block(block);
    b.seal_block(block);
    let state = b.block_params(block)[0];
    let pointer = b.block_params(block)[1];
    let mut args = vec![state];
    if kernel.loop_vars > 0 {
        args.push(pointer);
    }
    for (at, kind) in kernel.signature.params.iter().enumerate() {
        let raw = b
            .ins()
            .load(types::F64, MemFlagsData::trusted(), pointer, 8 * at as i32);
        args.push(match kind {
            Kind::Num => raw,
            Kind::Bool => {
                let zero = b.ins().f64const(0.0);
                b.ins().fcmp(FloatCC::NotEqual, raw, zero)
            }
        });
    }
    let call = b.ins().call(callee, &args);
    let result = b.inst_results(call)[0];
    let result = match kernel.signature.returns {
        Kind::Num => result,
        Kind::Bool => {
            let wide = b.ins().uextend(types::I32, result);
            b.ins().fcvt_from_uint(types::F64, wide)
        }
    };
    b.ins().return_(&[result]);
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
    b.append_block_param(exit, ty(kernel.signature.returns));

    b.switch_to_block(entry);
    b.seal_block(entry);
    let params: Vec<Value> = b.block_params(entry).to_vec();
    let state = params[0];
    let pointer = (kernel.loop_vars > 0).then(|| params[1]);
    let mut locals = Vec::with_capacity(kernel.locals.len());
    for (at, kind) in kernel.locals.iter().enumerate() {
        let variable = b.declare_var(ty(*kind));
        let initial = match (pointer, params.get(at + 1), kind) {
            (Some(pointer), _, kind) if at < kernel.loop_vars => {
                let raw = b
                    .ins()
                    .load(types::F64, MemFlagsData::trusted(), pointer, 8 * at as i32);
                match kind {
                    Kind::Num => raw,
                    Kind::Bool => {
                        let zero = b.ins().f64const(0.0);
                        b.ins().fcmp(FloatCC::NotEqual, raw, zero)
                    }
                }
            }
            (None, Some(value), _) => *value,
            (_, _, Kind::Num) => b.ins().f64const(0.0),
            (_, _, Kind::Bool) => b.ins().iconst(types::I8, 0),
        };
        b.def_var(variable, initial);
        locals.push(variable);
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
    let failed = failure_value(&mut b, kernel.signature.returns);
    b.ins().jump(propagate, &[failed.into()]);

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
    lowering
        .b
        .append_block_param(propagate, ty(kernel.signature.returns));
    lowering.block(&kernel.body);
    if !lowering.terminated {
        let failed = failure_value(&mut lowering.b, kernel.signature.returns);
        lowering.b.ins().jump(exit, &[failed.into()]);
    }

    lowering.b.switch_to_block(propagate);
    lowering.b.seal_block(propagate);
    let carried = lowering.b.block_params(propagate)[0];
    lowering.b.ins().jump(exit, &[carried.into()]);

    lowering.b.switch_to_block(exit);
    lowering.b.seal_block(exit);
    let result = lowering.b.block_params(exit)[0];
    if let Some(pointer) = pointer {
        for at in 0..kernel.loop_vars {
            let value = lowering.b.use_var(lowering.locals[at]);
            let raw = match kernel.locals[at] {
                Kind::Num => value,
                Kind::Bool => {
                    let wide = lowering.b.ins().uextend(types::I32, value);
                    lowering.b.ins().fcvt_from_uint(types::F64, wide)
                }
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
    lowering.b.ins().return_(&[result]);
    lowering.b.finalize(module.target_config());

    module
        .define_function(id, &mut context)
        .map_err(|e| format!("{e:?}"))?;
    module.clear_context(&mut context);
    Ok(())
}

fn failure_value(b: &mut FunctionBuilder, kind: Kind) -> Value {
    match kind {
        Kind::Num => b.ins().f64const(f64::NAN),
        Kind::Bool => b.ins().iconst(types::I8, 0),
    }
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

    fn statement(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Set(slot, value) => {
                let value = self.expression(value);
                self.b.def_var(self.locals[*slot as usize], value);
            }
            Stmt::Discard(value) => {
                self.expression(value);
            }
            Stmt::Return(value) => {
                let value = self.expression(value);
                self.b.ins().jump(self.exit, &[value.into()]);
                self.terminated = true;
            }
            Stmt::If(condition, then, otherwise) => {
                let condition = self.expression(condition);
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
                    let failed = failure_value(&mut self.b, self.kernel.signature.returns);
                    self.b.ins().jump(self.exit, &[failed.into()]);
                }
            }
            Stmt::While(condition, body) => {
                let header = self.b.create_block();
                let inner = self.b.create_block();
                let after = self.b.create_block();
                self.b.ins().jump(header, &[]);
                self.b.switch_to_block(header);
                let condition = self.expression(condition);
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

    fn expression(&mut self, expr: &Expr) -> Value {
        match expr {
            Expr::Num(n) => self.b.ins().f64const(*n),
            Expr::Bool(flag) => self.b.ins().iconst(types::I8, i64::from(*flag)),
            Expr::Local(slot) => self.b.use_var(self.locals[*slot as usize]),
            Expr::Neg(inner) => {
                let inner = self.expression(inner);
                self.b.ins().fneg(inner)
            }
            Expr::Not(inner) => {
                let inner = self.expression(inner);
                self.b.ins().bxor_imm_u(inner, 1)
            }
            Expr::Arith(op, left, right) => {
                let left = self.expression(left);
                let right = self.expression(right);
                match op {
                    Arith::Add => self.b.ins().fadd(left, right),
                    Arith::Sub => self.b.ins().fsub(left, right),
                    Arith::Mul => self.b.ins().fmul(left, right),
                    Arith::Div => self.b.ins().fdiv(left, right),
                    Arith::Rem => self.remainder(left, right),
                }
            }
            Expr::Compare(cmp, Kind::Num, left, right) => {
                let left = self.expression(left);
                let right = self.expression(right);
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
                let left = self.expression(left);
                let right = self.expression(right);
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
            Expr::And(left, right) | Expr::Or(left, right) => {
                let is_and = matches!(expr, Expr::And(..));
                let left = self.expression(left);
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
                let right = self.expression(right);
                self.b.ins().jump(merge, &[right.into()]);
                self.b.switch_to_block(merge);
                self.b.seal_block(merge);
                self.b.block_params(merge)[0]
            }
            Expr::Call(index, args) => {
                let mut values = vec![self.state];
                for arg in args {
                    values.push(self.expression(arg));
                }
                let call = self.b.ins().call(self.callees[*index as usize], &values);
                let result = self.b.inst_results(call)[0];
                let error = self
                    .b
                    .ins()
                    .load(types::I32, MemFlagsData::trusted(), self.state, 4);
                let fine = self.b.create_block();
                let failed = failure_value(&mut self.b, self.kernel.signature.returns);
                self.b
                    .ins()
                    .brif(error, self.propagate, &[failed.into()], fine, &[]);
                self.b.switch_to_block(fine);
                self.b.seal_block(fine);
                result
            }
        }
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
        let sources = sources(program);
        let kernels = group(
            root,
            &|name| sources.iter().find(|s| s.name == name).cloned(),
            true,
        )
        .unwrap();
        let compiled = compile(&kernels).unwrap();
        let mut context = Context::default();
        let mut args = args.to_vec();
        // SAFETY: the entry was produced by `compile` for `kernels[0]`, and
        // `args` holds one f64 per parameter of that kernel.
        let result = unsafe { (compiled[0].entry)(&mut context, args.as_mut_ptr()) };
        (result, context)
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
}
