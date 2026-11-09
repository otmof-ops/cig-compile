import sys
scale = int(sys.argv[1]) if len(sys.argv) > 1 else 1
n = 10000 * scale
s = ""
for i in range(n):
    s += "x"
print(len(s))
