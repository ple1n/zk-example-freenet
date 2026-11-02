#![no_main]
#![no_std]

extern crate alloc;

use zk_notepad_shared::ProofValues;
use zkm_zkvm::lib::poseidon2::poseidon2;

zkm_zkvm::entrypoint!(main);

pub fn main() {
    let secret: [u8; 32] = zkm_zkvm::io::read();
    let public_y: [u8; 32] = zkm_zkvm::io::read();
    let counter: u64 = zkm_zkvm::io::read();
    let text_hash: [u8; 32] = zkm_zkvm::io::read();

    assert_eq!(poseidon2(&secret), public_y);

    zkm_zkvm::io::commit(&ProofValues {
        public_y,
        counter,
        text_hash,
    });
}
