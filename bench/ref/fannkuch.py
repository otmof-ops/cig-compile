n=8; p1=list(range(n)); cnt=[0]*n; mf=0; cs=0; pc=0; r=n
while True:
    while r!=1: cnt[r-1]=r; r-=1
    p=p1[:]; f=0; k=p[0]
    while k: p[:k+1]=p[k::-1]; f+=1; k=p[0]
    mf=max(mf,f); cs+= f if pc%2==0 else -f
    while True:
        if r==n: print(cs,mf); raise SystemExit
        p0=p1[0]; p1[:r]=p1[1:r+1]; p1[r]=p0; cnt[r]-=1
        if cnt[r]>0: break
        r+=1
    pc+=1
