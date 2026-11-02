use serde::{Deserialize, Serialize};

#[cfg(feature = "hash")]
use p3_field::{FieldAlgebra, PrimeField32};
#[cfg(feature = "hash")]
use p3_koala_bear::KoalaBear;
#[cfg(feature = "hash")]
use p3_symmetric::Permutation;
#[cfg(feature = "hash")]
use zkm_primitives::poseidon2_init;

#[cfg(feature = "hash")]
const RATE: usize = 8;
#[cfg(feature = "hash")]
const WIDTH: usize = 16;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProofValues {
    pub public_y: [u8; 32],
    pub counter: u64,
    pub text_hash: [u8; 32],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContractParameters {
    pub public_y: String,
    pub verifying_key: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NoteCandidate {
    pub counter: u64,
    pub text: String,
    pub proof: String,
}

#[cfg(feature = "hash")]
pub fn poseidon2_bytes(input: &[u8]) -> [u8; 32] {
    let len = input.len();
    let padded_len = (len + 3) / 3 * 3;
    let mut padded = input.to_vec();
    padded.resize(padded_len, 0);

    if len % 3 == 2 {
        padded[len] = 0b1000_0001;
    } else {
        padded[len] = 1;
        padded[padded_len - 1] = 0b1000_0000;
    }

    let elements: Vec<KoalaBear> = padded
        .chunks(3)
        .map(|chunk| {
            let value = chunk
                .iter()
                .enumerate()
                .fold(0u32, |acc, (i, byte)| acc | (u32::from(*byte) << (i * 8)));
            KoalaBear::from_canonical_u32(value)
        })
        .collect();

    let permutation = poseidon2_init();
    let mut state = [KoalaBear::ZERO; WIDTH];
    for chunk in elements.chunks(RATE) {
        state[..chunk.len()].copy_from_slice(chunk);
        state = permutation.permute(state);
    }

    let mut output = [0u8; 32];
    for (index, byte) in state
        .iter()
        .flat_map(|x| x.as_canonical_u32().to_le_bytes())
        .take(output.len())
        .enumerate()
    {
        output[index] = byte;
    }
    output
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "hash")]
    use super::poseidon2_bytes;

    #[cfg(feature = "hash")]
    #[test]
    fn matches_ziren_poseidon2_guest_encoding() {
        let input = vec![1u8; 1000];
        assert_eq!(
            hex::encode(poseidon2_bytes(&input)),
            "2f8cb27c50875703e37d9b648e95ae6543cc305f88e61976221fa34fe0354849"
        );
    }
}
