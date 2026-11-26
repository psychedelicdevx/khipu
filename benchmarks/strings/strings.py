N = 200000
count = 0
for i in range(N):
    line = f"item {i}: " + "ok"
    count = count + len(line)
print(count)
