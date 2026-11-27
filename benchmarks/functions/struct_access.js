const N = 1000000

class Point {
    constructor(x, y) {
        this.x = x
        this.y = y
    }
}

function work(n) {
    const point = new Point(3, 4)
    let total = 0
    let i = 0
    while (i < n) {
        total = total + point.x * point.y
        i = i + 1
    }
    return total
}

console.log(work(N))
