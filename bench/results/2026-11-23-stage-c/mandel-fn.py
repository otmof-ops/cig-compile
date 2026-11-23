def escape(cr, ci, limit):
    x = 0.0
    y = 0.0
    i = 0
    while i < limit:
        xx = x * x
        yy = y * y
        if xx + yy > 4.0:
            return i
        y = 2.0 * x * y + ci
        x = xx - yy + cr
        i = i + 1
    return limit


def mandel(n, limit):
    inside = 0
    for py in range(n):
        for px in range(n):
            cr = px * 3.0 / n - 2.0
            ci = py * 2.0 / n - 1.0
            if escape(cr, ci, limit) == limit:
                inside = inside + 1
    return inside


print(mandel(400, 100))
