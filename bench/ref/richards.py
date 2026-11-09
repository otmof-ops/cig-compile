import sys
scale = int(sys.argv[1]) if len(sys.argv) > 1 else 1
tasks = [{"pri": (i + 3) % 5, "work": 0, "qlen": 0} for i in range(5)]
ticks = 10000 * scale
for t in range(ticks):
    best = 0
    for i in range(1, 5):
        if tasks[i]["pri"] > tasks[best]["pri"]:
            best = i
    tasks[best]["work"] += 1
    tasks[best]["qlen"] += 1
    next_i = (best + 1) % 5
    if tasks[best]["qlen"] > 3:
        tasks[best]["qlen"] -= 1
        tasks[next_i]["pri"] = (tasks[next_i]["pri"] + 1) % 5
total = sum(t["work"] for t in tasks)
pri_sum = sum(t["pri"] for t in tasks)
print(total, pri_sum)
