#![no_main]
#![no_std]

use sp1_merkle_shared::poseidon2_bytes;
use tiny_keccak::{Hasher, Keccak};

sp1_zkvm::entrypoint!(main);

pub fn main() {
    let message: [u8; 32] = sp1_zkvm::io::read();
    let expected_keccak: [u8; 32] = sp1_zkvm::io::read();
    let expected_poseidon2: [u8; 32] = sp1_zkvm::io::read();
    let mut keccak = [0u8; 32];
    let mut hasher = Keccak::v256();
    hasher.update(&message);
    hasher.finalize(&mut keccak);
    let poseidon2 = poseidon2_bytes(&message);
    assert_eq!(keccak, expected_keccak);
    assert_eq!(poseidon2, expected_poseidon2);
    sp1_zkvm::io::commit_slice(&keccak);
    sp1_zkvm::io::commit_slice(&poseidon2);
}
