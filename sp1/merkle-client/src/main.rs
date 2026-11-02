use std::{fs, path::PathBuf, time::Instant};

use anyhow::{bail, Context};
use clap::{Parser, Subcommand};
use serde::Serialize;
use sp1_host_common::{
    count_guest_cycles, load_or_setup_proving_key, monitor_async, monitor_sync, read_proving_key,
    submit_update, verify_compressed_proof,
};
use sp1_merkle_shared::{
    build_tree, membership_witness, MerkleCandidate, MerkleContractParameters, MerkleProofValues,
    MAX_TREE_DEPTH,
};
use sp1_sdk::{
    include_elf, Elf, HashableKey, ProveRequest, Prover, ProverClient, ProvingKey, SP1Proof,
    SP1Stdin,
};

const GUEST_ELF: Elf = include_elf!("sp1-merkle-guest");

#[derive(Parser)]
#[command(about = "Prove private Poseidon2 Merkle membership for a public root")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Setup {
        #[arg(long, default_value = "sp1-merkle-tree-secret.bin")]
        tree_secret: PathBuf,
        #[arg(long, default_value_t = 16)]
        depth: u32,
        #[arg(long, default_value = "sp1-merkle-contract-parameters.json")]
        output: PathBuf,
        #[arg(long, default_value = "sp1-merkle-proving-key.bin")]
        proving_key: PathBuf,
    },
    Prove {
        #[arg(long, default_value = "sp1-merkle-tree-secret.bin")]
        tree_secret: PathBuf,
        #[arg(long, default_value = "sp1-merkle-contract-parameters.json")]
        parameters: PathBuf,
        #[arg(long, default_value = "sp1-merkle-proving-key.bin")]
        proving_key: PathBuf,
        #[arg(long)]
        leaf_index: u32,
        #[arg(long, default_value = "sp1-merkle-candidate.json")]
        output: PathBuf,
        #[arg(long, help = "Run the guest once to report its SP1 cycle count")]
        count_cycles: bool,
    },
    Verify {
        #[arg(long, default_value = "sp1-merkle-contract-parameters.json")]
        parameters: PathBuf,
        #[arg(long, default_value = "sp1-merkle-candidate.json")]
        candidate: PathBuf,
    },
    Submit {
        #[arg(long)]
        endpoint: String,
        #[arg(
            long,
            default_value = "sp1/target/wasm32-unknown-unknown/release/sp1_merkle_container.wasm"
        )]
        contract_code: PathBuf,
        #[arg(long, default_value = "sp1-merkle-contract-parameters.json")]
        parameters: PathBuf,
        #[arg(long, default_value = "sp1-merkle-candidate.json")]
        candidate: PathBuf,
    },
}

fn read_tree_secret(path: &PathBuf) -> anyhow::Result<[u8; 32]> {
    let bytes =
        fs::read(path).with_context(|| format!("reading tree secret {}", path.display()))?;
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        anyhow::anyhow!("tree secret must be exactly 32 bytes, got {}", bytes.len())
    })
}

fn write_json(path: &PathBuf, value: &impl Serialize) -> anyhow::Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes).with_context(|| format!("writing {}", path.display()))
}

fn read_parameters(path: &PathBuf) -> anyhow::Result<MerkleContractParameters> {
    serde_json::from_slice(&fs::read(path).with_context(|| format!("reading {}", path.display()))?)
        .with_context(|| format!("parsing {}", path.display()))
}

fn verifying_key_hash(parameters: &MerkleContractParameters) -> anyhow::Result<Vec<u8>> {
    let bytes =
        hex::decode(&parameters.verifying_key).context("decoding SP1 verifying-key hash")?;
    if bytes.is_empty() {
        bail!("SP1 verifying-key hash cannot be empty");
    }
    Ok(bytes)
}

async fn setup(
    tree_secret_path: PathBuf,
    tree_depth: u32,
    output: PathBuf,
    proving_key_path: PathBuf,
) -> anyhow::Result<()> {
    let command_started = Instant::now();
    let tree_secret = read_tree_secret(&tree_secret_path)?;
    let tree_started = Instant::now();
    let levels = monitor_sync("Poseidon2 tree construction", || {
        build_tree(&tree_secret, tree_depth)
            .ok_or_else(|| anyhow::anyhow!("depth must be in 1..={MAX_TREE_DEPTH}"))
    })?;
    let root = levels[tree_depth as usize][0];
    println!(
        "Poseidon2 tree construction took {:.3}s",
        tree_started.elapsed().as_secs_f64()
    );

    let (proving_key, cache_hit, elapsed) =
        load_or_setup_proving_key(&GUEST_ELF, &proving_key_path).await?;
    if cache_hit {
        println!("Proving-key cache load took {:.3}s", elapsed.as_secs_f64());
    } else {
        println!(
            "Proving-key setup and cache write took {:.3}s",
            elapsed.as_secs_f64()
        );
    }
    let verifying_key = bincode::serialize(&proving_key.verifying_key().hash_koalabear())?;
    let parameters = MerkleContractParameters {
        tree_depth,
        root,
        verifying_key: hex::encode(verifying_key),
    };
    write_json(&output, &parameters)?;
    println!("Wrote public contract parameters to {}", output.display());
    println!(
        "Wrote reusable proving key to {}",
        proving_key_path.display()
    );
    println!("Public Merkle root: {root:?}");
    println!(
        "Setup command took {:.3}s",
        command_started.elapsed().as_secs_f64()
    );
    Ok(())
}

