use freenet_stdlib::prelude::*;
use sha2::{Digest, Sha256};
use stwo_cairo_protocol::{Candidate, ContractParameters};
use stwo_cairo_verifier::verify_candidate;

fn parse_parameters(bytes: &[u8]) -> Result<ContractParameters, ContractError> {
    let parameters =
        serde_json::from_slice(bytes).map_err(|error| ContractError::Deser(error.to_string()))?;
    Ok(parameters)
}

fn parse_candidate(bytes: &[u8]) -> Result<Candidate, ContractError> {
    serde_json::from_slice(bytes).map_err(|error| ContractError::Deser(error.to_string()))
}

fn verify(candidate: &Candidate, parameters: &ContractParameters) -> Result<(), ContractError> {
    verify_candidate(candidate, parameters)
        .map(|_| ())
        .map_err(ContractError::Other)
}

fn encode_state(candidate: &Candidate) -> Result<State<'static>, ContractError> {
    serde_json::to_vec(candidate)
        .map(State::from)
        .map_err(|error| ContractError::Other(format!("cannot encode state: {error}")))
}

fn candidate_order(candidate: &Candidate) -> &[u8] {
    candidate.proof.as_bytes()
}

#[contract]
impl ContractInterface for StwoCairoContract {
    fn validate_state(
        parameters: Parameters<'static>,
        state: State<'static>,
        _related: RelatedContracts<'static>,
    ) -> Result<ValidateResult, ContractError> {
        if state.is_empty() {
            return Ok(ValidateResult::Valid);
        }
        let parameters = parse_parameters(parameters.as_ref())?;
        let candidate = parse_candidate(state.as_ref())?;
        verify(&candidate, &parameters)?;
        Ok(ValidateResult::Valid)
    }

    fn update_state(
        parameters: Parameters<'static>,
        state: State<'static>,
        data: Vec<UpdateData<'static>>,
    ) -> Result<UpdateModification<'static>, ContractError> {
        let parameters = parse_parameters(parameters.as_ref())?;
        let mut current = if state.is_empty() {
            None
        } else {
            let candidate = parse_candidate(state.as_ref())?;
            verify(&candidate, &parameters)?;
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
            let incoming = parse_candidate(delta.as_ref())?;
            verify(&incoming, &parameters)?;
            if current
                .as_ref()
                .is_none_or(|old| candidate_order(&incoming) > candidate_order(old))
            {
                current = Some(incoming);
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
        Ok(StateSummary::from(Sha256::digest(state.as_ref()).to_vec()))
    }

    fn get_state_delta(
        _parameters: Parameters<'static>,
        state: State<'static>,
        summary: StateSummary<'static>,
    ) -> Result<StateDelta<'static>, ContractError> {
        if state.is_empty() {
            return Ok(StateDelta::from(Vec::new()));
        }
        let digest = Sha256::digest(state.as_ref());
        if summary.as_ref() == &digest[..] {
            return Ok(StateDelta::from(Vec::new()));
        }
        Ok(StateDelta::from(state.as_ref().to_vec()))
    }
}

pub struct StwoCairoContract;

#[cfg(test)]
mod tests {
    use super::candidate_order;
    use stwo_cairo_protocol::Candidate;

    #[test]
    fn candidate_order_is_deterministic() {
        let left = Candidate::new("01".into());
        let right = Candidate::new("02".into());
        assert!(candidate_order(&right) > candidate_order(&left));
    }
}
