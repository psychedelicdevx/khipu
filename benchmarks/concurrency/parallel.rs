const N: i64 = 100000;
const TASKS: i64 = 8;

fn crunch(seed: i64) -> i64 {
    let mut total = 0i64;
    let mut i = 0i64;
    while i < N {
        total = (total + i * seed) % 1000003;
        i += 1;
    }
    total
}

fn main() {
    let sum: i64 = std::thread::scope(|scope| {
        let handles: Vec<_> = (1..=TASKS).map(|seed| scope.spawn(move || crunch(seed))).collect();
        handles.into_iter().map(|handle| handle.join().unwrap()).sum()
    });
    println!("{}", sum);
}
