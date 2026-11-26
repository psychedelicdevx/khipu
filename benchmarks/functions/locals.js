const N = 1000000

function work(n) {
    let a = 0
    let b = 1
    let i = 0
    while (i < n) {
        a = a + b
        b = b + 1
        i = i + 1
    }
    return a % 1000
}

console.log(work(N))
