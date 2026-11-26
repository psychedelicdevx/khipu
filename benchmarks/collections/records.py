N = 200000


def point(x, y):
    return {"x": x, "y": y}


total = 0
for i in range(N):
    p = point(i, i + 1)
    total = total + p["x"] + p["y"] * 2
print(total)
