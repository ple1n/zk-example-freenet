use std::{fs, path::PathBuf, time::Instant};

use anyhow::{bail, Context};
use clap::{Parser, Subcommand};
use serde::Serialize;
use sp1_host_common::{
    count_guest_cycles, load_or_setup_proving_key, monitor_async, read_proving_key, submit_update,
    verify_compressed_proof,
};
use sp1_merkle_shared::poseidon2_bytes;
use sp1_sdk::{
    include_elf, Elf, HashableKey, ProveRequest, Prover, ProverClient, ProvingKey, SP1Proof,
    SP1Stdin,
};
use zk_notepad_shared::{process::measure_async, ContractParameters, NoteCandidate, ProofValues};

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
        #[arg(long, default_value = "secret.bin")]
        secret: PathBuf,
        #[arg(long, default_value = "sp1-contract-parameters.json")]
        output: PathBuf,
        #[arg(long, default_value = "sp1-proving-key.bin")]
        proving_key: PathBuf,
    },
    Prove {
        #[arg(long, default_value = "secret.bin")]
        secret: PathBuf,
        #[arg(long, default_value = "sp1-contract-parameters.json")]
        parameters: PathBuf,
        #[arg(long, default_value = "sp1-proving-key.bin")]
        proving_key: PathBuf,
        #[arg(long)]
        counter: u64,
        #[arg(long)]
        text: String,
        #[arg(long, default_value = "sp1-candidate.json")]
        output: PathBuf,
        #[arg(long, help = "Run the guest once to report its SP1 cycle count")]
        count_cycles: bool,
    },
    Verify {
        #[arg(long, default_value = "sp1-contract-parameters.json")]
        parameters: PathBuf,
        #[arg(long, default_value = "sp1-candidate.json")]
        candidate: PathBuf,
    },
    Clear {
        #[arg(long, default_value = "secret.bin")]
        secret: PathBuf,
        #[arg(long, default_value = "sp1-contract-parameters.json")]
        parameters: PathBuf,
        #[arg(long, default_value = "sp1-proving-key.bin")]
        proving_key: PathBuf,
        #[arg(long, default_value = "sp1-candidate.json")]
        candidate: PathBuf,
    },
    Submit {
        #[arg(long)]
        endpoint: String,
        #[arg(
            long,
            default_value = "sp1/target/wasm32-unknown-unknown/release/sp1_container.wasm"
        )]
        contract_code: PathBuf,
        #[arg(long, default_value = "sp1-contract-parameters.json")]
        parameters: PathBuf,
        #[arg(long, default_value = "sp1-candidate.json")]
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

fn clear(paths: [PathBuf; 4]) -> anyhow::Result<()> {
    for path in paths {
        match fs::remove_file(&path) {
            Ok(()) => println!("Removed {}", path.display()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("removing {}", path.display()));
            }
        }
    }
    Ok(())
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
    let result = verify_compressed_proof(proof, public_values, verifying_key_hash);
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

async fn setup(
    secret_path: PathBuf,
    output: PathBuf,
    proving_key_path: PathBuf,
) -> anyhow::Result<()> {
    let command_started = Instant::now();
    let secret = read_secret(&secret_path)?;
    let public_y = poseidon2_bytes(&secret);
    let (proving_key, cache_hit, key_duration) =
        load_or_setup_proving_key(&GUEST_ELF, &proving_key_path).await?;
    if cache_hit {
        println!(
            "Proving-key cache load took {:.3}s",
            key_duration.as_secs_f64()
        );
    } else {
        println!(
            "Proving-key setup and cache write took {:.3}s",
            key_duration.as_secs_f64()
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
    count_cycles: bool,
) -> anyhow::Result<()> {
    let command_started = Instant::now();
    let secret = read_secret(&secret_path)?;
    let parameters: ContractParameters = serde_json::from_slice(
        &fs::read(&parameters_path)
            .with_context(|| format!("reading {}", parameters_path.display()))?,
    )
    .with_context(|| format!("parsing {}", parameters_path.display()))?;
    let public_y = decode_public_y(&parameters.public_y)?;
    if poseidon2_bytes(&secret) != public_y {
        bail!("secret does not match the public_y in the contract parameters");
    }
    let text_hash = poseidon2_bytes(text.as_bytes());
    let mut stdin = SP1Stdin::new();
    stdin.write(&secret);
    stdin.write(&public_y);
    stdin.write(&counter);
    stdin.write(&text_hash);

    let key_started = Instant::now();
    let proving_key = read_proving_key(&proving_key_path, &GUEST_ELF)?;
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
    if count_cycles {
        let cycles = count_guest_cycles(&GUEST_ELF, &stdin).await?;
        println!("SP1 guest cycle count: {cycles}");
    }
    let prove_started = Instant::now();
    let proof = monitor_async("SP1 compressed proof generation", async {
        client
            .prove(&proving_key, stdin)
            .compressed()
            .await
            .context("generating SP1 compressed proof")
    })
    .await;
    println!(
        "Proof generation took {:.3}s",
        prove_started.elapsed().as_secs_f64()
    );
    let proof = proof?;
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
        text_hash: poseidon2_bytes(candidate.text.as_bytes()),
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
    let delta = fs::read(&candidate_path)
        .with_context(|| format!("reading candidate {}", candidate_path.display()))?;
    let _: NoteCandidate = serde_json::from_slice(&delta).context("parsing candidate JSON")?;
    submit_update(
        &endpoint,
        &contract_code_path,
        &parameters_path,
        &candidate_path,
    )
    .await
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
            count_cycles,
        } => {
            prove(
                secret,
                parameters,
                proving_key,
                counter,
                text,
                output,
                count_cycles,
            )
            .await
        }
        Command::Verify {
            parameters,
            candidate,
        } => verify(parameters, candidate),
        Command::Clear {
            secret,
            parameters,
            proving_key,
            candidate,
        } => {
            measure_async("SP1 clear", async {
                clear([secret, parameters, proving_key, candidate])
            })
            .await
        }
        Command::Submit {
            endpoint,
            contract_code,
            parameters,
            candidate,
        } => {
            measure_async("SP1 submit", async {
                submit(endpoint, contract_code, parameters, candidate).await
            })
            .await
        }
    }
}
