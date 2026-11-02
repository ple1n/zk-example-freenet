use std::{fs, path::PathBuf, time::Instant};

use anyhow::{bail, Context};
use clap::{Parser, Subcommand};
use freenet_stdlib::{
    client_api::{ClientRequest, ContractRequest, ContractResponse, HostResponse, WebApi},
    prelude::{ContractCode, ContractKey, Parameters, StateDelta, UpdateData},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sp1_sdk::{
    include_elf, Elf, HashableKey, ProveRequest, Prover, ProverClient, ProvingKey, SP1Proof,
    SP1ProvingKey, SP1Stdin,
};
use sp1_verifier::compressed::SP1CompressedVerifierRaw;
use zk_notepad_shared::{ContractParameters, NoteCandidate, ProofValues};

const GUEST_ELF: Elf = include_elf!("sp1-notepad-guest");

#[derive(Parser)]
#[command(about = "Generate and submit proofs for the SP1-authenticated notepad")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Setup {
        #[arg(long)]
        secret: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        proving_key: PathBuf,
    },
    Prove {
        #[arg(long)]
        secret: PathBuf,
        #[arg(long)]
        parameters: PathBuf,
        #[arg(long)]
        proving_key: PathBuf,
        #[arg(long)]
        counter: u64,
        #[arg(long)]
        text: String,
        #[arg(long)]
        output: PathBuf,
    },
    Verify {
        #[arg(long)]
        parameters: PathBuf,
        #[arg(long)]
        candidate: PathBuf,
    },
    Submit {
        #[arg(long)]
        endpoint: String,
        #[arg(long)]
        contract_code: PathBuf,
        #[arg(long)]
        parameters: PathBuf,
        #[arg(long)]
        candidate: PathBuf,
    },
}

fn read_secret(path: &PathBuf) -> anyhow::Result<[u8; 32]> {
    let bytes = fs::read(path).with_context(|| format!("reading secret {}", path.display()))?;
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        anyhow::anyhow!("secret must be exactly 32 bytes, got {}", bytes.len())
    })
}

fn write_json(path: &PathBuf, value: &impl Serialize) -> anyhow::Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes).with_context(|| format!("writing {}", path.display()))
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn decode_public_y(public_y: &str) -> anyhow::Result<[u8; 32]> {
    hex::decode(public_y)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("public_y must decode to 32 bytes"))
}

fn decode_verifying_key_hash(parameters: &ContractParameters) -> anyhow::Result<Vec<u8>> {
    let bytes =
        hex::decode(&parameters.verifying_key).context("decoding SP1 verifying-key hash")?;
    if bytes.is_empty() {
        bail!("SP1 verifying-key hash cannot be empty");
    }
    Ok(bytes)
}

fn verify_proof(
    proof: &[u8],
    public_values: &[u8],
    verifying_key_hash: &[u8],
) -> anyhow::Result<()> {
    let started = Instant::now();
    let result = SP1CompressedVerifierRaw::verify_with_public_values(
        proof,
        public_values,
        verifying_key_hash,
    )
    .context("verifying SP1 compressed proof with the WASM verifier");
    println!(
        "Proof verification took {:.3}s",
        started.elapsed().as_secs_f64()
    );
    match result {
        Ok(()) => {
            println!("WASM verifier accepts this proof: yes");
            Ok(())
        }
        Err(error) => {
            eprintln!("WASM verifier accepts this proof: no");
            Err(error)
        }
    }
}

fn read_proving_key(path: &PathBuf) -> anyhow::Result<SP1ProvingKey> {
    let bytes =
        fs::read(path).with_context(|| format!("reading proving key {}", path.display()))?;
    let proving_key: SP1ProvingKey = bincode::deserialize(&bytes)
        .with_context(|| format!("deserializing proving key {}", path.display()))?;
    if &**proving_key.elf() != &*GUEST_ELF {
        bail!(
            "proving key {} was generated for a different guest ELF; remove it and rerun setup",
            path.display()
        );
    }
    Ok(proving_key)
}

