text=" ".join(f"w{(i*7919)%997}" for i in range(40000)); c={}
for w in text.split(" "): c[w]=c.get(w,0)+1
best=min(c, key=lambda w:(-c[w],w)); print(len(c),best,c[best])
