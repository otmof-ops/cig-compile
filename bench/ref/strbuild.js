const scale = parseInt(process.argv[2] ?? "1");
const n = 10000 * scale;
let s = "";
for (let i = 0; i < n; i++) s += "x";
console.log(s.length);
