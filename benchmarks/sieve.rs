struct Arena {
    data: Vec<u8>,
}

impl Arena {
    fn new(capacity: usize) -> Self {
        Arena { data: vec![0; capacity] }
    }
}

fn sieve(arena: &mut Arena, limit: i32) -> i32 {
    let is_prime = &mut arena.data[..(limit + 1) as usize];
    is_prime.fill(1);
    is_prime[0] = 0;
    is_prime[1] = 0;

    let mut p: i32 = 2;
    while p * p <= limit {
        if is_prime[p as usize] != 0 {
            let mut i = p * p;
            while i <= limit {
                is_prime[i as usize] = 0;
                i += p;
            }
        }
        p += 1;
    }

    let mut count: i32 = 0;
    for i in 0..=limit {
        if is_prime[i as usize] != 0 {
            count += 1;
        }
    }
    count
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let limit = 1000000;
    let mut arena = Arena::new((limit + 1) as usize);
    let mut total_primes = 0;
    for _ in 0..200 {
        total_primes += sieve(&mut arena, limit);
    }
    if total_primes == 78498 * 200 {
        std::process::exit(0);
    }
    std::process::exit(1);
}
