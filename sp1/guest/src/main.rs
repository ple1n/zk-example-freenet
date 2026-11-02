#![no_main]
#![no_std]

use sp1_merkle_shared::poseidon2_bytes;
use zk_notepad_shared::ProofValues;

sp1_zkvm::entrypoint!(main);

pub fn main() {
    let secret: [u8; 32] = sp1_zkvm::io::read();
    let public_y: [u8; 32] = sp1_zkvm::io::read();
    let counter: u64 = sp1_zkvm::io::read();
    let text_hash: [u8; 32] = sp1_zkvm::io::read();

    assert_eq!(poseidon2_bytes(&secret), public_y);

    sp1_zkvm::io::commit(&ProofValues {
        public_y,
        counter,
        text_hash,
    });
}
