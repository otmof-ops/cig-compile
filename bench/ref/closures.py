def counter(s):
    n=s
    return lambda: n+1
t=0
for i in range(100000): t+=counter(i)()
print(t)
