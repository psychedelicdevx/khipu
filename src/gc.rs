use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use crate::value::{Closure, EnumValue, Globals, Instance, List, Record, StructDef, Value};

pub enum Tracked {
    List(Weak<RefCell<List>>),
    Record(Weak<RefCell<Record>>),
    Instance(Weak<Instance>),
    Closure(Weak<Closure>),
    Enum(Weak<EnumValue>),
    Struct(Weak<StructDef>),
    Cell(Weak<RefCell<Value>>),
    Globals(Weak<RefCell<Globals>>),
}

enum Live {
    List(Rc<RefCell<List>>),
    Record(Rc<RefCell<Record>>),
    Instance(Rc<Instance>),
    Closure(Rc<Closure>),
    Enum(Rc<EnumValue>),
    Struct(Rc<StructDef>),
    Cell(Rc<RefCell<Value>>),
    Globals(Rc<RefCell<Globals>>),
}

const MIN_THRESHOLD: usize = 32_768;

#[derive(Default)]
struct Heap {
    objects: Vec<Tracked>,
    since: usize,
    threshold: usize,
}

thread_local! {
    static HEAP: RefCell<Heap> = const {
        RefCell::new(Heap {
            objects: Vec::new(),
            since: 0,
            threshold: MIN_THRESHOLD,
        })
    };
}

pub fn track(object: Tracked) {
    let due = HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        heap.objects.push(object);
        heap.since += 1;
        heap.since >= heap.threshold
    });
    if due {
        collect();
    }
}

pub fn tracked() -> usize {
    HEAP.with(|heap| heap.borrow().objects.len())
}

fn address<T: ?Sized>(rc: &Rc<T>) -> usize {
    Rc::as_ptr(rc) as *const u8 as usize
}

impl Live {
    fn address(&self) -> usize {
        match self {
            Live::List(rc) => address(rc),
            Live::Record(rc) => address(rc),
            Live::Instance(rc) => address(rc),
            Live::Closure(rc) => address(rc),
            Live::Enum(rc) => address(rc),
            Live::Struct(rc) => address(rc),
            Live::Cell(rc) => address(rc),
            Live::Globals(rc) => address(rc),
        }
    }

    fn strong_count(&self) -> usize {
        match self {
            Live::List(rc) => Rc::strong_count(rc),
            Live::Record(rc) => Rc::strong_count(rc),
            Live::Instance(rc) => Rc::strong_count(rc),
            Live::Closure(rc) => Rc::strong_count(rc),
            Live::Enum(rc) => Rc::strong_count(rc),
            Live::Struct(rc) => Rc::strong_count(rc),
            Live::Cell(rc) => Rc::strong_count(rc),
            Live::Globals(rc) => Rc::strong_count(rc),
        }
    }

    fn weak(&self) -> Tracked {
        match self {
            Live::List(rc) => Tracked::List(Rc::downgrade(rc)),
            Live::Record(rc) => Tracked::Record(Rc::downgrade(rc)),
            Live::Instance(rc) => Tracked::Instance(Rc::downgrade(rc)),
            Live::Closure(rc) => Tracked::Closure(Rc::downgrade(rc)),
            Live::Enum(rc) => Tracked::Enum(Rc::downgrade(rc)),
            Live::Struct(rc) => Tracked::Struct(Rc::downgrade(rc)),
            Live::Cell(rc) => Tracked::Cell(Rc::downgrade(rc)),
            Live::Globals(rc) => Tracked::Globals(Rc::downgrade(rc)),
        }
    }

    fn children(&self, visit: &mut impl FnMut(usize)) {
        match self {
            Live::List(list) => {
                if let Ok(list) = list.try_borrow() {
                    list.visit(&mut |value| visit_value(value, visit));
                }
            }
            Live::Record(record) => {
                if let Ok(record) = record.try_borrow() {
                    visit_values(record.values.iter(), visit);
                }
            }
            Live::Instance(instance) => {
                visit(address(&instance.def));
                if let Ok(fields) = instance.fields.try_borrow() {
                    visit_values(fields.iter(), visit);
                }
            }
            Live::Closure(closure) => {
                for cell in &closure.captures {
                    visit(address(cell));
                }
                visit(address(&closure.globals));
            }
            Live::Enum(variant) => visit_values(variant.values.iter(), visit),
            Live::Struct(def) => visit_values(def.methods.iter().map(|(_, value)| value), visit),
            Live::Cell(cell) => {
                if let Ok(inner) = cell.try_borrow() {
                    visit_value(&inner, visit);
                }
            }
            Live::Globals(globals) => {
                if let Ok(globals) = globals.try_borrow() {
                    visit_values(globals.values.iter().flatten(), visit);
                }
            }
        }
    }

    fn clear(&self) {
        match self {
            Live::List(list) => {
                if let Ok(mut list) = list.try_borrow_mut() {
                    *list = List::default();
                }
            }
            Live::Record(record) => {
                if let Ok(mut record) = record.try_borrow_mut() {
                    record.values.clear();
                }
            }
            Live::Instance(instance) => {
                if let Ok(mut fields) = instance.fields.try_borrow_mut() {
                    fields.clear();
                }
            }
            Live::Cell(cell) => {
                if let Ok(mut inner) = cell.try_borrow_mut() {
                    *inner = Value::None;
                }
            }
            Live::Globals(globals) => {
                if let Ok(mut globals) = globals.try_borrow_mut() {
                    globals.values.clear();
                }
            }
            Live::Closure(_) | Live::Enum(_) | Live::Struct(_) => {}
        }
    }
}

