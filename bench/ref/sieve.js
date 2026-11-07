const n=300000,s=Array(n+1).fill(true);s[0]=s[1]=false;for(let p=2;p*p<=n;p++)if(s[p])for(let m=p*p;m<=n;m+=p)s[m]=false;let c=0;for(const b of s)if(b)c++;console.log(c);
