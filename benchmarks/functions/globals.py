N = 1000000
counter = 0


def bump():
    global counter
    counter = counter + 1


i = 0
while i < N:
    bump()
    i = i + 1
print(counter)
