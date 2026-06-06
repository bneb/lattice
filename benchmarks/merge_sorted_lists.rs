
struct Arena {
    nodes: Vec<ListNode>,
}

impl Arena {
    fn new(capacity: usize) -> Self {
        Arena { nodes: Vec::with_capacity(capacity) }
    }
    fn alloc(&mut self, node: ListNode) -> u32 {
        let idx = self.nodes.len() as u32;
        self.nodes.push(node);
        idx
    }
    fn reset(&mut self) {
        self.nodes.clear();
    }
}

struct ListNode {
    val: i32,
    next: u32,
}

const NULL: u32 = u32::MAX;

fn create_list(arena: &mut Arena, len: i32, start: i32, step: i32) -> u32 {
    if len <= 0 {
        return NULL;
    }
    let next = create_list(arena, len - 1, start + step, step);
    arena.alloc(ListNode { val: start, next })
}

fn merge_two_lists(arena: &mut Arena, l1: u32, l2: u32) -> u32 {
    if l1 == NULL {
        return l2;
    }
    if l2 == NULL {
        return l1;
    }
    
    let v1 = arena.nodes[l1 as usize].val;
    let v2 = arena.nodes[l2 as usize].val;
    
    if v1 < v2 {
        let next = arena.nodes[l1 as usize].next;
        let merged_next = merge_two_lists(arena, next, l2);
        arena.nodes[l1 as usize].next = merged_next;
        l1
    } else {
        let next = arena.nodes[l2 as usize].next;
        let merged_next = merge_two_lists(arena, l1, next);
        arena.nodes[l2 as usize].next = merged_next;
        l2
    }
}

fn main() {
    let mut checksum = 0;
    let mut arena = Arena::new(500);
    for _ in 0..5000 {
        let l1 = create_list(&mut arena, 100, 0, 2);
        let l2 = create_list(&mut arena, 100, 1, 2);
        let merged = merge_two_lists(&mut arena, l1, l2);
        
        let mut curr = merged;
        while curr != NULL {
            checksum += arena.nodes[curr as usize].val;
            curr = arena.nodes[curr as usize].next;
        }
        arena.reset();
    }
    println!("Checksum: {}", checksum);
}
