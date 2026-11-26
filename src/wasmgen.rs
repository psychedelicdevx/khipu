use crate::kernel::{Arith, Cmp, Expr, Kernel, Kind, Stmt};

const MAX_DEPTH: i32 = 10_000;
const F64: u8 = 0x7c;
const I32: u8 = 0x7f;
const EMPTY: u8 = 0x40;

const UNREACHABLE: u8 = 0x00;
const BLOCK: u8 = 0x02;
const LOOP: u8 = 0x03;
const IF: u8 = 0x04;
const ELSE: u8 = 0x05;
const END: u8 = 0x0b;
const BR: u8 = 0x0c;
const BR_IF: u8 = 0x0d;
const RETURN: u8 = 0x0f;
const CALL: u8 = 0x10;
const LOCAL_GET: u8 = 0x20;
const LOCAL_SET: u8 = 0x21;
const GLOBAL_GET: u8 = 0x23;
const GLOBAL_SET: u8 = 0x24;
const I32_CONST: u8 = 0x41;
const F64_CONST: u8 = 0x44;
const I32_EQZ: u8 = 0x45;
const I32_EQ: u8 = 0x46;
const I32_NE: u8 = 0x47;
const I32_LT_U: u8 = 0x49;
const I32_GT_U: u8 = 0x4b;
const I32_LE_U: u8 = 0x4d;
const I32_GE_U: u8 = 0x4f;
const I32_GT_S: u8 = 0x4a;
const F64_EQ: u8 = 0x61;
const F64_NE: u8 = 0x62;
const F64_LT: u8 = 0x63;
const F64_GT: u8 = 0x64;
const F64_LE: u8 = 0x65;
const F64_GE: u8 = 0x66;
const I32_ADD: u8 = 0x6a;
const I32_SUB: u8 = 0x6b;
const I32_AND: u8 = 0x71;
const I32_OR: u8 = 0x72;
const I64_REM_S: u8 = 0x81;
const F64_ABS: u8 = 0x99;
const F64_NEG: u8 = 0x9a;
const F64_TRUNC: u8 = 0x9d;
const F64_ADD: u8 = 0xa0;
const F64_SUB: u8 = 0xa1;
const F64_MUL: u8 = 0xa2;
const F64_DIV: u8 = 0xa3;
const I64_TRUNC_F64_S: u8 = 0xb0;
const F64_CONVERT_I64_S: u8 = 0xb9;

const DEPTH: u32 = 0;
const ERROR: u32 = 1;
const REMAINDER: u32 = 0;

