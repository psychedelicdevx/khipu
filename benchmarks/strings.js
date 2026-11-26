let count = 0
for (let i = 0; i < 200000; i++) {
    const line = `item ${i}: ` + "ok"
    count = count + line.length
}
console.log(count)
