const N: i64 = 100000;

fn main() {
    let n = N as usize;
    let mut flags = vec![true; n + 1];
    flags[0] = false;
    flags[1] = false;
    let mut p = 2;
    while p * p <= n {
        if flags[p] {
            let mut m = p * p;
            while m <= n {
                flags[m] = false;
                m += p;
            }
        }
        p += 1;
    }
    let count = flags.iter().filter(|flag| **flag).count();
    println!("{}", count);
}