async fn setup(
    secret_path: PathBuf,
    output: PathBuf,
    proving_key_path: PathBuf,
) -> anyhow::Result<()> {
    let command_started = Instant::now();
    let secret = read_secret(&secret_path)?;
    let public_y = sha256(&secret);
    let cache_hit = proving_key_path.exists();
    let key_started = Instant::now();
    let proving_key = if cache_hit {
        read_proving_key(&proving_key_path)?
    } else {
        let client = ProverClient::builder().cpu().build().await;
        let proving_key = client
            .setup(GUEST_ELF.clone())
            .await
            .context("setting up SP1 proving key")?;
        let bytes = bincode::serialize(&proving_key).context("serializing SP1 proving key")?;
        fs::write(&proving_key_path, bytes)
            .with_context(|| format!("writing proving key {}", proving_key_path.display()))?;
        proving_key
    };
    if cache_hit {
        println!(
            "Proving-key cache load took {:.3}s",
            key_started.elapsed().as_secs_f64()
        );
    } else {
        println!(
            "Proving-key setup and cache write took {:.3}s",
            key_started.elapsed().as_secs_f64()
        );
    }
    let verifying_key_hash = bincode::serialize(&proving_key.verifying_key().hash_koalabear())?;
    let parameters = ContractParameters {
        public_y: hex::encode(public_y),
        verifying_key: hex::encode(verifying_key_hash),
    };
    write_json(&output, &parameters)?;
    println!("Wrote public contract parameters to {}", output.display());
    println!(
        "Wrote reusable proving key to {}",
        proving_key_path.display()
    );
    println!("Public y: {}", parameters.public_y);
    println!(
        "Setup command took {:.3}s",
        command_started.elapsed().as_secs_f64()
    );
    Ok(())
}

async fn prove(
    secret_path: PathBuf,
    parameters_path: PathBuf,
    proving_key_path: PathBuf,
    counter: u64,
    text: String,
    output: PathBuf,
) -> anyhow::Result<()> {
    let command_started = Instant::now();
    let secret = read_secret(&secret_path)?;
    let parameters: ContractParameters = serde_json::from_slice(
        &fs::read(&parameters_path)
            .with_context(|| format!("reading {}", parameters_path.display()))?,
    )
    .with_context(|| format!("parsing {}", parameters_path.display()))?;
    let public_y = decode_public_y(&parameters.public_y)?;
    if sha256(&secret) != public_y {
        bail!("secret does not match the public_y in the contract parameters");
    }
    let text_hash = sha256(text.as_bytes());
    let mut stdin = SP1Stdin::new();
    stdin.write(&secret);
    stdin.write(&public_y);
    stdin.write(&counter);
    stdin.write(&text_hash);

    let key_started = Instant::now();
    let proving_key = read_proving_key(&proving_key_path)?;
    println!(
        "Proving-key load took {:.3}s",
        key_started.elapsed().as_secs_f64()
    );
    let verifying_key_hash = bincode::serialize(&proving_key.verifying_key().hash_koalabear())?;
    if decode_verifying_key_hash(&parameters)? != verifying_key_hash {
        bail!(
            "parameters do not match proving key {}; use matching contract parameters",
            proving_key_path.display()
        );
    }

    let client = ProverClient::builder().cpu().build().await;
    let prove_started = Instant::now();
    let proof_result = client
        .prove(&proving_key, stdin)
        .compressed()
        .await
        .context("generating SP1 compressed proof");
    println!(
        "Proof generation took {:.3}s",
        prove_started.elapsed().as_secs_f64()
    );
    let proof = proof_result?;
    if !matches!(&proof.proof, SP1Proof::Compressed(_)) {
        bail!("SP1 prover returned a proof that is not compressed");
    }
    let proof_bytes = bincode::serialize(&proof.proof).context("serializing SP1 proof")?;
    let public_values = bincode::serialize(&ProofValues {
        public_y,
        counter,
        text_hash,
    })?;
    if proof.public_values.as_slice() != public_values {
        bail!("SP1 guest public values differ from the expected notepad values");
    }
    verify_proof(&proof_bytes, &public_values, &verifying_key_hash)
        .context("locally verifying generated proof")?;

    let candidate = NoteCandidate {
        counter,
        text,
        proof: hex::encode(proof_bytes),
    };
    write_json(&output, &candidate)?;
    println!("Wrote verified update candidate to {}", output.display());
    println!(
        "Prove command took {:.3}s",
        command_started.elapsed().as_secs_f64()
    );
    Ok(())
}

