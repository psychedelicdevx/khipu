const N = 300000

const items = [3, 4, 5, 6, 7, 8, 9, 10]

function total(values, rounds) {
    let acc = 0
    let at = 0
    const n = values.length
    while (at < rounds) {
        acc = acc + values[at % n]
        at = at + 1
    }
    values[0] = values[0] + 1
    return acc + values[values.length - 1] + values[0]
}

console.log(total(items, N))
