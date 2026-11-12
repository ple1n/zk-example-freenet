use core::hash::HashStateTrait;
use core::poseidon::PoseidonTrait;

pub fn poseidon_hash(secret: felt252) -> (u128, u128) {
    let result = PoseidonTrait::new().update(secret).finalize();
    let result: u256 = result.try_into().unwrap();
    (result.low, result.high)
}
