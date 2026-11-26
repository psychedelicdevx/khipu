N = 60
inside = 0
for py in range(N):
    for px in range(N):
        cx = px * 3.0 / N - 2.0
        cy = py * 2.0 / N - 1.0
        x = 0.0
        y = 0.0
        i = 0
        while i < 50 and x * x + y * y <= 4:
            t = x * x - y * y + cx
            y = 2 * x * y + cy
            x = t
            i = i + 1
        if i == 50:
            inside = inside + 1
print(inside)
