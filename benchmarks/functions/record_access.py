N = 1000000
point = {"x": 3, "y": 4}
total = 0
i = 0
while i < N:
    total = total + point["x"] * point["y"]
    i = i + 1
print(total)
