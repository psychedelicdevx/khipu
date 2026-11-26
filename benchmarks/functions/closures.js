const N = 500000

function counter() {
    let count = 0
    return function () {
        count = count + 1
        return count
    }
}

const next = counter()
let last = 0
for (let i = 0; i < N; i++) {
    last = next()
}
console.log(last)
