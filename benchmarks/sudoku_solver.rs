
const N: usize = 9;

fn is_valid(b: &[u8; 81], r: usize, c: usize, n: u8) -> bool {
    for i in 0..9 {
        if b[r * 9 + i] == n || b[i * 9 + c] == n { return false; }
    }
    let sr = (r / 3) * 3;
    let sc = (c / 3) * 3;
    for i in 0..3 {
        for j in 0..3 {
            if b[(sr + i) * 9 + (sc + j)] == n { return false; }
        }
    }
    true
}

fn solve(b: &mut [u8; 81]) -> bool {
    for i in 0..81 {
        if b[i] == 0 {
            let r = i / 9;
            let c = i % 9;
            for n in 1..=9 {
                if is_valid(b, r, c, n) {
                    b[i] = n;
                    if solve(b) { return true; }
                    b[i] = 0;
                }
            }
            return false;
        }
    }
    true
}

fn main() {
    let mut total_solved = 0;
    
    for _ in 0..600 {
        let mut board: [u8; 81] = [
            3, 0, 6, 5, 0, 8, 4, 0, 0,
            5, 2, 0, 0, 0, 0, 0, 0, 0,
            0, 8, 7, 0, 0, 0, 0, 3, 1,
            0, 0, 3, 0, 1, 0, 0, 8, 0,
            9, 0, 0, 8, 6, 3, 0, 0, 5,
            0, 5, 0, 0, 9, 0, 6, 0, 0,
            1, 3, 0, 0, 0, 0, 2, 5, 0,
            0, 0, 0, 0, 0, 0, 0, 7, 4,
            0, 0, 5, 2, 0, 6, 3, 0, 0
        ];
        
        if solve(&mut board) {
            total_solved += 1;
        }
    }
    
    // std::hint::black_box(total_solved);
}