fn verify(parameters_path: PathBuf, candidate_path: PathBuf) -> anyhow::Result<()> {
    let command_started = Instant::now();
    let parameters: ContractParameters = serde_json::from_slice(
        &fs::read(&parameters_path)
            .with_context(|| format!("reading {}", parameters_path.display()))?,
    )
    .with_context(|| format!("parsing {}", parameters_path.display()))?;
    let candidate: NoteCandidate = serde_json::from_slice(
        &fs::read(&candidate_path)
            .with_context(|| format!("reading {}", candidate_path.display()))?,
    )
    .with_context(|| format!("parsing {}", candidate_path.display()))?;
    let public_y = decode_public_y(&parameters.public_y)?;
    let verifying_key_hash = decode_verifying_key_hash(&parameters)?;
    let proof = hex::decode(&candidate.proof).context("decoding candidate proof")?;
    let public_values = bincode::serialize(&ProofValues {
        public_y,
        counter: candidate.counter,
        text_hash: sha256(candidate.text.as_bytes()),
    })?;
    verify_proof(&proof, &public_values, &verifying_key_hash)
        .context("candidate proof is invalid")?;
    println!("Candidate proof is valid.");
    println!(
        "Verify command took {:.3}s",
        command_started.elapsed().as_secs_f64()
    );
    Ok(())
}

async fn submit(
    endpoint: String,
    contract_code_path: PathBuf,
    parameters_path: PathBuf,
    candidate_path: PathBuf,
) -> anyhow::Result<()> {
    let contract_code = ContractCode::load_raw(&contract_code_path)
        .with_context(|| format!("reading contract wasm {}", contract_code_path.display()))?;
    let parameters = Parameters::from(
        fs::read(&parameters_path)
            .with_context(|| format!("reading {}", parameters_path.display()))?,
    );
    let key = ContractKey::from_params_and_code(&parameters, &contract_code);
    let delta = fs::read(&candidate_path)
        .with_context(|| format!("reading candidate {}", candidate_path.display()))?;
    let _: NoteCandidate = serde_json::from_slice(&delta).context("parsing candidate JSON")?;

    let (socket, _) = tokio_tungstenite::connect_async(&endpoint)
        .await
        .with_context(|| format!("connecting to Freenet node at {endpoint}"))?;
    let mut client = WebApi::start(socket);
    client
        .send(ClientRequest::ContractOp(ContractRequest::Update {
            key,
            data: UpdateData::Delta(StateDelta::from(delta)),
        }))
        .await
        .context("sending contract update")?;

    let response = tokio::time::timeout(std::time::Duration::from_secs(120), async {
        loop {
            match client.recv().await? {
                HostResponse::ContractResponse(ContractResponse::UpdateResponse {
                    key: response_key,
                    ..
                }) if response_key == key => return Ok::<(), anyhow::Error>(()),
                HostResponse::ContractResponse(ContractResponse::UpdateNotification { .. }) => {}
                other => bail!("unexpected response while awaiting update: {other}"),
            }
        }
    })
    .await
    .context("timed out waiting for Freenet update response")?;
    response.context("Freenet node rejected the update request")?;
    println!("Freenet accepted the update request for contract {key}");
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Setup {
            secret,
            output,
            proving_key,
        } => setup(secret, output, proving_key).await,
        Command::Prove {
            secret,
            parameters,
            proving_key,
            counter,
            text,
            output,
        } => prove(secret, parameters, proving_key, counter, text, output).await,
        Command::Verify {
            parameters,
            candidate,
        } => verify(parameters, candidate),
        Command::Submit {
            endpoint,
            contract_code,
            parameters,
            candidate,
        } => submit(endpoint, contract_code, parameters, candidate).await,
    }
}
