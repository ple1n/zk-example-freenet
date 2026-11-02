#![no_main]
#![no_std]

use sha2::{Digest, Sha256};
use zk_notepad_shared::ProofValues;

sp1_zkvm::entrypoint!(main);

pub fn main() {
    let secret: [u8; 32] = sp1_zkvm::io::read();
    let public_y: [u8; 32] = sp1_zkvm::io::read();
    let counter: u64 = sp1_zkvm::io::read();
    let text_hash: [u8; 32] = sp1_zkvm::io::read();

    assert_eq!(<[u8; 32]>::from(Sha256::digest(secret)), public_y);

    sp1_zkvm::io::commit(&ProofValues {
        public_y,
        counter,
        text_hash,
    });
}
