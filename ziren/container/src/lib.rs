use freenet_stdlib::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zk_notepad_shared::{ContractParameters, NoteCandidate, ProofValues, poseidon2_bytes};
use zkm_verifier::StarkVerifier;

#[derive(Deserialize, Serialize)]
struct NoteState {
    counter: u64,
    text: String,
    proof: String,
}

fn parse_parameters(bytes: &[u8]) -> Result<([u8; 32], Vec<u8>), ContractError> {
    let params: ContractParameters =
        serde_json::from_slice(bytes).map_err(|e| ContractError::Deser(e.to_string()))?;
    let public_y = hex::decode(&params.public_y)
        .map_err(|e| ContractError::Other(format!("invalid public_y: {e}")))?;
    let public_y: [u8; 32] = public_y
        .try_into()
        .map_err(|_| ContractError::Other("public_y must decode to 32 bytes".into()))?;
    let verifying_key = hex::decode(&params.verifying_key)
        .map_err(|e| ContractError::Other(format!("invalid verifying_key: {e}")))?;
    if verifying_key.is_empty() {
        return Err(ContractError::Other("verifying_key cannot be empty".into()));
    }
    Ok((public_y, verifying_key))
}

fn verify_candidate(
    public_y: &[u8; 32],
    verifying_key: &[u8],
    candidate: &NoteCandidate,
) -> Result<NoteState, ContractError> {
    let proof = hex::decode(&candidate.proof)
        .map_err(|e| ContractError::Other(format!("invalid proof encoding: {e}")))?;
    let text_hash = poseidon2_bytes(candidate.text.as_bytes());
    let public_values = bincode::serialize(&ProofValues {
        public_y: *public_y,
        counter: candidate.counter,
        text_hash,
    })
    .map_err(|e| ContractError::Other(format!("cannot encode public values: {e}")))?;

    StarkVerifier::verify(&proof, &public_values, verifying_key)
        .map_err(|e| ContractError::Other(format!("Ziren proof verification failed: {e}")))?;

    Ok(NoteState {
        counter: candidate.counter,
        text: candidate.text.clone(),
        proof: candidate.proof.clone(),
    })
}

fn candidate_order(candidate: &NoteState) -> (u64, [u8; 32], &[u8], &[u8]) {
    (
        candidate.counter,
        poseidon2_bytes(candidate.text.as_bytes()),
        candidate.text.as_bytes(),
        candidate.proof.as_bytes(),
    )
}

fn encode_state(state: &NoteState) -> Result<State<'static>, ContractError> {
    serde_json::to_vec(state)
        .map(State::from)
        .map_err(|e| ContractError::Other(format!("cannot serialize notepad state: {e}")))
}

#[contract]
impl ContractInterface for NotepadContract {
    fn validate_state(
        parameters: Parameters<'static>,
        state: State<'static>,
        _related: RelatedContracts<'static>,
    ) -> Result<ValidateResult, ContractError> {
        if state.is_empty() {
            return Ok(ValidateResult::Valid);
        }
        let (public_y, verifying_key) = parse_parameters(parameters.as_ref())?;
        let candidate: NoteCandidate = serde_json::from_slice(state.as_ref())
            .map_err(|e| ContractError::Deser(e.to_string()))?;
        verify_candidate(&public_y, &verifying_key, &candidate)?;
        Ok(ValidateResult::Valid)
    }

    fn update_state(
        parameters: Parameters<'static>,
        state: State<'static>,
        data: Vec<UpdateData<'static>>,
    ) -> Result<UpdateModification<'static>, ContractError> {
        let (public_y, verifying_key) = parse_parameters(parameters.as_ref())?;
        let mut current = if state.is_empty() {
            None
        } else {
            let candidate: NoteCandidate = serde_json::from_slice(state.as_ref())
                .map_err(|e| ContractError::Deser(e.to_string()))?;
            Some(verify_candidate(&public_y, &verifying_key, &candidate)?)
        };

        for update in data {
            let delta = match update {
                UpdateData::Delta(delta) => delta,
                _ => return Err(ContractError::InvalidUpdate),
            };
            if delta.is_empty() {
                continue;
            }
            let candidate: NoteCandidate = serde_json::from_slice(delta.as_ref())
                .map_err(|e| ContractError::Deser(e.to_string()))?;
            let incoming = verify_candidate(&public_y, &verifying_key, &candidate)?;
            if current
                .as_ref()
                .is_none_or(|old| candidate_order(&incoming) > candidate_order(old))
            {
                current = Some(incoming);
            }
        }

        match current {
            Some(state) => Ok(UpdateModification::valid(encode_state(&state)?)),
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
        let digest = Sha256::digest(state.as_ref()).to_vec();
        Ok(StateSummary::from(digest))
    }

    fn get_state_delta(
        _parameters: Parameters<'static>,
        state: State<'static>,
        summary: StateSummary<'static>,
    ) -> Result<StateDelta<'static>, ContractError> {
        if state.is_empty() {
            return Ok(StateDelta::from(Vec::new()));
        }
        let current_digest = Sha256::digest(state.as_ref());
        if summary.as_ref() == &current_digest[..] {
            return Ok(StateDelta::from(Vec::new()));
        }
        Ok(StateDelta::from(state.as_ref().to_vec()))
    }
}

pub struct NotepadContract;

#[cfg(test)]
mod tests {
    use super::{NoteState, candidate_order};

    fn note(counter: u64, text: &str, proof: &str) -> NoteState {
        NoteState {
            counter,
            text: text.into(),
            proof: proof.into(),
        }
    }

    #[test]
    fn higher_counter_wins_over_hash_tiebreak() {
        let lower = note(7, "z", "ff");
        let higher = note(8, "a", "00");
        assert!(candidate_order(&higher) > candidate_order(&lower));
    }

    #[test]
    fn equal_counters_have_deterministic_ordering() {
        let left = note(7, "alpha", "01");
        let right = note(7, "beta", "02");
        assert_ne!(candidate_order(&left), candidate_order(&right));
        let forward = if candidate_order(&left) > candidate_order(&right) {
            &left
        } else {
            &right
        };
        let reverse = if candidate_order(&right) > candidate_order(&left) {
            &right
        } else {
            &left
        };
        assert_eq!(forward.text, reverse.text);
    }
}
