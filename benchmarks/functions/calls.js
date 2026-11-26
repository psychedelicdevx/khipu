const N = 300000

function identity(x) {
    return x
}

let total = 0
for (let i = 0; i < N; i++) {
    total = total + identity(i)
}
console.log(total)
