const N: i64 = 300000;

fn main() {
    let mut items = Vec::new();
    for i in 0..N {
        items.push(i * 2);
    }
    let mut total = 0i64;
    for item in &items {
        total += item;
    }
    println!("{}", total + items[items.len() - 1]);
}
