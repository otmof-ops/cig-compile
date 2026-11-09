import sys, json
scale = int(sys.argv[1]) if len(sys.argv) > 1 else 1
def mkdoc(d, n):
    if d == 0: return {"leaf": n}
    return {"level": d, "kids": [mkdoc(d-1, n) for _ in range(n)]}
def walk(x):
    if "leaf" in x: return x["leaf"]
    return x["level"] + sum(walk(k) for k in x["kids"])
doc = mkdoc(6, 3)
body = json.dumps(doc, separators=(",", ":"))
total = 0
for _ in range(scale):
    back = json.loads(body)
    total += walk(back)
print(total, len(body))
