fn main(){let n:i64=std::env::args().nth(1).map(|s|s.parse().unwrap()).unwrap_or(400000);let mut t:i64=0;for i in 0..n{t+=i*i%7}println!("{}",t)}
