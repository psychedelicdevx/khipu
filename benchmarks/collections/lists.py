N = 300000
items = []
for i in range(N):
    items.append(i * 2)
total = 0
for item in items:
    total = total + item
print(total + items[len(items) - 1])
