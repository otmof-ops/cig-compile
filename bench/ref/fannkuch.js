const n=8;const p1=[...Array(n).keys()],cnt=Array(n).fill(0);let mf=0,cs=0,pc=0,r=n;
for(;;){while(r!=1){cnt[r-1]=r;r--}const p=p1.slice();let f=0,k=p[0];while(k){let i=0,j=k;while(i<j){const t=p[i];p[i]=p[j];p[j]=t;i++;j--}f++;k=p[0]}if(f>mf)mf=f;cs+=pc%2==0?f:-f;
for(;;){if(r==n){console.log(cs,mf);process.exit(0)}const p0=p1[0];for(let i=0;i<r;i++)p1[i]=p1[i+1];p1[r]=p0;cnt[r]--;if(cnt[r]>0)break;r++}pc++}