fn leb(out: &mut Vec<u8>, mut value: u32) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn sleb(out: &mut Vec<u8>, mut value: i64) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        let done = (value == 0 && byte & 0x40 == 0) || (value == -1 && byte & 0x40 != 0);
        if done {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn valtype(kind: Kind) -> u8 {
    match kind {
        Kind::Num => F64,
        Kind::Bool => I32,
    }
}

fn section(out: &mut Vec<u8>, id: u8, content: &[u8]) {
    out.push(id);
    leb(out, content.len() as u32);
    out.extend_from_slice(content);
}

fn functype(params: &[u8], results: &[u8]) -> Vec<u8> {
    let mut out = vec![0x60];
    leb(&mut out, params.len() as u32);
    out.extend_from_slice(params);
    leb(&mut out, results.len() as u32);
    out.extend_from_slice(results);
    out
}

pub struct Export {
    pub name: String,
    pub arity: usize,
}

pub fn module(kernels: &[Kernel]) -> (Vec<u8>, Vec<Export>) {
    let mut types: Vec<Vec<u8>> = vec![functype(&[F64, F64], &[F64])];
    let mut type_of = |signature: Vec<u8>| -> u32 {
        if let Some(at) = types.iter().position(|t| *t == signature) {
            return at as u32;
        }
        types.push(signature);
        (types.len() - 1) as u32
    };
    let mut function_types = vec![0u32];
    for kernel in kernels {
        let params: Vec<u8> = kernel
            .signature
            .params
            .iter()
            .map(|k| valtype(*k))
            .collect();
        let results = [valtype(kernel.signature.returns)];
        function_types.push(type_of(functype(&params, &results)));
    }

    let mut type_section = Vec::new();
    leb(&mut type_section, types.len() as u32);
    for signature in &types {
        type_section.extend_from_slice(signature);
    }

    let mut function_section = Vec::new();
    leb(&mut function_section, function_types.len() as u32);
    for index in &function_types {
        leb(&mut function_section, *index);
    }

    let mut global_section = Vec::new();
    leb(&mut global_section, 2);
    for _ in 0..2 {
        global_section.extend_from_slice(&[I32, 0x01, I32_CONST, 0, END]);
    }

    let mut export_section = Vec::new();
    leb(&mut export_section, kernels.len() as u32 + 1);
    for (index, _) in kernels.iter().enumerate() {
        let name = format!("k{index}");
        leb(&mut export_section, name.len() as u32);
        export_section.extend_from_slice(name.as_bytes());
        export_section.push(0x00);
        leb(&mut export_section, index as u32 + 1);
    }
    leb(&mut export_section, 5);
    export_section.extend_from_slice(b"error");
    export_section.push(0x03);
    leb(&mut export_section, ERROR);

    let mut code_section = Vec::new();
    leb(&mut code_section, kernels.len() as u32 + 1);
    let mut bodies = vec![remainder_body()];
    bodies.extend(kernels.iter().map(kernel_body));
    for body in bodies {
        leb(&mut code_section, body.len() as u32);
        code_section.extend_from_slice(&body);
    }

    let mut out = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
    section(&mut out, 1, &type_section);
    section(&mut out, 3, &function_section);
    section(&mut out, 6, &global_section);
    section(&mut out, 7, &export_section);
    section(&mut out, 10, &code_section);

    let exports = kernels
        .iter()
        .map(|kernel| Export {
            name: kernel.name.clone(),
            arity: kernel.signature.params.len(),
        })
        .collect();
    (out, exports)
}

fn f64_const(out: &mut Vec<u8>, value: f64) {
    out.push(F64_CONST);
    out.extend_from_slice(&value.to_le_bytes());
}

fn i32_const(out: &mut Vec<u8>, value: i32) {
    out.push(I32_CONST);
    sleb(out, i64::from(value));
}

fn local_get(out: &mut Vec<u8>, index: u32) {
    out.push(LOCAL_GET);
    leb(out, index);
}

fn remainder_body() -> Vec<u8> {
    let mut out = Vec::new();
    leb(&mut out, 1);
    leb(&mut out, 3);
    out.push(F64);
    let (x, y, r, d, t) = (0, 1, 2, 3, 4);
    let limit = 9007199254740992.0;
    let local_set = |out: &mut Vec<u8>, index: u32| {
        out.push(LOCAL_SET);
        leb(out, index);
    };

    local_get(&mut out, x);
    local_get(&mut out, x);
    out.push(F64_NE);
    local_get(&mut out, y);
    local_get(&mut out, y);
    out.push(F64_NE);
    out.push(I32_OR);
    local_get(&mut out, y);
    f64_const(&mut out, 0.0);
    out.push(F64_EQ);
    out.push(I32_OR);
    local_get(&mut out, x);
    out.push(F64_ABS);
    f64_const(&mut out, f64::INFINITY);
    out.push(F64_EQ);
    out.push(I32_OR);
    out.extend_from_slice(&[IF, F64]);
    f64_const(&mut out, f64::NAN);
    out.push(ELSE);

    local_get(&mut out, y);
    out.push(F64_ABS);
    f64_const(&mut out, f64::INFINITY);
    out.push(F64_EQ);
    out.extend_from_slice(&[IF, F64]);
    local_get(&mut out, x);
    out.push(ELSE);

    local_get(&mut out, x);
    local_get(&mut out, x);
    out.push(F64_TRUNC);
    out.push(F64_EQ);
    local_get(&mut out, y);
    local_get(&mut out, y);
    out.push(F64_TRUNC);
    out.push(F64_EQ);
    out.push(I32_AND);
    local_get(&mut out, x);
    out.push(F64_ABS);
    f64_const(&mut out, limit);
    out.push(F64_LT);
    out.push(I32_AND);
    local_get(&mut out, y);
    out.push(F64_ABS);
    f64_const(&mut out, limit);
    out.push(F64_LT);
    out.push(I32_AND);
    out.extend_from_slice(&[IF, EMPTY]);
    local_get(&mut out, x);
    out.push(I64_TRUNC_F64_S);
    local_get(&mut out, y);
    out.push(I64_TRUNC_F64_S);
    out.push(I64_REM_S);
    out.push(F64_CONVERT_I64_S);
    local_set(&mut out, r);
    out.push(ELSE);

    local_get(&mut out, x);
    out.push(F64_ABS);
    local_set(&mut out, r);
    local_get(&mut out, y);
    out.push(F64_ABS);
    local_set(&mut out, d);
    out.extend_from_slice(&[BLOCK, EMPTY, LOOP, EMPTY]);
    local_get(&mut out, r);
    local_get(&mut out, d);
    out.push(F64_LT);
    out.extend_from_slice(&[BR_IF, 1]);
    local_get(&mut out, d);
    local_set(&mut out, t);
    out.extend_from_slice(&[BLOCK, EMPTY, LOOP, EMPTY]);
    local_get(&mut out, t);
    f64_const(&mut out, 2.0);
    out.push(F64_MUL);
    local_get(&mut out, r);
    out.push(F64_GT);
    out.extend_from_slice(&[BR_IF, 1]);
    local_get(&mut out, t);
    f64_const(&mut out, 2.0);
    out.push(F64_MUL);
    local_set(&mut out, t);
    out.extend_from_slice(&[BR, 0, END, END]);
    local_get(&mut out, r);
    local_get(&mut out, t);
    out.push(F64_SUB);
    local_set(&mut out, r);
    out.extend_from_slice(&[BR, 0, END, END]);
    local_get(&mut out, x);
    f64_const(&mut out, 0.0);
    out.push(F64_LT);
    out.extend_from_slice(&[IF, F64]);
    local_get(&mut out, r);
    out.push(F64_NEG);
    out.push(ELSE);
    local_get(&mut out, r);
    out.push(END);
    local_set(&mut out, r);
    out.push(END);

    local_get(&mut out, r);
    f64_const(&mut out, 0.0);
    out.push(F64_EQ);
    local_get(&mut out, x);
    f64_const(&mut out, 0.0);
    out.push(F64_LT);
    out.push(I32_AND);
    out.extend_from_slice(&[IF, F64]);
    f64_const(&mut out, -0.0);
    out.push(ELSE);
    local_get(&mut out, r);
    out.push(END);
    out.push(END);
    out.push(END);
    out.push(END);
    out
}

struct Emitter<'a> {
    out: Vec<u8>,
    kernel: &'a Kernel,
    labels: u32,
    loops: Vec<(u32, u32)>,
}

