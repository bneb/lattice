// string_hashmap_bench.rs - String Key HashMap Benchmark
// Uses std::collections::HashMap with String keys for comparison

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

pub struct FnvHasher(u64);

impl Default for FnvHasher {
    fn default() -> Self {
        FnvHasher(14695981039346656037)
    }
}

impl Hasher for FnvHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 ^= byte as u64;
            self.0 = self.0.wrapping_mul(1099511628211);
        }
    }
}

type FnvHashMap<K, V> = HashMap<K, V, BuildHasherDefault<FnvHasher>>;

fn main() {
    let mut checksum: i64 = 0;

    for _ in 0..100 {
        let mut map: FnvHashMap<String, i64> = FnvHashMap::default();

        // Insert 1000 string keys
        for i in 0..1000i64 {
            let key = format!("key_{}", i);
            map.insert(key, i * 7);
        }

        // Lookup all keys
        for i in 0..1000i64 {
            let key = format!("key_{}", i);
            checksum += map.get(&key).copied().unwrap_or(0);
        }

        // Remove half
        for i in 0..500i64 {
            let key = format!("key_{}", i * 2);
            map.remove(&key);
        }

        // Re-insert
        for i in 0..500i64 {
            let key = format!("key_{}", i * 2);
            map.insert(key, i * 11);
        }

        // Final lookups
        for i in 0..1000i64 {
            let key = format!("key_{}", i);
            checksum += map.get(&key).copied().unwrap_or(0);
        }
    }

    println!("Checksum: {}", checksum);
}