fn visit_value(value: &Value, visit: &mut impl FnMut(usize)) {
    if let Some(address) = heap_address(value) {
        visit(address);
    }
}

fn visit_values<'a>(values: impl Iterator<Item = &'a Value>, visit: &mut impl FnMut(usize)) {
    for value in values {
        visit_value(value, visit);
    }
}

fn heap_address(value: &Value) -> Option<usize> {
    Some(match value {
        Value::List(rc) => address(rc),
        Value::Record(rc) => address(rc),
        Value::Function(rc) => address(rc),
        Value::Instance(rc) => address(rc),
        Value::Enum(rc) => address(rc),
        Value::StructDef(rc) => address(rc),
        Value::Cell(rc) => address(rc),
        Value::Number(_)
        | Value::Bool(_)
        | Value::None
        | Value::Str(_)
        | Value::Native(_)
        | Value::EnumDef(_)
        | Value::Channel(_) => return None,
    })
}

fn upgrade(tracked: &Tracked) -> Option<Live> {
    Some(match tracked {
        Tracked::List(weak) => Live::List(weak.upgrade()?),
        Tracked::Record(weak) => Live::Record(weak.upgrade()?),
        Tracked::Instance(weak) => Live::Instance(weak.upgrade()?),
        Tracked::Closure(weak) => Live::Closure(weak.upgrade()?),
        Tracked::Enum(weak) => Live::Enum(weak.upgrade()?),
        Tracked::Struct(weak) => Live::Struct(weak.upgrade()?),
        Tracked::Cell(weak) => Live::Cell(weak.upgrade()?),
        Tracked::Globals(weak) => Live::Globals(weak.upgrade()?),
    })
}

pub fn collect() {
    let objects = HEAP.with(|heap| std::mem::take(&mut heap.borrow_mut().objects));

    let live: Vec<Live> = objects.iter().filter_map(upgrade).collect();
    drop(objects);

    let index: HashMap<usize, usize> = live
        .iter()
        .enumerate()
        .map(|(at, object)| (object.address(), at))
        .collect();

    let mut internal = vec![0usize; live.len()];
    for object in &live {
        object.children(&mut |child| {
            if let Some(at) = index.get(&child) {
                internal[*at] += 1;
            }
        });
    }

    let mut marked = vec![false; live.len()];
    let mut pending: Vec<usize> = (0..live.len())
        .filter(|&at| live[at].strong_count() - 1 > internal[at])
        .collect();
    let mut work = 0;
    while let Some(at) = pending.pop() {
        if std::mem::replace(&mut marked[at], true) {
            continue;
        }
        work += 1;
        live[at].children(&mut |child| {
            work += 1;
            if let Some(&next) = index.get(&child)
                && !marked[next]
            {
                pending.push(next);
            }
        });
    }

    let mut survivors = Vec::new();
    for (at, object) in live.iter().enumerate() {
        if marked[at] {
            survivors.push(object.weak());
        } else {
            object.clear();
        }
    }
    drop(live);

    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        survivors.append(&mut heap.objects);
        heap.objects = survivors;
        heap.since = 0;
        heap.threshold = work.max(MIN_THRESHOLD);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::Interpreter;
    use crate::lexer::lex;
    use crate::parser::Parser;
    use crate::value::Value;

    fn run(source: &str) {
        let program = Parser::new(lex(source).unwrap()).parse_program().unwrap();
        Interpreter::new().run_main(&program).unwrap();
    }

    #[test]
    fn frees_cycles_a_program_leaves_behind() {
        run(
            "let i = 0\nwhile i < 1000 {\n    let r = { me: none, pad: [1, 2, 3] }\n    r.me = r\n    i = i + 1\n}",
        );
        collect();
        let records = tracked();
        assert!(
            records < 400,
            "{records} objects survived, the cycles leaked"
        );

        run(
            "fn outer() {\n    fn inner(n: number) -> number {\n        if n == 0 {\n            return 0\n        }\n        return inner(n - 1)\n    }\n    return inner(2)\n}\nlet j = 0\nwhile j < 1000 {\n    outer()\n    j = j + 1\n}",
        );
        collect();
        let closures = tracked();
        assert!(
            closures < 400,
            "{closures} objects survived, the closures leaked"
        );
    }

    #[test]
    fn frees_a_record_that_holds_itself() {
        let record = Value::record(vec![(Rc::from("me"), Value::None)]);
        let Value::Record(fields) = &record else {
            unreachable!()
        };
        fields.borrow_mut().values[0] = record.clone();
        let weak = Rc::downgrade(fields);
        drop(record);
        assert!(weak.upgrade().is_some(), "the cycle keeps it alive");
        collect();
        assert!(weak.upgrade().is_none(), "the collector broke the cycle");
    }

    #[test]
    fn keeps_what_is_still_in_use() {
        let list = Value::list(vec![Value::Number(1.0)]);
        let record = Value::record(vec![(Rc::from("items"), list.clone())]);
        let Value::List(items) = &list else {
            unreachable!()
        };
        items.borrow_mut().push(record.clone());
        drop(record);
        collect();
        let Value::List(items) = &list else {
            unreachable!()
        };
        assert_eq!(items.borrow().len(), 2, "a reachable cycle is left alone");
    }
}