fn kernel_body(kernel: &Kernel) -> Vec<u8> {
    let mut out = Vec::new();
    let extra = &kernel.locals[kernel.signature.params.len()..];
    let mut runs: Vec<(u32, u8)> = Vec::new();
    for kind in extra {
        match runs.last_mut() {
            Some((count, ty)) if *ty == valtype(*kind) => *count += 1,
            _ => runs.push((1, valtype(*kind))),
        }
    }
    leb(&mut out, runs.len() as u32);
    for (count, ty) in runs {
        leb(&mut out, count);
        out.push(ty);
    }

    let mut emitter = Emitter {
        out,
        kernel,
        labels: 0,
        loops: Vec::new(),
    };
    emitter.adjust_depth(1);
    emitter.out.push(GLOBAL_GET);
    leb(&mut emitter.out, DEPTH);
    i32_const(&mut emitter.out, MAX_DEPTH);
    emitter.out.push(I32_GT_S);
    emitter.out.extend_from_slice(&[IF, EMPTY]);
    i32_const(&mut emitter.out, 1);
    emitter.out.push(GLOBAL_SET);
    leb(&mut emitter.out, ERROR);
    emitter.adjust_depth(-1);
    emitter.failure();
    emitter.out.push(RETURN);
    emitter.out.push(END);

    emitter.out.push(BLOCK);
    emitter.out.push(valtype(kernel.signature.returns));
    emitter.labels = 1;
    emitter.block(&kernel.body);
    emitter.out.push(UNREACHABLE);
    emitter.out.push(END);
    emitter.adjust_depth(-1);
    emitter.out.push(END);
    emitter.out
}

