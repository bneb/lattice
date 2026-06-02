use std::time::Instant;
fn main() {
    let start = Instant::now();
    let mut sum: i64 = 0;
    for i in 0..1000000 { sum += i; }
    println!("Chase-lev Rust: {:?}", start.elapsed());
}
