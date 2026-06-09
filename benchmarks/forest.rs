use std::time::Instant;

struct Arena {
    data: Vec<Node>,
}

impl Arena {
    fn new(capacity: usize) -> Self {
        Arena {
            data: Vec::with_capacity(capacity),
        }
    }
    fn alloc(&mut self, node: Node) -> u32 {
        let idx = self.data.len() as u32;
        self.data.push(node);
        idx
    }
    fn reset(&mut self) {
        self.data.clear();
    }
}

struct Node {
    left: usize,
    right: usize,
    val: i32,
}

const NULL: usize = usize::MAX;

fn make_tree(arena: &mut Arena, depth: i32) -> usize {
    if depth == 0 {
        return NULL;
    }
    let left = make_tree(arena, depth - 1);
    let right = make_tree(arena, depth - 1);
    arena.alloc(Node { left, right, val: depth }) as usize
}

fn main() {
    let t0 = Instant::now();
    
    // Depth 22 -> ~4M nodes (2^22 - 1)
    let mut arena = Arena::new(4_200_000);
    let _root = make_tree(&mut arena, 22);
    
    let t1 = Instant::now();
    let build_time = t1.duration_since(t0).as_nanos();
    println!("Build Time: {} ns", build_time);
    
    let t2 = Instant::now();
    
    // Drop triggers recursive free (in this case just clear)
    arena.reset();
    
    let t3 = Instant::now();
    let free_time = t3.duration_since(t2).as_nanos();
    let total = t3.duration_since(t0).as_nanos();
    
    println!("Free Time: {} ns", free_time);
    println!("Total Churn: {} ns", total);
}
