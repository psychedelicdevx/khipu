const N: i64 = 200000;

fn main() {
    let mut count = 0usize;
    for i in 0..N {
        let line = format!("item {i}: ") + "ok";
        count += line.len();
    }
    println!("{}", count);
}
