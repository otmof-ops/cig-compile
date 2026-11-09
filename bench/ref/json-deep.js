const scale = parseInt(process.argv[2] ?? "1");
const mkdoc = (d, n) => d == 0 ? {leaf: n} : {level: d, kids: Array.from({length:n}, () => mkdoc(d-1, n))};
const walk = (x) => "leaf" in x ? x.leaf : x.level + x.kids.reduce((a, k) => a + walk(k), 0);
const doc = mkdoc(6, 3);
const body = JSON.stringify(doc);
let total = 0;
for (let i = 0; i < scale; i++) { total += walk(JSON.parse(body)); }
console.log(total, body.length);
