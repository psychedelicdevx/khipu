items = []
for i in range(300000):
    items.append(i * 2)
total = 0
for item in items:
    total = total + item
print(total + items[len(items) - 1])
