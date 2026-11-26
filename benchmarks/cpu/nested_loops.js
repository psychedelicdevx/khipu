const N = 300
let total = 0
for (let i = 0; i < N; i++) {
    for (let j = 0; j < N; j++) {
        total = total + (i * j) % 7
    }
}
console.log(total)
