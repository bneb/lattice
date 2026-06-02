#include <stdio.h>
#include <stdlib.h>

typedef struct TreeNode {
    struct TreeNode* left;
    struct TreeNode* right;
} TreeNode;

TreeNode* create_tree(int depth) {
    TreeNode* node = (TreeNode*)malloc(sizeof(TreeNode));
    if (depth > 0) {
        node->left = create_tree(depth - 1);
        node->right = create_tree(depth - 1);
    } else {
        node->left = NULL;
        node->right = NULL;
    }
    return node;
}

int compute_checksum(TreeNode* node) {
    if (node) {
        int checksum = 1 + compute_checksum(node->left) + compute_checksum(node->right);
        free(node);
        return checksum;
    }
    return 0;
}

int main(int argc, char* argv[]) {
    int max_depth = 15;
    if (argc > 1) {
        max_depth = atoi(argv[1]);
    }
    
    // Stretch tree
    int stretch_depth = max_depth + 1;
    TreeNode* stretch_tree = create_tree(stretch_depth);
    int stretch_check = compute_checksum(stretch_tree);
    // Note: compute_checksum also frees the nodes in this C implementation
    
    TreeNode* long_lived_tree = create_tree(max_depth);
    
    long total_check = 0;
    
    for (int depth = 4; depth <= max_depth; depth += 2) {
        int iterations = 1 << (max_depth - depth + 4);
        long check = 0;
        
        for (int i = 1; i <= iterations; ++i) {
            TreeNode* t = create_tree(depth);
            check += compute_checksum(t);
        }
        total_check += check;
    }
    
    long long_lived_check = compute_checksum(long_lived_tree);
    
    return total_check % 256;
}
