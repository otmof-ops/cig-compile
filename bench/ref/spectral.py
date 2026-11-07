import math
def a(i,j): return 1.0/((i+j)*(i+j+1)/2+i+1)
def mulav(v,n): return [sum(a(i,j)*v[j] for j in range(n)) for i in range(n)]
def mulatv(v,n): return [sum(a(j,i)*v[j] for j in range(n)) for i in range(n)]
n=100; u=[1.0]*n
for _ in range(10): v=mulatv(mulav(u,n),n); u=mulatv(mulav(v,n),n)
print(round(math.sqrt(sum(u[i]*v[i] for i in range(n))/sum(x*x for x in v)),9))
