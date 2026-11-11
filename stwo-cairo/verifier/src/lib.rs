use std::io::Read;

use bzip2::read::BzDecoder;
use cairo_air::{
    CairoProofForRustVerifier,
    utils::{VerificationOutput, get_verification_output},
    verifier::verify_cairo_ex,
};
use stwo::core::{channel::MerkleChannel, vcs_lifted::blake2_merkle::Blake2sMerkleChannel};
use stwo_cairo_protocol::{Candidate, ContractParameters, PROTOCOL_VERSION};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedOutput {
    pub program_hash: String,
    pub public_output: Vec<String>,
}

pub fn inspect_candidate(candidate: &Candidate) -> Result<VerifiedOutput, String> {
    if candidate.version != PROTOCOL_VERSION {
        return Err(format!(
            "unsupported candidate version {}; expected {}",
            candidate.version, PROTOCOL_VERSION
        ));
    }
    let proof_bytes =
        hex::decode(&candidate.proof).map_err(|error| format!("invalid proof hex: {error}"))?;
    let mut decoder = BzDecoder::new(proof_bytes.as_slice());
    let mut encoded = Vec::new();
    decoder
        .read_to_end(&mut encoded)
        .map_err(|error| format!("invalid compressed proof: {error}"))?;
    let proof: CairoProofForRustVerifier<<Blake2sMerkleChannel as MerkleChannel>::H> =
        bincode::deserialize(&encoded)
            .map_err(|error| format!("invalid serialized proof: {error}"))?;

    let output: VerificationOutput =
        get_verification_output(&proof.claim.public_data.public_memory);
    verify_cairo_ex::<Blake2sMerkleChannel>(proof, false)
        .map_err(|error| format!("Stwo Cairo proof verification failed: {error}"))?;

    Ok(VerifiedOutput {
        program_hash: output.program_hash.to_string(),
        public_output: output.output.iter().map(ToString::to_string).collect(),
    })
}

pub fn verify_candidate(
    candidate: &Candidate,
    parameters: &ContractParameters,
) -> Result<VerifiedOutput, String> {
    if parameters.version != PROTOCOL_VERSION {
        return Err(format!(
            "unsupported contract parameter version {}; expected {}",
            parameters.version, PROTOCOL_VERSION
        ));
    }
    let output = inspect_candidate(candidate)?;
    if output.program_hash != parameters.program_hash {
        return Err("proof program hash does not match contract parameters".into());
    }
    if output.public_output != parameters.public_output {
        return Err("proof public output does not match contract parameters".into());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{PROTOCOL_VERSION, inspect_candidate};
    use stwo_cairo_protocol::Candidate;

    #[test]
    fn rejects_unsupported_candidate_version_before_decoding_proof() {
        let mut candidate = Candidate::new("not-hex".into());
        candidate.version = PROTOCOL_VERSION + 1;

        let error = inspect_candidate(&candidate).unwrap_err();

        assert!(error.contains("unsupported candidate version"));
    }

    #[test]
    fn rejects_invalid_compressed_proof() {
        let error = inspect_candidate(&Candidate::new("00".into())).unwrap_err();

        assert!(error.contains("invalid compressed proof"));
    }
}
