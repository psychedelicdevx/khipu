const N = 1000000
let counter = 0

function bump() {
    counter = counter + 1
}

let i = 0
while (i < N) {
    bump()
    i = i + 1
}
console.log(counter)
