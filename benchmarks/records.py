def point(x, y):
    return {"x": x, "y": y}
total = 0
for i in range(200000):
    p = point(i, i + 1)
    total = total + p["x"] * p["y"]
print(total)
