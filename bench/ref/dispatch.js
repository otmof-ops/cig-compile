const ops={add:(a,b)=>a+b,sub:(a,b)=>a-b,mul:(a,b)=>a*b%1000003},names=["add","sub","mul"];let acc=1;for(let i=0;i<150000;i++)acc=((ops[names[i%3]](acc,i)%1000003)+1000003)%1000003;console.log(acc);
