const N = 100000
const flags = new Array(N + 1).fill(true)
flags[0] = false
flags[1] = false
for (let p = 2; p * p <= N; p++) {
    if (flags[p]) {
        for (let m = p * p; m <= N; m += p) {
            flags[m] = false
        }
    }
}
let count = 0
for (const flag of flags) {
    if (flag) count = count + 1
}
console.log(count)
