const counter=s=>{let n=s;return()=>n+1};let t=0;for(let i=0;i<100000;i++)t+=counter(i)();console.log(t);
