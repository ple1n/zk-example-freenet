use freenet_stdlib::prelude::*;
use sp1_merkle_shared::{
    poseidon2_bytes, MerkleCandidate, MerkleContractParameters, MerkleProofValues, MAX_TREE_DEPTH,
};
use sp1_verifier::compressed::SP1CompressedVerifierRaw;

#[cfg(target_arch = "wasm32")]
#[no_mangle]
unsafe extern "Rust" fn __getrandom_v02_custom(
    _dest: *mut u8,
    _len: usize,
) -> Result<(), getrandom::Error> {
    Err(getrandom::Error::UNSUPPORTED)
}

fn parse_parameters(bytes: &[u8]) -> Result<(MerkleContractParameters, Vec<u8>), ContractError> {
    let params: MerkleContractParameters =
        serde_json::from_slice(bytes).map_err(|e| ContractError::Deser(e.to_string()))?;
    if params.tree_depth == 0 || params.tree_depth > MAX_TREE_DEPTH {
        return Err(ContractError::Other(
            "tree_depth is outside the supported range".into(),
        ));
    }
    let verifying_key = hex::decode(&params.verifying_key)
        .map_err(|e| ContractError::Other(format!("invalid SP1 verifying-key hash: {e}")))?;
    if verifying_key.is_empty() {
        return Err(ContractError::Other(
            "SP1 verifying-key hash cannot be empty".into(),
        ));
    }
    Ok((params, verifying_key))
}

fn verify_candidate(
    params: &MerkleContractParameters,
    verifying_key_hash: &[u8],
    candidate: &MerkleCandidate,
) -> Result<(), ContractError> {
    let proof = hex::decode(&candidate.proof)
        .map_err(|e| ContractError::Other(format!("invalid proof encoding: {e}")))?;
    let public_values = bincode::serialize(&MerkleProofValues {
        tree_depth: params.tree_depth,
        root: params.root,
    })
    .map_err(|e| ContractError::Other(format!("cannot encode public values: {e}")))?;
    SP1CompressedVerifierRaw::verify_with_public_values(&proof, &public_values, verifying_key_hash)
        .map_err(|e| ContractError::Other(format!("SP1 proof verification failed: {e}")))
}

fn encode_state(candidate: &MerkleCandidate) -> Result<State<'static>, ContractError> {
    serde_json::to_vec(candidate)
        .map(State::from)
        .map_err(|e| ContractError::Other(format!("cannot serialize Merkle state: {e}")))
}

#[contract]
impl ContractInterface for Sp1MerkleContract {
    fn validate_state(
        parameters: Parameters<'static>,
        state: State<'static>,
        _related: RelatedContracts<'static>,
    ) -> Result<ValidateResult, ContractError> {
        if state.is_empty() {
            return Ok(ValidateResult::Valid);
        }
        let (params, verifying_key_hash) = parse_parameters(parameters.as_ref())?;
        let candidate: MerkleCandidate = serde_json::from_slice(state.as_ref())
            .map_err(|e| ContractError::Deser(e.to_string()))?;
        verify_candidate(&params, &verifying_key_hash, &candidate)?;
        Ok(ValidateResult::Valid)
    }

    fn update_state(
        parameters: Parameters<'static>,
        state: State<'static>,
        data: Vec<UpdateData<'static>>,
    ) -> Result<UpdateModification<'static>, ContractError> {
        let (params, verifying_key_hash) = parse_parameters(parameters.as_ref())?;
        let mut current = if state.is_empty() {
            None
        } else {
            let candidate: MerkleCandidate = serde_json::from_slice(state.as_ref())
                .map_err(|e| ContractError::Deser(e.to_string()))?;
            verify_candidate(&params, &verifying_key_hash, &candidate)?;
            Some(candidate)
        };

        for update in data {
            let delta = match update {
                UpdateData::Delta(delta) => delta,
                _ => return Err(ContractError::InvalidUpdate),
            };
            if delta.is_empty() {
                continue;
            }
            let candidate: MerkleCandidate = serde_json::from_slice(delta.as_ref())
                .map_err(|e| ContractError::Deser(e.to_string()))?;
            verify_candidate(&params, &verifying_key_hash, &candidate)?;
            if current
                .as_ref()
                .is_none_or(|old| candidate.proof > old.proof)
            {
                current = Some(candidate);
            }
        }

        match current {
            Some(candidate) => Ok(UpdateModification::valid(encode_state(&candidate)?)),
            None => Ok(UpdateModification::valid(State::from(Vec::new()))),
        }
    }

    fn summarize_state(
        _parameters: Parameters<'static>,
        state: State<'static>,
    ) -> Result<StateSummary<'static>, ContractError> {
        if state.is_empty() {
            return Ok(StateSummary::from(Vec::new()));
        }
        Ok(StateSummary::from(poseidon2_bytes(state.as_ref()).to_vec()))
    }

    fn get_state_delta(
        _parameters: Parameters<'static>,
        state: State<'static>,
        summary: StateSummary<'static>,
    ) -> Result<StateDelta<'static>, ContractError> {
        if state.is_empty() {
            return Ok(StateDelta::from(Vec::new()));
        }
        if summary.as_ref() == poseidon2_bytes(state.as_ref()) {
            return Ok(StateDelta::from(Vec::new()));
        }
        Ok(StateDelta::from(state.as_ref().to_vec()))
    }
}

pub struct Sp1MerkleContract;
