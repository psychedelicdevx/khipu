const N = 1000000

class Point {
    constructor(x, y) {
        this.x = x
        this.y = y
    }
}

const point = new Point(3, 4)
let total = 0
let i = 0
while (i < N) {
    total = total + point.x * point.y
    i = i + 1
}
console.log(total)
