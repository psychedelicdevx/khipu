const N: i64 = 3000000;

fn main() {
    let mut total = 0i64;
    let mut i = 0i64;
    while i < N {
        total += i;
        i += 1;
    }
    println!("{}", total);
}
