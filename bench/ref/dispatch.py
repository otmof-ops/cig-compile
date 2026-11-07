ops={"add":lambda a,b:a+b,"sub":lambda a,b:a-b,"mul":lambda a,b:a*b%1000003}; names=["add","sub","mul"]; acc=1
for i in range(150000): acc=ops[names[i%3]](acc,i)%1000003
print(acc)
