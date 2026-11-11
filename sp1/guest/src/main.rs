#![no_main]
#![no_std]

sp1_zkvm::entrypoint!(main);

pub fn main() {
    let input = sp1_zkvm::io::read_vec();
    assert_eq!(input.len(), 104);

    let mut secret = [0u8; 32];
    secret.copy_from_slice(&input[..32]);
    let mut public_y = [0u8; 32];
    public_y.copy_from_slice(&input[32..64]);
    let counter = u64::from_le_bytes(input[64..72].try_into().unwrap());
    let mut text_hash = [0u8; 32];
    text_hash.copy_from_slice(&input[72..]);

    let digest = sp1_zkvm::syscalls::Poseidon2ByteHash::hash(&secret);
    let public_y_words: [u32; 8] = core::array::from_fn(|i| {
        let offset = i * 4;
        u32::from_le_bytes([
            public_y[offset],
            public_y[offset + 1],
            public_y[offset + 2],
            public_y[offset + 3],
        ])
    });
    assert_eq!(digest, public_y_words);

    let mut public_values = [0u8; 72];
    public_values[..32].copy_from_slice(&public_y);
    public_values[32..40].copy_from_slice(&counter.to_le_bytes());
    public_values[40..].copy_from_slice(&text_hash);
    sp1_zkvm::io::commit_slice(&public_values);
}
