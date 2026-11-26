const N = 1000000
const point = { x: 3, y: 4 }
let total = 0
let i = 0
while (i < N) {
    total = total + point.x * point.y
    i = i + 1
}
console.log(total)