async fn prove(
    tree_secret_path: PathBuf,
    parameters_path: PathBuf,
    proving_key_path: PathBuf,
    leaf_index: u32,
    output: PathBuf,
    count_cycles: bool,
) -> anyhow::Result<()> {
    let command_started = Instant::now();
    let tree_secret = read_tree_secret(&tree_secret_path)?;
    let parameters = read_parameters(&parameters_path)?;
    if parameters.tree_depth == 0 || parameters.tree_depth > MAX_TREE_DEPTH {
        bail!("tree_depth is outside the supported range");
    }
    if leaf_index >= (1u32 << parameters.tree_depth) {
        bail!("leaf_index is outside the configured tree");
    }
    let (leaf, siblings, root) =
        membership_witness(&tree_secret, parameters.tree_depth, leaf_index)
            .ok_or_else(|| anyhow::anyhow!("cannot construct membership witness"))?;
    if root != parameters.root {
        bail!("tree secret does not match the public root in contract parameters");
    }
    let expected_key_hash = verifying_key_hash(&parameters)?;
    let key_started = Instant::now();
    let proving_key = read_proving_key(&proving_key_path, &GUEST_ELF)?;
    println!(
        "Proving-key load took {:.3}s",
        key_started.elapsed().as_secs_f64()
    );
    let actual_key_hash = bincode::serialize(&proving_key.verifying_key().hash_koalabear())?;
    if actual_key_hash != expected_key_hash {
        bail!(
            "parameters do not match proving key {}; use matching contract parameters",
            proving_key_path.display()
        );
    }

    let mut stdin = SP1Stdin::new();
    stdin.write(&parameters.tree_depth);
    stdin.write(&leaf_index);
    stdin.write(&leaf);
    stdin.write(&siblings);
    stdin.write(&parameters.root);
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
            .context("generating SP1 compressed Merkle membership proof")
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
    let public_values = bincode::serialize(&MerkleProofValues {
        tree_depth: parameters.tree_depth,
        root: parameters.root,
    })?;
    if proof.public_values.as_slice() != public_values {
        bail!("SP1 guest public values differ from the expected Merkle root");
    }
    let verify_started = Instant::now();
    verify_compressed_proof(&proof_bytes, &public_values, &actual_key_hash)
        .context("locally verifying generated proof")?;
    println!(
        "Proof verification took {:.3}s",
        verify_started.elapsed().as_secs_f64()
    );
    let candidate = MerkleCandidate {
        proof: hex::encode(&proof_bytes),
    };
    write_json(&output, &candidate)?;
    println!("Serialized proof size: {} bytes", proof_bytes.len());
    println!("Wrote verified candidate to {}", output.display());
    println!(
        "Prove command took {:.3}s",
        command_started.elapsed().as_secs_f64()
    );
    Ok(())
}

fn verify(parameters_path: PathBuf, candidate_path: PathBuf) -> anyhow::Result<()> {
    let command_started = Instant::now();
    let parameters = read_parameters(&parameters_path)?;
    if parameters.tree_depth == 0 || parameters.tree_depth > MAX_TREE_DEPTH {
        bail!("tree_depth is outside the supported range");
    }
    let candidate: MerkleCandidate = serde_json::from_slice(
        &fs::read(&candidate_path)
            .with_context(|| format!("reading {}", candidate_path.display()))?,
    )
    .with_context(|| format!("parsing {}", candidate_path.display()))?;
    let proof = hex::decode(&candidate.proof).context("decoding candidate proof")?;
    let public_values = bincode::serialize(&MerkleProofValues {
        tree_depth: parameters.tree_depth,
        root: parameters.root,
    })?;
    let started = Instant::now();
    verify_compressed_proof(&proof, &public_values, &verifying_key_hash(&parameters)?)
        .context("candidate proof is invalid")?;
    println!(
        "Proof verification took {:.3}s",
        started.elapsed().as_secs_f64()
    );
    println!("Candidate proof is valid.");
    println!(
        "Verify command took {:.3}s",
        command_started.elapsed().as_secs_f64()
    );
    Ok(())
}

async fn submit(
    endpoint: String,
    contract_code: PathBuf,
    parameters: PathBuf,
    candidate: PathBuf,
) -> anyhow::Result<()> {
    submit_update(&endpoint, &contract_code, &parameters, &candidate).await
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Setup {
            tree_secret,
            depth,
            output,
            proving_key,
        } => setup(tree_secret, depth, output, proving_key).await,
        Command::Prove {
            tree_secret,
            parameters,
            proving_key,
            leaf_index,
            output,
            count_cycles,
        } => {
            prove(
                tree_secret,
                parameters,
                proving_key,
                leaf_index,
                output,
                count_cycles,
            )
            .await
        }
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
