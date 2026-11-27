use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::error::Error;
use crate::kernel::{Kind, LoopSource, Place, Source};
use crate::native::Tier;
use crate::parser::{Arm, ArmBody, Block, Expr, Import, Op, Pattern, Stmt, Type};
use crate::value::{EnumDef, FieldCache, Value};

#[derive(Debug, Clone, Copy)]
pub enum Check {
    If,
    While,
    And,
    Or,
}

impl Check {
    pub fn label(self) -> &'static str {
        match self {
            Check::If => "if",
            Check::While => "while",
            Check::And => "'&&'",
            Check::Or => "'||'",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Code {
    Constant(u32),
    None,
    True,
    False,
    Pop,
    PopN(u32),
    GetLocal(u32),
    SetLocal(u32),
    MakeCell(u32),
    GetCell(u32),
    SetCell(u32),
    GetCaptured(u32),
    SetCaptured(u32),
    GetGlobal(u32),
    SetGlobal(u32),
    DefineGlobal(u32),
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Equal,
    NotEqual,
    Negate,
    Not,
    Jump(u32),
    JumpIfFalse(u32, Check),
    JumpUnlessLess(u32),
    JumpUnlessLessEqual(u32),
    JumpUnlessGreater(u32),
    JumpUnlessGreaterEqual(u32),
    JumpUnlessEqual(u32),
    JumpUnlessNotEqual(u32),
    JumpUnlessLessConst(u32, u32),
    JumpUnlessLessEqualConst(u32, u32),
    JumpUnlessGreaterConst(u32, u32),
    JumpUnlessGreaterEqualConst(u32, u32),
    JumpUnlessEqualConst(u32, u32),
    JumpUnlessNotEqualConst(u32, u32),
    AddConst(u32),
    SubConst(u32),
    UpdateGlobal(u32, Op),
    UpdateGlobalConst(u32, Op, u32),
    UpdateLocal(u32, Op),
    UpdateLocalConst(u32, Op, u32),
    JumpIfFalseKeep(u32, Check),
    JumpIfTrueKeep(u32, Check),
    CheckBool(Check),
    JumpIfNotNoneKeep(u32),
    JumpIfNoneKeep(u32),
    Call(u32),
    CallGlobal(u32, u32),
    Return,
    Closure(u32),
    List(u32),
    Record(u32),
    Index,
    SetIndex,
    GetField(u32),
    SetField(u32),
    Concat(u32),
    Stringify,
    MakeStructDef(u32),
    MakeStruct(u32),
    MakeEnumDef(u32),
    Invoke(u32, u32),
    Test(u32, u32),
    Squash(u32),
    MatchFail(u32),
    IterInit,
    ForNext(u32, u32),
    Import(u32),
    ImportName(u32, u32),
    MoveLocal(u32, u32),
    ReturnLocal(u32),
    BinaryLocals(u32, u32, Op),
    JumpUnlessLocals(u32, u32, Op, u32),
    LocalAddConst(u32, u32),
    LocalSubConst(u32, u32),
    UpdateLocalWithLocal(u32, u32, Op),
    UpdateGlobalWithGlobal(u32, u32, Op),
    PopJump(u32, u32),
    GetLocalField(u32, u32),
    GetGlobalField(u32, u32),
    IndexLocals(u32, u32),
    JumpUnlessLocalConst(u32, u32, Op, u32),
    ReturnBinary(Op),
    Loop(u32, u32),
}

fn fuse(code: &[Code], label: usize, next: Code) -> Option<(usize, Code)> {
    let len = code.len();
    let last = *code.last()?;
    let free = |back: usize| label + back <= len;
    let one = |fused: Code| free(1).then_some((1, fused));
    let two = |fused: Code| free(2).then_some((2, fused));
    let before = || (len >= 2).then(|| code[len - 2]);
    match (next, last) {
        (Code::SetLocal(to), Code::GetLocal(from)) => one(Code::MoveLocal(from, to)),
        (Code::Return, Code::GetLocal(slot)) => one(Code::ReturnLocal(slot)),
        (Code::Return, binary) => one(Code::ReturnBinary(binary_op(binary)?)),
        (Code::AddConst(c), Code::GetLocal(slot)) => one(Code::LocalAddConst(slot, c)),
        (Code::SubConst(c), Code::GetLocal(slot)) => one(Code::LocalSubConst(slot, c)),
        (Code::UpdateLocal(slot, op), Code::GetLocal(from)) => {
            one(Code::UpdateLocalWithLocal(slot, from, op))
        }
        (Code::UpdateGlobal(slot, op), Code::GetGlobal(from)) => {
            one(Code::UpdateGlobalWithGlobal(slot, from, op))
        }
        (Code::Jump(target), Code::PopN(count)) => one(Code::PopJump(count, target)),
        (Code::GetField(name), Code::GetLocal(slot)) => one(Code::GetLocalField(slot, name)),
        (Code::GetField(name), Code::GetGlobal(slot)) => one(Code::GetGlobalField(slot, name)),
        (Code::Index, Code::GetLocal(index)) => match before()? {
            Code::GetLocal(list) => two(Code::IndexLocals(list, index)),
            _ => None,
        },
        (binary, Code::GetLocal(b)) => match (binary_op(binary)?, before()?) {
            (op, Code::GetLocal(a)) => two(Code::BinaryLocals(a, b, op)),
            _ => None,
        },
        _ => None,
    }
}

fn binary_op(code: Code) -> Option<Op> {
    Some(match code {
        Code::Add => Op::Add,
        Code::Sub => Op::Sub,
        Code::Mul => Op::Mul,
        Code::Div => Op::Div,
        Code::Rem => Op::Rem,
        Code::Less => Op::Less,
        Code::LessEqual => Op::LessEqual,
        Code::Greater => Op::Greater,
        Code::GreaterEqual => Op::GreaterEqual,
        Code::Equal => Op::Equal,
        Code::NotEqual => Op::NotEqual,
        _ => return None,
    })
}

fn comparison_op(op: Op) -> bool {
    matches!(
        op,
        Op::Less | Op::LessEqual | Op::Greater | Op::GreaterEqual | Op::Equal | Op::NotEqual
    )
}

fn comparison(code: Code) -> Option<Op> {
    binary_op(code).filter(|op| comparison_op(*op))
}

pub struct StructInfo {
    pub name: Rc<str>,
    pub fields: Rc<[Rc<str>]>,
    pub methods: Vec<Rc<str>>,
}

pub enum PatternTest {
    Value {
        slot: u32,
        constant: u32,
    },
    Variant {
        slot: u32,
        enum_name: Option<Rc<str>>,
        variant: Rc<str>,
        arity: u32,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum Capture {
    Local(u32),
    Outer(u32),
}

pub struct Proto {
    pub name: Rc<str>,
    pub arity: u32,
    pub code: Vec<Code>,
    pub lines: Vec<u32>,
    pub constants: Vec<Value>,
    pub protos: Vec<Rc<Proto>>,
    pub shapes: Vec<Rc<[Rc<str>]>>,
    pub captures: Vec<Capture>,
    pub structs: Vec<StructInfo>,
    pub enums: Vec<Rc<EnumDef>>,
    pub patterns: Vec<PatternTest>,
    pub builtin: bool,
    pub source: Option<Rc<Source>>,
    pub top_level: bool,
    pub tier: Cell<Tier>,
    pub loops: Vec<Rc<LoopSource>>,
    pub caches: Vec<FieldCache>,
    pub kernel_struct_layouts: Vec<(String, Vec<(String, Kind)>)>,
}

impl Proto {
    fn new(name: &str, arity: usize) -> Self {
        Proto {
            name: Rc::from(name),
            arity: arity as u32,
            code: Vec::new(),
            lines: Vec::new(),
            constants: Vec::new(),
            protos: Vec::new(),
            shapes: Vec::new(),
            captures: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            patterns: Vec::new(),
            builtin: false,
            source: None,
            top_level: false,
            tier: Cell::new(Tier::default()),
            loops: Vec::new(),
            caches: Vec::new(),
            kernel_struct_layouts: Vec::new(),
        }
    }

    pub fn finish(&mut self) {
        self.caches = (0..self.code.len())
            .map(|_| FieldCache::default())
            .collect();
    }

    pub fn label(&self) -> &str {
        if self.name.is_empty() {
            "anonymous"
        } else {
            &self.name
        }
    }
}

#[derive(Default)]
pub struct Globals {
    slots: HashMap<String, u32>,
    pub names: Vec<String>,
}

impl Globals {
    pub fn slot(&mut self, name: &str) -> u32 {
        if let Some(slot) = self.slots.get(name) {
            return *slot;
        }
        let slot = self.names.len() as u32;
        self.slots.insert(name.to_string(), slot);
        self.names.push(name.to_string());
        slot
    }
}

struct Local {
    name: String,
    depth: usize,
    cell: bool,
}

struct Loop {
    start: usize,
    locals: usize,
    breaks: Vec<usize>,
}

struct FnState {
    proto: Proto,
    locals: Vec<Local>,
    depth: usize,
    captured: HashSet<String>,
    capture_names: Vec<String>,
    loops: Vec<Loop>,
    label: usize,
}

impl FnState {
    fn new(name: &str, arity: usize, captured: HashSet<String>) -> Self {
        FnState {
            proto: Proto::new(name, arity),
            locals: Vec::new(),
            depth: 0,
            captured,
            capture_names: Vec::new(),
            loops: Vec::new(),
            label: 0,
        }
    }

    fn find_local(&self, name: &str) -> Option<(u32, bool)> {
        self.locals
            .iter()
            .enumerate()
            .rev()
            .find(|(_, local)| local.name == name)
            .map(|(slot, local)| (slot as u32, local.cell))
    }
}

enum Var {
    Local(u32),
    Cell(u32),
    Captured(u32),
    Global(u32),
}

pub struct Compiler<'g> {
    fns: Vec<FnState>,
    globals: &'g mut Globals,
    line: u32,
    promote: bool,
}

pub fn mark_builtin(proto: &mut Proto) {
    proto.builtin = true;
    for inner in &mut proto.protos {
        if let Some(inner) = Rc::get_mut(inner) {
            mark_builtin(inner);
        }
    }
}

pub fn compile_script(
    program: &Block,
    globals: &mut Globals,
    promote: bool,
) -> Result<(Proto, bool), Error> {
    let mut compiler = Compiler {
        fns: vec![FnState::new("", 0, captured_names(program))],
        globals,
        line: 1,
        promote,
    };
    let ends_with_value = matches!(program.last(), Some((_, Stmt::Expr(_))));

    for (index, (line, stmt)) in program.iter().enumerate() {
        compiler.line = *line as u32;
        match stmt {
            Stmt::Expr(expr) if index + 1 == program.len() => {
                compiler.value(expr)?;
                compiler.emit(Code::Return);
            }
            _ => compiler.statement(stmt)?,
        }
    }
    if !ends_with_value {
        compiler.emit(Code::None);
        compiler.emit(Code::Return);
    }

    let state = compiler
        .fns
        .pop()
        .expect("the script function always exists");
    let mut proto = state.proto;
    proto.finish();
    let layouts = crate::kernel::struct_layouts(program);
    share_struct_layouts(&mut proto, &layouts);
    Ok((proto, ends_with_value))
}

fn share_struct_layouts(proto: &mut Proto, layouts: &[(String, Vec<(String, Kind)>)]) {
    proto.kernel_struct_layouts = layouts.to_vec();
    for inner in &mut proto.protos {
        if let Some(inner) = Rc::get_mut(inner) {
            share_struct_layouts(inner, layouts);
        }
    }
}

impl Compiler<'_> {
    fn current(&mut self) -> &mut FnState {
        self.fns
            .last_mut()
            .expect("there is always a function being compiled")
    }

    fn emit(&mut self, code: Code) -> usize {
        let line = self.line;
        let state = self.current();
        let (code, line) = match fuse(&state.proto.code, state.label, code) {
            Some((replaced, fused)) => {
                let keep = state.proto.code.len() - replaced;
                state.proto.code.truncate(keep);
                state.proto.lines.truncate(keep);
                (fused, line)
            }
            None => (code, line),
        };
        let proto = &mut state.proto;
        proto.code.push(code);
        proto.lines.push(line);
        proto.code.len() - 1
    }

    fn here(&mut self) -> u32 {
        let state = self.current();
        state.label = state.proto.code.len();
        state.label as u32
    }

    fn patch(&mut self, at: usize) {
        let target = self.here();
        self.current().label = target as usize;
        let code = &mut self.current().proto.code[at];
        *code = match *code {
            Code::Jump(_) => Code::Jump(target),
            Code::PopJump(count, _) => Code::PopJump(count, target),
            Code::JumpUnlessLocals(a, b, op, _) => Code::JumpUnlessLocals(a, b, op, target),
            Code::JumpUnlessLocalConst(s, c, op, _) => Code::JumpUnlessLocalConst(s, c, op, target),
            Code::Loop(index, _) => Code::Loop(index, target),
            Code::JumpIfFalse(_, check) => Code::JumpIfFalse(target, check),
            Code::JumpUnlessLess(_) => Code::JumpUnlessLess(target),
            Code::JumpUnlessLessEqual(_) => Code::JumpUnlessLessEqual(target),
            Code::JumpUnlessGreater(_) => Code::JumpUnlessGreater(target),
            Code::JumpUnlessGreaterEqual(_) => Code::JumpUnlessGreaterEqual(target),
            Code::JumpUnlessEqual(_) => Code::JumpUnlessEqual(target),
            Code::JumpUnlessNotEqual(_) => Code::JumpUnlessNotEqual(target),
            Code::JumpUnlessLessConst(c, _) => Code::JumpUnlessLessConst(c, target),
            Code::JumpUnlessLessEqualConst(c, _) => Code::JumpUnlessLessEqualConst(c, target),
            Code::JumpUnlessGreaterConst(c, _) => Code::JumpUnlessGreaterConst(c, target),
            Code::JumpUnlessGreaterEqualConst(c, _) => Code::JumpUnlessGreaterEqualConst(c, target),
            Code::JumpUnlessEqualConst(c, _) => Code::JumpUnlessEqualConst(c, target),
            Code::JumpUnlessNotEqualConst(c, _) => Code::JumpUnlessNotEqualConst(c, target),
            Code::JumpIfFalseKeep(_, check) => Code::JumpIfFalseKeep(target, check),
            Code::JumpIfTrueKeep(_, check) => Code::JumpIfTrueKeep(target, check),
            Code::JumpIfNotNoneKeep(_) => Code::JumpIfNotNoneKeep(target),
            Code::JumpIfNoneKeep(_) => Code::JumpIfNoneKeep(target),
            Code::ForNext(slot, _) => Code::ForNext(slot, target),
            Code::Test(pattern, _) => Code::Test(pattern, target),
            other => other,
        };
    }

    fn jump_unless(&mut self, check: Check) -> usize {
        let state = self.current();
        let here = state.proto.code.len();
        if let [.., Code::GetLocal(slot), Code::Constant(c), compare] = state.proto.code[..]
            && state.label + 3 <= here
            && let Some(op) = comparison(compare)
        {
            let line = state.proto.lines[here - 1];
            state.proto.code.truncate(here - 3);
            state.proto.lines.truncate(here - 3);
            state
                .proto
                .code
                .push(Code::JumpUnlessLocalConst(slot, c, op, 0));
            state.proto.lines.push(line);
            return here - 3;
        }
        let last_two = match state.proto.code[..] {
            [.., Code::Constant(c), compare] if state.label != here && state.label != here - 1 => {
                match compare {
                    Code::Less => Some(Code::JumpUnlessLessConst(c, 0)),
                    Code::LessEqual => Some(Code::JumpUnlessLessEqualConst(c, 0)),
                    Code::Greater => Some(Code::JumpUnlessGreaterConst(c, 0)),
                    Code::GreaterEqual => Some(Code::JumpUnlessGreaterEqualConst(c, 0)),
                    Code::Equal => Some(Code::JumpUnlessEqualConst(c, 0)),
                    Code::NotEqual => Some(Code::JumpUnlessNotEqualConst(c, 0)),
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(code) = last_two {
            state.proto.code.truncate(here - 2);
            let line = state.proto.lines[here - 1];
            state.proto.lines.truncate(here - 2);
            state.proto.code.push(code);
            state.proto.lines.push(line);
            return here - 2;
        }
        let fused = match state.proto.code.last() {
            _ if state.label == here => None,
            Some(Code::BinaryLocals(a, b, op)) if comparison_op(*op) => {
                Some(Code::JumpUnlessLocals(*a, *b, *op, 0))
            }
            Some(Code::Less) => Some(Code::JumpUnlessLess(0)),
            Some(Code::LessEqual) => Some(Code::JumpUnlessLessEqual(0)),
            Some(Code::Greater) => Some(Code::JumpUnlessGreater(0)),
            Some(Code::GreaterEqual) => Some(Code::JumpUnlessGreaterEqual(0)),
            Some(Code::Equal) => Some(Code::JumpUnlessEqual(0)),
            Some(Code::NotEqual) => Some(Code::JumpUnlessNotEqual(0)),
            _ => None,
        };
        match fused {
            Some(code) => {
                let line = state.proto.lines.pop().expect("the comparison has a line");
                state.proto.code.pop();
                state.proto.code.push(code);
                state.proto.lines.push(line);
                here - 1
            }
            None => self.emit(Code::JumpIfFalse(0, check)),
        }
    }

    fn constant(&mut self, value: Value) -> u32 {
        let constants = &mut self.current().proto.constants;
        if let Value::Str(text) = &value
            && let Some(index) = constants
                .iter()
                .position(|c| matches!(c, Value::Str(existing) if existing == text))
        {
            return index as u32;
        }
        constants.push(value);
        (constants.len() - 1) as u32
    }

    fn name_constant(&mut self, name: &str) -> u32 {
        self.constant(Value::str(name))
    }

    fn at_global_level(&self) -> bool {
        self.fns.len() == 1 && self.fns[0].depth == 0
    }

    fn defines_global(&self, name: &str) -> bool {
        self.at_global_level() && (!self.promote || self.fns[0].captured.contains(name))
    }

    fn begin_scope(&mut self) {
        self.current().depth += 1;
    }

    fn end_scope(&mut self) {
        let state = self.current();
        state.depth -= 1;
        let depth = state.depth;
        let keep = state
            .locals
            .iter()
            .position(|local| local.depth > depth)
            .unwrap_or(state.locals.len());
        let removed = state.locals.len() - keep;
        state.locals.truncate(keep);
        if removed > 0 {
            self.emit(Code::PopN(removed as u32));
        }
    }

    fn scoped(&mut self, block: &Block) -> Result<(), Error> {
        self.begin_scope();
        self.statements(block)?;
        self.end_scope();
        Ok(())
    }

    fn declare_local(&mut self, name: &str, cell: bool) -> u32 {
        let state = self.current();
        state.locals.push(Local {
            name: name.to_string(),
            depth: state.depth,
            cell,
        });
        (state.locals.len() - 1) as u32
    }

    fn define(&mut self, name: &str) {
        if self.defines_global(name) {
            let slot = self.globals.slot(name);
            self.emit(Code::DefineGlobal(slot));
        } else {
            let boxed = self.current().captured.contains(name);
            let slot = self.declare_local(name, boxed);
            if boxed {
                self.emit(Code::MakeCell(slot));
            }
        }
    }

    fn resolve(&mut self, name: &str) -> Result<Var, Error> {
        let top = self.fns.len() - 1;
        if let Some((slot, cell)) = self.fns[top].find_local(name) {
            return Ok(if cell {
                Var::Cell(slot)
            } else {
                Var::Local(slot)
            });
        }
        if let Some(index) = self.resolve_capture(top, name)? {
            return Ok(Var::Captured(index));
        }
        Ok(Var::Global(self.globals.slot(name)))
    }

    fn resolve_capture(&mut self, fn_index: usize, name: &str) -> Result<Option<u32>, Error> {
        if fn_index == 0 {
            return Ok(None);
        }
        let enclosing = fn_index - 1;
        if let Some((slot, cell)) = self.fns[enclosing].find_local(name) {
            if !cell {
                return Err(Error::new(
                    self.line as usize,
                    format!("internal error: '{name}' is captured but not boxed"),
                ));
            }
            return Ok(Some(self.add_capture(fn_index, Capture::Local(slot), name)));
        }
        match self.resolve_capture(enclosing, name)? {
            Some(outer) => Ok(Some(self.add_capture(
                fn_index,
                Capture::Outer(outer),
                name,
            ))),
            None => Ok(None),
        }
    }

    fn add_capture(&mut self, fn_index: usize, capture: Capture, name: &str) -> u32 {
        let state = &mut self.fns[fn_index];
        if let Some(index) = state.capture_names.iter().position(|n| n == name) {
            return index as u32;
        }
        state.capture_names.push(name.to_string());
        state.proto.captures.push(capture);
        (state.proto.captures.len() - 1) as u32
    }

    fn get_variable(&mut self, name: &str) -> Result<(), Error> {
        let code = match self.resolve(name)? {
            Var::Local(slot) => Code::GetLocal(slot),
            Var::Cell(slot) => Code::GetCell(slot),
            Var::Captured(index) => Code::GetCaptured(index),
            Var::Global(slot) => Code::GetGlobal(slot),
        };
        self.emit(code);
        Ok(())
    }

    fn set_variable(&mut self, name: &str) -> Result<(), Error> {
        let code = match self.resolve(name)? {
            Var::Local(slot) => Code::SetLocal(slot),
            Var::Cell(slot) => Code::SetCell(slot),
            Var::Captured(index) => Code::SetCaptured(index),
            Var::Global(slot) => Code::SetGlobal(slot),
        };
        self.emit(code);
        Ok(())
    }

    fn statements(&mut self, block: &Block) -> Result<(), Error> {
        for (line, stmt) in block {
            self.line = *line as u32;
            self.statement(stmt)?;
        }
        Ok(())
    }

    fn statement(&mut self, stmt: &Stmt) -> Result<(), Error> {
        match stmt {
            Stmt::Let(name, _, Expr::Function { .. }) if self.defines_global(name) => {
                self.value(expr_of(stmt))?;
                if let Some(proto) = self.current().proto.protos.last_mut()
                    && let Some(proto) = Rc::get_mut(proto)
                {
                    proto.top_level = true;
                }
                self.define(name);
            }
            Stmt::Let(name, _, expr @ Expr::Function { .. }) if !self.defines_global(name) => {
                self.emit(Code::None);
                let boxed = self.current().captured.contains(name);
                let slot = self.declare_local(name, boxed);
                if boxed {
                    self.emit(Code::MakeCell(slot));
                }
                self.expression(expr)?;
                self.emit(if boxed {
                    Code::SetCell(slot)
                } else {
                    Code::SetLocal(slot)
                });
            }
            Stmt::Let(name, _, expr) => {
                self.value(expr)?;
                self.define(name);
            }
            Stmt::Assign(name, expr @ Expr::Binary(left, op @ (Op::Add | Op::Sub), right)) if matches!(&**left, Expr::Variable(target) if target == name) =>
            {
                let constant = match **right {
                    Expr::Number(n) => Some(self.constant(Value::Number(n))),
                    _ => None,
                };
                let code = match (self.resolve(name)?, constant) {
                    (Var::Global(slot), Some(c)) => Some(Code::UpdateGlobalConst(slot, *op, c)),
                    (Var::Local(slot), Some(c)) => Some(Code::UpdateLocalConst(slot, *op, c)),
                    (Var::Global(slot), None) => Some(Code::UpdateGlobal(slot, *op)),
                    (Var::Local(slot), None) => Some(Code::UpdateLocal(slot, *op)),
                    _ => None,
                };
                match code {
                    Some(code @ (Code::UpdateGlobalConst(..) | Code::UpdateLocalConst(..))) => {
                        self.emit(code);
                    }
                    Some(code) => {
                        self.expression(right)?;
                        self.emit(code);
                    }
                    None => {
                        self.expression(expr)?;
                        self.set_variable(name)?;
                    }
                }
            }
            Stmt::Assign(name, expr) => {
                self.value(expr)?;
                self.set_variable(name)?;
            }
            Stmt::SetIndex(target, index, value) => {
                self.expression(target)?;
                self.expression(index)?;
                self.expression(value)?;
                self.emit(Code::SetIndex);
            }
            Stmt::SetField(target, name, value) => {
                self.expression(target)?;
                self.expression(value)?;
                let name = self.name_constant(name);
                self.emit(Code::SetField(name));
            }
            Stmt::If(condition, then_block, else_block) => {
                self.expression(condition)?;
                let to_else = self.jump_unless(Check::If);
                self.scoped(then_block)?;
                match else_block {
                    Some(else_block) => {
                        let to_end = self.emit(Code::Jump(0));
                        self.patch(to_else);
                        self.scoped(else_block)?;
                        self.patch(to_end);
                    }
                    None => self.patch(to_else),
                }
            }
            Stmt::While(condition, body) => {
                let native = self.native_loop(stmt);
                let start = self.here();
                self.expression(condition)?;
                let exit = self.jump_unless(Check::While);
                self.begin_loop(start);
                self.scoped(body)?;
                self.emit(Code::Jump(start));
                self.patch(exit);
                self.end_loop();
                if let Some(at) = native {
                    self.patch(at);
                }
            }
            Stmt::For(name, iterable, body) => {
                let native = self.native_loop(stmt);
                self.begin_scope();
                self.expression(iterable)?;
                self.emit(Code::IterInit);
                let items = self.declare_local("", false);
                let zero = self.constant(Value::Number(0.0));
                self.emit(Code::Constant(zero));
                self.declare_local("", false);

                let start = self.here();
                let next = self.emit(Code::ForNext(items, 0));
                self.begin_loop(start);
                self.begin_scope();
                let boxed = self.current().captured.contains(name);
                let slot = self.declare_local(name, boxed);
                if boxed {
                    self.emit(Code::MakeCell(slot));
                }
                self.statements(body)?;
                self.end_scope();
                self.emit(Code::Jump(start));
                self.patch(next);
                self.end_loop();
                self.end_scope();
                if let Some(at) = native {
                    self.patch(at);
                }
            }
            Stmt::Break | Stmt::Continue => {
                let state = self.current();
                let loop_state = state
                    .loops
                    .last()
                    .expect("the parser only allows loops here");
                let extra = state.locals.len() - loop_state.locals;
                let start = loop_state.start as u32;
                if extra > 0 {
                    self.emit(Code::PopN(extra as u32));
                }
                if matches!(stmt, Stmt::Break) {
                    let jump = self.emit(Code::Jump(0));
                    self.current()
                        .loops
                        .last_mut()
                        .expect("still inside the loop")
                        .breaks
                        .push(jump);
                } else {
                    self.emit(Code::Jump(start));
                }
            }
            Stmt::Return(expr) => {
                match expr {
                    Some(expr) => self.value(expr)?,
                    None => {
                        self.emit(Code::None);
                    }
                }
                self.emit(Code::Return);
            }
            Stmt::Import(path, binding) => {
                let path = self.name_constant(path);
                match binding {
                    Import::Module(name) => {
                        self.emit(Code::Import(path));
                        self.define(name);
                    }
                    Import::Names(names) => {
                        for name in names {
                            let field = self.name_constant(name);
                            self.emit(Code::Import(path));
                            self.emit(Code::ImportName(path, field));
                            self.define(name);
                        }
                    }
                }
            }
            Stmt::Struct(decl) => {
                let local = self.declare_first(&decl.name);
                for (_, method) in &decl.methods {
                    self.expression(method)?;
                }
                let structs = &mut self.current().proto.structs;
                structs.push(StructInfo {
                    name: Rc::from(decl.name.as_str()),
                    fields: decl
                        .fields
                        .iter()
                        .map(|(field, _)| Rc::from(field.as_str()))
                        .collect(),
                    methods: decl
                        .methods
                        .iter()
                        .map(|(method, _)| Rc::from(method.as_str()))
                        .collect(),
                });
                let index = (structs.len() - 1) as u32;
                self.emit(Code::MakeStructDef(index));
                self.finish_declaration(&decl.name, local);
            }
            Stmt::Enum(decl) => {
                let enums = &mut self.current().proto.enums;
                enums.push(Rc::new(EnumDef {
                    name: Rc::from(decl.name.as_str()),
                    variants: decl
                        .variants
                        .iter()
                        .map(|(variant, payload)| (Rc::from(variant.as_str()), payload.len()))
                        .collect(),
                }));
                let index = (enums.len() - 1) as u32;
                self.emit(Code::MakeEnumDef(index));
                self.define(&decl.name);
            }
            Stmt::Test(..) => {}
            Stmt::Expr(Expr::Match(scrutinee, arms)) => self.match_expr(scrutinee, arms, false)?,
            Stmt::Expr(expr) => {
                self.expression(expr)?;
                self.emit(Code::Pop);
            }
        }
        Ok(())
    }

    fn native_loop(&mut self, stmt: &Stmt) -> Option<usize> {
        let mut vars = Vec::new();
        let (used, called) = crate::kernel::loop_names(stmt);
        for name in &called {
            if !matches!(self.resolve(name).ok()?, Var::Global(_)) {
                return None;
            }
        }
        for name in used {
            let place = match self.resolve(&name).ok()? {
                Var::Local(slot) => Place::Local(slot),
                Var::Global(slot) => Place::Global(slot),
                Var::Cell(_) | Var::Captured(_) => return None,
            };
            vars.push((name, place));
        }
        if vars.len() > 16 {
            return None;
        }
        let line = self.line as usize;
        let state = self.current();
        let source = LoopSource {
            owner: state.proto.name.to_string(),
            line,
            index: state.proto.loops.len() as u32,
            stmt: stmt.clone(),
            vars,
            tier: Cell::new(Tier::default()),
        };
        state.proto.loops.push(Rc::new(source));
        let index = (self.current().proto.loops.len() - 1) as u32;
        Some(self.emit(Code::Loop(index, 0)))
    }

    fn declare_first(&mut self, name: &str) -> Option<(u32, bool)> {
        if self.defines_global(name) {
            return None;
        }
        self.emit(Code::None);
        let boxed = self.current().captured.contains(name);
        let slot = self.declare_local(name, boxed);
        if boxed {
            self.emit(Code::MakeCell(slot));
        }
        Some((slot, boxed))
    }

    fn finish_declaration(&mut self, name: &str, local: Option<(u32, bool)>) {
        match local {
            Some((slot, true)) => {
                self.emit(Code::SetCell(slot));
            }
            Some((slot, false)) => {
                self.emit(Code::SetLocal(slot));
            }
            None => {
                let slot = self.globals.slot(name);
                self.emit(Code::DefineGlobal(slot));
            }
        }
    }

    fn value(&mut self, expr: &Expr) -> Result<(), Error> {
        match expr {
            Expr::Match(scrutinee, arms) => self.match_expr(scrutinee, arms, true),
            other => self.expression(other),
        }
    }

    fn bind(&mut self, name: &str) {
        if name == "_" {
            self.declare_local("", false);
            return;
        }
        let boxed = self.current().captured.contains(name);
        let slot = self.declare_local(name, boxed);
        if boxed {
            self.emit(Code::MakeCell(slot));
        }
    }

    fn leave_locals(&mut self, keep: usize) -> usize {
        let state = self.current();
        let removed = state.locals.len() - keep;
        state.locals.truncate(keep);
        state.depth -= 1;
        removed
    }

    fn match_expr(&mut self, scrutinee: &Expr, arms: &[Arm], keep: bool) -> Result<(), Error> {
        let line = self.line;
        self.expression(scrutinee)?;
        self.begin_scope();
        let before_subject = self.current().locals.len();
        let subject = self.declare_local("", false);
        let mut ends = Vec::new();

        for arm in arms {
            self.line = arm.line as u32;
            let before_arm = self.current().locals.len();
            self.begin_scope();
            let fail = match &arm.pattern {
                Pattern::Wildcard => None,
                Pattern::Bind(name) => {
                    self.emit(Code::GetLocal(subject));
                    self.bind(name);
                    None
                }
                Pattern::Literal(literal) => {
                    let value = match literal {
                        Expr::Number(n) => Value::Number(*n),
                        Expr::Str(text) => Value::str(text),
                        Expr::Bool(b) => Value::Bool(*b),
                        _ => Value::None,
                    };
                    let constant = self.constant(value);
                    let patterns = &mut self.current().proto.patterns;
                    patterns.push(PatternTest::Value {
                        slot: subject,
                        constant,
                    });
                    let index = (patterns.len() - 1) as u32;
                    Some(self.emit(Code::Test(index, 0)))
                }
                Pattern::Variant {
                    enum_name,
                    variant,
                    bindings,
                } => {
                    let patterns = &mut self.current().proto.patterns;
                    patterns.push(PatternTest::Variant {
                        slot: subject,
                        enum_name: enum_name.as_deref().map(Rc::from),
                        variant: Rc::from(variant.as_str()),
                        arity: bindings.len() as u32,
                    });
                    let index = (patterns.len() - 1) as u32;
                    let fail = self.emit(Code::Test(index, 0));
                    for binding in bindings {
                        self.bind(binding);
                    }
                    Some(fail)
                }
            };

            match &arm.body {
                ArmBody::Expr(expr) => {
                    if keep {
                        self.value(expr)?;
                    } else if let Expr::Match(inner, inner_arms) = expr {
                        self.match_expr(inner, inner_arms, false)?;
                    } else {
                        self.expression(expr)?;
                        self.emit(Code::Pop);
                    }
                }
                ArmBody::Block(block) => {
                    self.statements(block)?;
                    if keep {
                        self.emit(Code::None);
                    }
                }
            }
            self.line = arm.line as u32;
            let removed = self.leave_locals(before_arm);
            if removed > 0 {
                self.emit(if keep {
                    Code::Squash(removed as u32)
                } else {
                    Code::PopN(removed as u32)
                });
            }
            ends.push(self.emit(Code::Jump(0)));
            if let Some(fail) = fail {
                self.patch(fail);
            }
        }

        self.line = line;
        self.emit(Code::MatchFail(subject));
        for end in ends {
            self.patch(end);
        }
        self.leave_locals(before_subject);
        self.emit(if keep { Code::Squash(1) } else { Code::PopN(1) });
        Ok(())
    }

    fn begin_loop(&mut self, start: u32) {
        let state = self.current();
        let locals = state.locals.len();
        state.loops.push(Loop {
            start: start as usize,
            locals,
            breaks: Vec::new(),
        });
    }

    fn end_loop(&mut self) {
        let finished = self.current().loops.pop().expect("a loop was started");
        for jump in finished.breaks {
            self.patch(jump);
        }
    }

    fn expression(&mut self, expr: &Expr) -> Result<(), Error> {
        match expr {
            Expr::Number(n) => {
                let index = self.constant(Value::Number(*n));
                self.emit(Code::Constant(index));
            }
            Expr::Bool(true) => {
                self.emit(Code::True);
            }
            Expr::Bool(false) => {
                self.emit(Code::False);
            }
            Expr::None => {
                self.emit(Code::None);
            }
            Expr::Str(text) => {
                let index = self.name_constant(text);
                self.emit(Code::Constant(index));
            }
            Expr::Template(parts) => {
                let last_computed = parts
                    .iter()
                    .rposition(|part| !matches!(part, Expr::Str(_)))
                    .unwrap_or(0);
                for (index, part) in parts.iter().enumerate() {
                    self.expression(part)?;
                    if index < last_computed && !matches!(part, Expr::Str(_)) {
                        self.emit(Code::Stringify);
                    }
                }
                self.emit(Code::Concat(parts.len() as u32));
            }
            Expr::List(items) => {
                for item in items {
                    self.expression(item)?;
                }
                self.emit(Code::List(items.len() as u32));
            }
            Expr::Record(fields) => {
                for (_, value) in fields {
                    self.expression(value)?;
                }
                let shape: Rc<[Rc<str>]> = fields
                    .iter()
                    .map(|(name, _)| Rc::from(name.as_str()))
                    .collect();
                let shapes = &mut self.current().proto.shapes;
                shapes.push(shape);
                let index = (shapes.len() - 1) as u32;
                self.emit(Code::Record(index));
            }
            Expr::Index(target, index) => {
                self.expression(target)?;
                self.expression(index)?;
                self.emit(Code::Index);
            }
            Expr::Field(target, name) => {
                self.expression(target)?;
                let name = self.name_constant(name);
                self.emit(Code::GetField(name));
            }
            Expr::OptionalField(target, name) => {
                self.expression(target)?;
                let skip = self.emit(Code::JumpIfNoneKeep(0));
                let name = self.name_constant(name);
                self.emit(Code::GetField(name));
                self.patch(skip);
            }
            Expr::Variable(name) => self.get_variable(name)?,
            Expr::Negate(inner) => {
                self.expression(inner)?;
                self.emit(Code::Negate);
            }
            Expr::Not(inner) => {
                self.expression(inner)?;
                self.emit(Code::Not);
            }
            Expr::Binary(left, Op::And, right) => {
                self.expression(left)?;
                let skip = self.emit(Code::JumpIfFalseKeep(0, Check::And));
                self.expression(right)?;
                self.emit(Code::CheckBool(Check::And));
                self.patch(skip);
            }
            Expr::Binary(left, Op::Or, right) => {
                self.expression(left)?;
                let skip = self.emit(Code::JumpIfTrueKeep(0, Check::Or));
                self.expression(right)?;
                self.emit(Code::CheckBool(Check::Or));
                self.patch(skip);
            }
            Expr::Binary(left, Op::Coalesce, right) => {
                self.expression(left)?;
                let skip = self.emit(Code::JumpIfNotNoneKeep(0));
                self.expression(right)?;
                self.patch(skip);
            }
            Expr::Binary(left, op @ (Op::Add | Op::Sub), right)
                if matches!(**right, Expr::Number(_)) =>
            {
                let Expr::Number(n) = **right else {
                    unreachable!("checked by the guard")
                };
                self.expression(left)?;
                let index = self.constant(Value::Number(n));
                self.emit(if matches!(op, Op::Add) {
                    Code::AddConst(index)
                } else {
                    Code::SubConst(index)
                });
            }
            Expr::Binary(left, op, right) => {
                self.expression(left)?;
                self.expression(right)?;
                self.emit(match op {
                    Op::Add => Code::Add,
                    Op::Sub => Code::Sub,
                    Op::Mul => Code::Mul,
                    Op::Div => Code::Div,
                    Op::Rem => Code::Rem,
                    Op::Less => Code::Less,
                    Op::LessEqual => Code::LessEqual,
                    Op::Greater => Code::Greater,
                    Op::GreaterEqual => Code::GreaterEqual,
                    Op::Equal => Code::Equal,
                    Op::NotEqual => Code::NotEqual,
                    Op::And | Op::Or | Op::Coalesce => unreachable!("handled above"),
                });
            }
            Expr::Function {
                name,
                params,
                param_types,
                returns,
                body,
                ..
            } => self.function(name, params, param_types, returns.as_ref(), body)?,
            Expr::Match(..) => {
                return Err(Error::new(
                    self.line as usize,
                    "match can only be used as a statement, or right after let, = or return",
                ));
            }
            Expr::StructLit(name, fields) => {
                self.get_variable(name)?;
                for (_, value) in fields {
                    self.expression(value)?;
                }
                let shape: Rc<[Rc<str>]> = fields
                    .iter()
                    .map(|(field, _)| Rc::from(field.as_str()))
                    .collect();
                let shapes = &mut self.current().proto.shapes;
                shapes.push(shape);
                let index = (shapes.len() - 1) as u32;
                self.emit(Code::MakeStruct(index));
            }
            Expr::Call(callee, args) if matches!(**callee, Expr::Field(..)) => {
                let Expr::Field(target, method) = &**callee else {
                    unreachable!("checked by the guard")
                };
                self.expression(target)?;
                for arg in args {
                    self.expression(arg)?;
                }
                let name = self.name_constant(method);
                self.emit(Code::Invoke(name, args.len() as u32));
            }
            Expr::Call(callee, args) => {
                if let Expr::Variable(name) = &**callee
                    && let Var::Global(slot) = self.resolve(name)?
                {
                    for arg in args {
                        self.expression(arg)?;
                    }
                    self.emit(Code::CallGlobal(slot, args.len() as u32));
                    return Ok(());
                }
                self.expression(callee)?;
                for arg in args {
                    self.expression(arg)?;
                }
                self.emit(Code::Call(args.len() as u32));
            }
        }
        Ok(())
    }

    fn function(
        &mut self,
        name: &str,
        params: &[String],
        param_types: &[Option<Type>],
        returns: Option<&Type>,
        body: &Rc<Block>,
    ) -> Result<(), Error> {
        let line = self.line;
        let captured = captured_names(body);
        self.fns.push(FnState::new(name, params.len(), captured));
        self.current().depth = 1;
        for param in params {
            let boxed = self.current().captured.contains(param);
            let slot = self.declare_local(param, boxed);
            if boxed {
                self.emit(Code::MakeCell(slot));
            }
        }
        self.statements(body)?;
        self.emit(Code::None);
        self.emit(Code::Return);

        let mut finished = self.fns.pop().expect("the function was pushed above");
        finished.proto.source = Some(Rc::new(Source {
            name: name.to_string(),
            params: params.to_vec(),
            param_types: param_types.to_vec(),
            returns: returns.cloned(),
            looping: crate::kernel::has_loop(body),
            body: Rc::clone(body),
        }));
        self.line = line;
        let protos = &mut self.current().proto.protos;
        finished.proto.finish();
        protos.push(Rc::new(finished.proto));
        let index = (protos.len() - 1) as u32;
        self.emit(Code::Closure(index));
        Ok(())
    }
}

fn expr_of(stmt: &Stmt) -> &Expr {
    match stmt {
        Stmt::Let(_, _, expr) => expr,
        _ => unreachable!("only called for let"),
    }
}

fn captured_names(block: &Block) -> HashSet<String> {
    let mut names = HashSet::new();
    walk_block(block, false, &mut names);
    names
}

fn walk_block(block: &Block, inside: bool, names: &mut HashSet<String>) {
    for (_, stmt) in block {
        walk_stmt(stmt, inside, names);
    }
}

fn walk_stmt(stmt: &Stmt, inside: bool, names: &mut HashSet<String>) {
    match stmt {
        Stmt::Let(_, _, expr) | Stmt::Expr(expr) => walk_expr(expr, inside, names),
        Stmt::Assign(name, expr) => {
            if inside {
                names.insert(name.clone());
            }
            walk_expr(expr, inside, names);
        }
        Stmt::SetIndex(a, b, c) => {
            walk_expr(a, inside, names);
            walk_expr(b, inside, names);
            walk_expr(c, inside, names);
        }
        Stmt::SetField(a, _, b) => {
            walk_expr(a, inside, names);
            walk_expr(b, inside, names);
        }
        Stmt::If(condition, then_block, else_block) => {
            walk_expr(condition, inside, names);
            walk_block(then_block, inside, names);
            if let Some(else_block) = else_block {
                walk_block(else_block, inside, names);
            }
        }
        Stmt::While(condition, body) | Stmt::For(_, condition, body) => {
            walk_expr(condition, inside, names);
            walk_block(body, inside, names);
        }
        Stmt::Return(expr) => {
            if let Some(expr) = expr {
                walk_expr(expr, inside, names);
            }
        }
        Stmt::Struct(decl) => {
            for (_, method) in &decl.methods {
                walk_expr(method, inside, names);
            }
        }
        Stmt::Break | Stmt::Continue | Stmt::Import(..) | Stmt::Enum(_) | Stmt::Test(..) => {}
    }
}

fn walk_expr(expr: &Expr, inside: bool, names: &mut HashSet<String>) {
    match expr {
        Expr::Variable(name) => {
            if inside {
                names.insert(name.clone());
            }
        }
        Expr::Function { body, .. } => walk_block(body, true, names),
        Expr::Number(_) | Expr::Bool(_) | Expr::Str(_) | Expr::None => {}
        Expr::Template(items) | Expr::List(items) => {
            for item in items {
                walk_expr(item, inside, names);
            }
        }
        Expr::Record(fields) => {
            for (_, value) in fields {
                walk_expr(value, inside, names);
            }
        }
        Expr::Index(a, b) | Expr::Binary(a, _, b) => {
            walk_expr(a, inside, names);
            walk_expr(b, inside, names);
        }
        Expr::Field(inner, _)
        | Expr::OptionalField(inner, _)
        | Expr::Negate(inner)
        | Expr::Not(inner) => walk_expr(inner, inside, names),
        Expr::Call(callee, args) => {
            walk_expr(callee, inside, names);
            for arg in args {
                walk_expr(arg, inside, names);
            }
        }
        Expr::StructLit(name, fields) => {
            if inside {
                names.insert(name.clone());
            }
            for (_, value) in fields {
                walk_expr(value, inside, names);
            }
        }
        Expr::Match(scrutinee, arms) => {
            walk_expr(scrutinee, inside, names);
            for arm in arms {
                match &arm.body {
                    ArmBody::Expr(expr) => walk_expr(expr, inside, names),
                    ArmBody::Block(block) => walk_block(block, inside, names),
                }
            }
        }
    }
}
