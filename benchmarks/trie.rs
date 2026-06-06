#[derive(Default)]
struct TrieNode {
    children: [u32; 26],
    is_word: bool,
}

impl TrieNode {
    fn new() -> Self {
        TrieNode {
            children: [u32::MAX; 26],
            is_word: false,
        }
    }
}

struct Arena {
    nodes: Vec<TrieNode>,
}

impl Arena {
    fn new(capacity: usize) -> Self {
        let mut arena = Arena { nodes: Vec::with_capacity(capacity) };
        arena.alloc(TrieNode::new()); // root at index 0
        arena
    }
    fn alloc(&mut self, node: TrieNode) -> u32 {
        let idx = self.nodes.len() as u32;
        self.nodes.push(node);
        idx
    }
}

fn insert(arena: &mut Arena, word: &[u8]) {
    let mut curr = 0;
    for &c in word {
        let idx = (c - b'a') as usize;
        let child = arena.nodes[curr as usize].children[idx];
        if child == u32::MAX {
            let new_child = arena.alloc(TrieNode::new());
            arena.nodes[curr as usize].children[idx] = new_child;
            curr = new_child;
        } else {
            curr = child;
        }
    }
    arena.nodes[curr as usize].is_word = true;
}

fn search(arena: &Arena, word: &[u8]) -> bool {
    let mut curr = 0;
    for &c in word {
        let idx = (c - b'a') as usize;
        let child = arena.nodes[curr as usize].children[idx];
        if child == u32::MAX {
            return false;
        }
        curr = child;
    }
    arena.nodes[curr as usize].is_word
}

fn main() {
    let mut arena = Arena::new(3_000_000);
    let mut word = [0u8; 5];

    println!("Inserting 700k words...");
    for i in 0..700000u32 {
        word[0] = ((i % 26) + 97) as u8;
        word[1] = (((i / 26) % 26) + 97) as u8;
        word[2] = (((i / 676) % 26) + 97) as u8;
        word[3] = (((i / 17576) % 26) + 97) as u8;
        word[4] = ((i % 7) + 97) as u8;
        insert(&mut arena, &word);
    }

    println!("Searching 700k words...");
    let mut found = 0i64;
    for i in 0..700000u32 {
        word[0] = ((i % 26) + 97) as u8;
        word[1] = (((i / 26) % 26) + 97) as u8;
        word[2] = (((i / 676) % 26) + 97) as u8;
        word[3] = (((i / 17576) % 26) + 97) as u8;
        word[4] = ((i % 7) + 97) as u8;
        if search(&arena, &word) {
            found += 1;
        }
    }
    println!("Found: {}", found);
}
