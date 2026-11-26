const N: i64 = 300;

fn main() {
    let mut total = 0i64;
    for i in 0..N {
        for j in 0..N {
            total += (i * j) % 7;
        }
    }
    println!("{}", total);
}
