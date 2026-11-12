use core::hash::HashStateTrait;
use core::poseidon::PoseidonTrait;

fn hash_pair(left: felt252, right: felt252) -> felt252 {
    PoseidonTrait::new().update(left).update(right).finalize()
}

pub fn poseidon_merkle(
    leaf: felt252,
    sibling_0: felt252,
    sibling_1: felt252,
    bit_0: bool,
    bit_1: bool,
) -> (u128, u128) {
    let node_0 = if bit_0 {
        hash_pair(sibling_0, leaf)
    } else {
        hash_pair(leaf, sibling_0)
    };
    let root = if bit_1 {
        hash_pair(sibling_1, node_0)
    } else {
        hash_pair(node_0, sibling_1)
    };
    let root: u256 = root.try_into().unwrap();
    (root.low, root.high)
}
