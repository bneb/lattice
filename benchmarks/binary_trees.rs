use std::env;

struct TreeNode {
    left: Option<Box<TreeNode>>,
    right: Option<Box<TreeNode>>,
}

fn create_tree(depth: i32) -> Option<Box<TreeNode>> {
    if depth > 0 {
        Some(Box::new(TreeNode {
            left: create_tree(depth - 1),
            right: create_tree(depth - 1),
        }))
    } else {
        Some(Box::new(TreeNode {
            left: None,
            right: None,
        }))
    }
}

fn compute_checksum(node: &Option<Box<TreeNode>>) -> i32 {
    match node {
        Some(n) => 1 + compute_checksum(&n.left) + compute_checksum(&n.right),
        None => 0,
    }
}

fn main() {
    let mut max_depth = 15;
    if let Some(arg) = env::args().nth(1) {
        max_depth = arg.parse().unwrap_or(15);
    }
    
    // Stretch tree
    let stretch_depth = max_depth + 1;
    let stretch_tree = create_tree(stretch_depth);
    let _stretch_check = compute_checksum(&stretch_tree);
    drop(stretch_tree);
    
    let long_lived_tree = create_tree(max_depth);
    
    let mut total_check: i64 = 0;
    
    let mut depth = 4;
    while depth <= max_depth {
        let iterations = 1 << (max_depth - depth + 4);
        let mut check: i64 = 0;
        
        for _ in 1..=iterations {
            let t = create_tree(depth);
            check += compute_checksum(&t) as i64;
            // t is dropped here
        }
        
        total_check += check;
        depth += 2;
    }
    
    let _long_lived_check = compute_checksum(&long_lived_tree);
    
    std::process::exit((total_check % 256) as i32);
}
