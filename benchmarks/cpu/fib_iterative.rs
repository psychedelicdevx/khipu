const N: i64 = 100000;

fn fib(n: i64) -> i64 {
    let (mut a, mut b) = (0i64, 1i64);
    for _ in 0..n {
        let next = a + b;
        a = b;
        b = next;
    }
    a
}

fn main() {
    let mut total = 0i64;
    for i in 0..N {
        total = (total + fib(i % 50)) % 1000000007;
    }
    println!("{}", total);
}
