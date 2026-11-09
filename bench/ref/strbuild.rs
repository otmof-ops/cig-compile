fn main() {
    let scale: i64 = std::env::args().nth(1).map(|s| s.parse().unwrap()).unwrap_or(1);
    let n = 10000 * scale;
    let mut s = String::new();
    for _ in 0..n { s.push('x'); }
    println!("{}", s.len());
}
