// Writer Protocol Performance Benchmark - Rust
// Matches Salt's memcpy and direct pointer write implementation

const ITERATIONS: usize = 10_000_000;
const BUFFER_SIZE: usize = 4096;

#[inline(always)]
fn write_i32_to_ptr(base: &mut [u8], mut offset: usize, val: i32) -> usize {
    if val == 0 {
        base[offset] = b'0';
        return offset + 1;
    }
    
    let mut tmp = [0u8; 16];
    let mut len = 0;
    let mut n = val;
    let neg = n < 0;
    if neg { n = -n; }
    
    while n > 0 {
        tmp[len] = b'0' + (n % 10) as u8;
        len += 1;
        n /= 10;
    }
    
    if neg {
        base[offset] = b'-';
        offset += 1;
    }
    
    for i in (0..len).rev() {
        base[offset] = tmp[i];
        offset += 1;
    }
    offset
}

#[inline(always)]
fn write_i64_to_ptr(base: &mut [u8], mut offset: usize, val: i64) -> usize {
    if val == 0 {
        base[offset] = b'0';
        return offset + 1;
    }
    
    let mut tmp = [0u8; 24];
    let mut len = 0;
    let mut n = val;
    let neg = n < 0;
    if neg { n = -n; }
    
    while n > 0 {
        tmp[len] = b'0' + (n % 10) as u8;
        len += 1;
        n /= 10;
    }
    
    if neg {
        base[offset] = b'-';
        offset += 1;
    }
    
    for i in (0..len).rev() {
        base[offset] = tmp[i];
        offset += 1;
    }
    offset
}

fn main() {
    let mut buf = vec![0u8; BUFFER_SIZE];
    let mut total_len: i64 = 0;
    let mut checksum: u8 = 0;
    
    for i in 0..ITERATIONS {
        let mut offset = 0;
        
        buf[offset..offset+5].copy_from_slice(b"Item ");
        offset += 5;
        
        offset = write_i32_to_ptr(&mut buf, offset, i as i32);
        
        buf[offset..offset+8].copy_from_slice(b": val = ");
        offset += 8;
        
        offset = write_i64_to_ptr(&mut buf, offset, (i as i64) * 1000);
        
        total_len += offset as i64;
        
        checksum ^= buf[0];
        checksum ^= buf[offset - 1];
    }
    
    if total_len == 0 || checksum == 255 {
        std::process::exit(1);
    }
}
