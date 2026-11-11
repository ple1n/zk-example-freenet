use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u8 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub version: u8,
    pub proof: String,
}

impl Candidate {
    pub fn new(proof: String) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            proof,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractParameters {
    pub version: u8,
    pub program_hash: String,
    pub public_output: Vec<String>,
}

impl ContractParameters {
    pub fn new(program_hash: String, public_output: Vec<String>) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            program_hash,
            public_output,
        }
    }
}