impl Emitter<'_> {
    fn adjust_depth(&mut self, by: i32) {
        self.out.push(GLOBAL_GET);
        leb(&mut self.out, DEPTH);
        i32_const(&mut self.out, 1);
        self.out.push(if by > 0 { I32_ADD } else { I32_SUB });
        self.out.push(GLOBAL_SET);
        leb(&mut self.out, DEPTH);
    }

    fn failure(&mut self) {
        match self.kernel.signature.returns {
            Kind::Num => f64_const(&mut self.out, f64::NAN),
            Kind::Bool => i32_const(&mut self.out, 0),
        }
    }

    fn exit(&mut self) {
        self.out.push(BR);
        leb(&mut self.out, self.labels - 1);
    }

    fn block(&mut self, block: &[Stmt]) {
        for stmt in block {
            self.statement(stmt);
            if matches!(stmt, Stmt::Return(_) | Stmt::Break | Stmt::Continue) {
                return;
            }
        }
    }

    fn statement(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Set(slot, value) => {
                self.expression(value);
                self.out.push(LOCAL_SET);
                leb(&mut self.out, *slot);
            }
            Stmt::Discard(value) => {
                self.expression(value);
                self.out.push(0x1a);
            }
            Stmt::Return(value) => {
                self.expression(value);
                self.exit();
            }
            Stmt::If(condition, then, otherwise) => {
                self.expression(condition);
                self.out.extend_from_slice(&[IF, EMPTY]);
                self.labels += 1;
                self.block(then);
                if !otherwise.is_empty() {
                    self.out.push(ELSE);
                    self.block(otherwise);
                }
                self.labels -= 1;
                self.out.push(END);
            }
            Stmt::While(condition, body) => {
                self.out.extend_from_slice(&[BLOCK, EMPTY, LOOP, EMPTY]);
                self.labels += 2;
                let (header, after) = (self.labels - 1, self.labels - 2);
                self.expression(condition);
                self.out.push(I32_EQZ);
                self.out.push(BR_IF);
                leb(&mut self.out, 1);
                self.loops.push((header, after));
                self.block(body);
                self.loops.pop();
                self.out.push(BR);
                leb(&mut self.out, 0);
                self.labels -= 2;
                self.out.extend_from_slice(&[END, END]);
            }
            Stmt::Break => {
                let (_, after) = self.loops.last().expect("break inside a loop");
                self.out.push(BR);
                leb(&mut self.out, self.labels - 1 - after);
            }
            Stmt::Continue => {
                let (header, _) = self.loops.last().expect("continue inside a loop");
                self.out.push(BR);
                leb(&mut self.out, self.labels - 1 - header);
            }
        }
    }

    fn expression(&mut self, expr: &Expr) {
        match expr {
            Expr::Num(n) => f64_const(&mut self.out, *n),
            Expr::Bool(flag) => i32_const(&mut self.out, i32::from(*flag)),
            Expr::Local(slot) => local_get(&mut self.out, *slot),
            Expr::Neg(inner) => {
                self.expression(inner);
                self.out.push(F64_NEG);
            }
            Expr::Not(inner) => {
                self.expression(inner);
                self.out.push(I32_EQZ);
            }
            Expr::Arith(op, left, right) => {
                self.expression(left);
                self.expression(right);
                match op {
                    Arith::Add => self.out.push(F64_ADD),
                    Arith::Sub => self.out.push(F64_SUB),
                    Arith::Mul => self.out.push(F64_MUL),
                    Arith::Div => self.out.push(F64_DIV),
                    Arith::Rem => {
                        self.out.push(CALL);
                        leb(&mut self.out, REMAINDER);
                    }
                }
            }
            Expr::Compare(cmp, kind, left, right) => {
                self.expression(left);
                self.expression(right);
                self.out.push(match (kind, cmp) {
                    (Kind::Num, Cmp::Eq) => F64_EQ,
                    (Kind::Num, Cmp::Ne) => F64_NE,
                    (Kind::Num, Cmp::Lt) => F64_LT,
                    (Kind::Num, Cmp::Le) => F64_LE,
                    (Kind::Num, Cmp::Gt) => F64_GT,
                    (Kind::Num, Cmp::Ge) => F64_GE,
                    (Kind::Bool, Cmp::Eq) => I32_EQ,
                    (Kind::Bool, Cmp::Ne) => I32_NE,
                    (Kind::Bool, Cmp::Lt) => I32_LT_U,
                    (Kind::Bool, Cmp::Le) => I32_LE_U,
                    (Kind::Bool, Cmp::Gt) => I32_GT_U,
                    (Kind::Bool, Cmp::Ge) => I32_GE_U,
                });
            }
            Expr::And(left, right) => {
                self.expression(left);
                self.out.extend_from_slice(&[IF, I32]);
                self.labels += 1;
                self.expression(right);
                self.out.push(ELSE);
                i32_const(&mut self.out, 0);
                self.labels -= 1;
                self.out.push(END);
            }
            Expr::Or(left, right) => {
                self.expression(left);
                self.out.extend_from_slice(&[IF, I32]);
                self.labels += 1;
                i32_const(&mut self.out, 1);
                self.out.push(ELSE);
                self.expression(right);
                self.labels -= 1;
                self.out.push(END);
            }
            Expr::Call(index, args) => {
                for arg in args {
                    self.expression(arg);
                }
                self.out.push(CALL);
                leb(&mut self.out, *index + 1);
                self.out.push(GLOBAL_GET);
                leb(&mut self.out, ERROR);
                self.out.extend_from_slice(&[IF, EMPTY]);
                self.labels += 1;
                self.failure();
                self.exit();
                self.labels -= 1;
                self.out.push(END);
            }
        }
    }
}
