count = 0
for i in range(200000):
    line = f"item {i}: " + "ok"
    count = count + len(line)
print(count)
