#![no_std]

extern crate alloc;

use alloc::{string::String, vec, vec::Vec};
use serde::{Deserialize, Serialize};
use slop_algebra::{AbstractField, PrimeField32};
use sp1_primitives::{poseidon2_hash, SP1Field};

pub type MerkleNode = [u32; 8];
pub const MAX_TREE_DEPTH: u32 = 20;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MerkleProofValues {
    pub tree_depth: u32,
    pub root: MerkleNode,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MerkleContractParameters {
    pub tree_depth: u32,
    pub root: MerkleNode,
    pub verifying_key: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MerkleCandidate {
    pub proof: String,
}

fn poseidon2_byte_hash(input: &[u8]) -> [u32; 8] {
    #[cfg(target_os = "zkvm")]
    {
        sp1_zkvm::syscalls::Poseidon2ByteHash::hash(input)
    }

    #[cfg(not(target_os = "zkvm"))]
    {
        let mut fields = Vec::new();
        let mut length_block = [0u8; 24];
        length_block[..core::mem::size_of::<usize>()].copy_from_slice(&input.len().to_le_bytes());
        append_byte_block_fields(&mut fields, &length_block);
        for chunk in input.chunks(24) {
            let mut block = [0u8; 24];
            block[..chunk.len()].copy_from_slice(chunk);
            append_byte_block_fields(&mut fields, &block);
        }
        poseidon2_hash(fields.into_iter().map(SP1Field::from_wrapped_u32).collect())
            .map(|value| value.as_canonical_u32())
    }
}

#[cfg(not(target_os = "zkvm"))]
fn append_byte_block_fields(fields: &mut Vec<u32>, block: &[u8; 24]) {
    for chunk in block.chunks_exact(3) {
        fields.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], 0]));
    }
}

fn digest_words_to_bytes(digest: &[u32; 8]) -> [u8; 32] {
    core::array::from_fn(|i| digest[i / 4].to_le_bytes()[i % 4])
}

fn hash_fields(values: &[u32]) -> MerkleNode {
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for value in values {
        let canonical = SP1Field::from_wrapped_u32(*value).as_canonical_u32();
        bytes.extend_from_slice(&canonical.to_le_bytes());
    }
    poseidon2_byte_hash(&bytes)
}

fn seed_fields(seed: &[u8; 32]) -> MerkleNode {
    core::array::from_fn(|i| {
        u32::from_le_bytes(seed[i * 4..i * 4 + 4].try_into().expect("fixed seed size"))
            & 0x3fff_ffff
    })
}

pub fn derive_leaf(seed: &[u8; 32], index: u32) -> MerkleNode {
    let mut input = vec![2, index];
    input.extend_from_slice(&seed_fields(seed));
    hash_fields(&input)
}

pub fn hash_leaf(index: u32, leaf: &MerkleNode) -> MerkleNode {
    let mut input = vec![0, index];
    input.extend_from_slice(leaf);
    hash_fields(&input)
}

pub fn hash_pair(left: &MerkleNode, right: &MerkleNode) -> MerkleNode {
    let mut input = vec![1];
    input.extend_from_slice(left);
    input.extend_from_slice(right);
    hash_fields(&input)
}

pub fn build_tree(seed: &[u8; 32], tree_depth: u32) -> Option<Vec<Vec<MerkleNode>>> {
    if tree_depth == 0 || tree_depth > MAX_TREE_DEPTH {
        return None;
    }

    let leaf_count = 1u32 << tree_depth;
    let mut levels = Vec::with_capacity(tree_depth as usize + 1);
    let leaves: Vec<MerkleNode> = (0..leaf_count)
        .map(|index| hash_leaf(index, &derive_leaf(seed, index)))
        .collect();
    levels.push(leaves);

    for _ in 0..tree_depth {
        let current = levels.last()?;
        let next = current
            .chunks_exact(2)
            .map(|pair| hash_pair(&pair[0], &pair[1]))
            .collect();
        levels.push(next);
    }
    Some(levels)
}

pub fn membership_witness(
    seed: &[u8; 32],
    tree_depth: u32,
    leaf_index: u32,
) -> Option<(MerkleNode, Vec<MerkleNode>, MerkleNode)> {
    let levels = build_tree(seed, tree_depth)?;
    if leaf_index >= levels[0].len() as u32 {
        return None;
    }

    let leaf = derive_leaf(seed, leaf_index);
    let mut index = leaf_index as usize;
    let mut siblings = Vec::with_capacity(tree_depth as usize);
    for level in levels.iter().take(tree_depth as usize) {
        siblings.push(level[index ^ 1]);
        index >>= 1;
    }
    Some((leaf, siblings, levels[tree_depth as usize][0]))
}

pub fn poseidon2_bytes(input: &[u8]) -> [u8; 32] {
    digest_words_to_bytes(&poseidon2_byte_hash(input))
}

#[cfg(test)]
mod tests {
    use super::{build_tree, hash_leaf, hash_pair, membership_witness, MAX_TREE_DEPTH};

    #[test]
    fn membership_witness_reconstructs_public_root() {
        let seed = [7u8; 32];
        let levels = build_tree(&seed, 4).unwrap();
        let (leaf, siblings, root) = membership_witness(&seed, 4, 9).unwrap();
        assert_eq!(siblings.len(), 4);
        assert_eq!(root, levels[4][0]);
        assert_ne!(root, [0; 8]);
        let mut node = hash_leaf(9, &leaf);
        for (level, sibling) in siblings.iter().enumerate() {
            node = if (9 >> level) & 1 == 0 {
                hash_pair(&node, sibling)
            } else {
                hash_pair(sibling, &node)
            };
        }
        assert_eq!(node, root);
        assert!(membership_witness(&seed, 4, 16).is_none());
    }

    #[test]
    fn rejects_unsupported_tree_depths() {
        let seed = [0; 32];
        assert!(build_tree(&seed, 0).is_none());
        assert!(build_tree(&seed, MAX_TREE_DEPTH + 1).is_none());
    }
}
