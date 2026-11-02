#![no_main]
#![no_std]

extern crate alloc;

use sp1_merkle_shared::{hash_leaf, hash_pair, MerkleNode, MerkleProofValues, MAX_TREE_DEPTH};

sp1_zkvm::entrypoint!(main);

pub fn main() {
    let tree_depth: u32 = sp1_zkvm::io::read();
    let leaf_index: u32 = sp1_zkvm::io::read();
    let leaf: MerkleNode = sp1_zkvm::io::read();
    let siblings: alloc::vec::Vec<MerkleNode> = sp1_zkvm::io::read();
    let root: MerkleNode = sp1_zkvm::io::read();

    assert!(tree_depth > 0 && tree_depth <= MAX_TREE_DEPTH);
    assert!(siblings.len() == tree_depth as usize);
    assert!(leaf_index < (1u32 << tree_depth));

    let mut node = hash_leaf(leaf_index, &leaf);
    for (level, sibling) in siblings.iter().enumerate() {
        node = if (leaf_index >> level) & 1 == 0 {
            hash_pair(&node, sibling)
        } else {
            hash_pair(sibling, &node)
        };
    }
    assert_eq!(node, root);

    sp1_zkvm::io::commit(&MerkleProofValues { tree_depth, root });
}
