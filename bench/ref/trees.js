const make=d=>d==0?[null,null]:[make(d-1),make(d-1)];const check=t=>t[0]===null?1:1+check(t[0])+check(t[1]);
const M=12;console.log("stretch",check(make(M+1)));const L=make(M);for(let d=4;d<=M;d+=2){const n=2**(M-d+4);let c=0;for(let i=0;i<n;i++)c+=check(make(d));console.log(n,d,c)}console.log("long",check(L));
