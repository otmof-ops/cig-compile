def make(d): return [None,None] if d==0 else [make(d-1),make(d-1)]
def check(t): return 1 if t[0] is None else 1+check(t[0])+check(t[1])
M=12; print("stretch",check(make(M+1))); L=make(M); d=4
while d<=M:
    n=2**(M-d+4); c=sum(check(make(d)) for _ in range(n)); print(n,d,c); d+=2
print("long",check(L))
