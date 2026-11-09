const scale = parseInt(process.argv[2] ?? "1");
const n = 20 * scale;
const iters = 500;
const MOD = 1000003;
const vars_ = new Array(n).fill(0);
let changes = 0;
for (let k = 0; k < iters; k++) {
  vars_[0] = k;
  for (let i = 1; i < n; i++) {
    const prev = vars_[i-1];
    const want = (prev * 2 + i * 7 + 1) % MOD;
    if (vars_[i] !== want) { vars_[i] = want; changes += 1; }
  }
}
let total = 0;
for (const v of vars_) total = (total + v) % MOD;
console.log(total, changes);
