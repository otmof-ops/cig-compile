import sys
scale = int(sys.argv[1]) if len(sys.argv) > 1 else 1
n = 20 * scale
iters = 500
MOD = 1000003
vars_ = [0] * n
changes = 0
for k in range(iters):
    vars_[0] = k
    for i in range(1, n):
        prev = vars_[i-1]
        want = (prev * 2 + i * 7 + 1) % MOD
        if vars_[i] != want:
            vars_[i] = want
            changes += 1
total = 0
for v in vars_:
    total = (total + v) % MOD
print(total, changes)
