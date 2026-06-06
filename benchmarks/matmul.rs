fn matmul(n: usize, a: &[f64], b: &[f64], c: &mut [f64]) {
    for i in 0..n {
        for k in 0..n {
            for j in 0..n {
                c[i * n + j] += a[i * n + k] * b[k * n + j];
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let argc = args.len() as f64;

    let n = 1024;
    let a = vec![1.0 * argc; n * n];
    let b = vec![2.0; n * n];
    let mut c = vec![0.0; n * n];

    matmul(n, &a, &b, &mut c);
    
    let checksum: f64 = c.iter().sum();
    if checksum != (2147483648.0 * argc) {
        std::process::exit(1);
    }
}
