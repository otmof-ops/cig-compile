w=h=160; inside=0
for y in range(h):
    for x in range(w):
        cr=2.0*x/w-1.5; ci=2.0*y/h-1.0; zr=zi=0.0; k=0; esc=False
        while k<50 and not esc:
            t=zr*zr-zi*zi+cr; zi=2.0*zr*zi+ci; zr=t
            if zr*zr+zi*zi>4.0: esc=True
            k+=1
        if not esc: inside+=1
print(inside)
