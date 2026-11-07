n=300000; s=[True]*(n+1); s[0]=s[1]=False; p=2
while p*p<=n:
    if s[p]:
        m=p*p
        while m<=n: s[m]=False; m+=p
    p+=1
print(sum(s))
