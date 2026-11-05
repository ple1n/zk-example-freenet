use std::{fs, path::PathBuf, time::Instant};

use anyhow::{bail, ensure, Context, Result};
use clap::{Parser, Subcommand};
use sp1_host_common::{
    load_or_setup_proving_key, monitor_async, read_proving_key, verify_compressed_proof,
};
use sp1_merkle_shared::poseidon2_bytes;
use sp1_sdk::{
    include_elf, HashableKey, ProveRequest, Prover, ProverClient, ProvingKey, SP1Proof, SP1Stdin,
};
use tiny_keccak::{Hasher, Keccak};

const GUEST_ELF: sp1_sdk::Elf = include_elf!("sp1-keccak-guest");

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Setup,
    Prove,
    Verify,
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Setup => setup().await,
        Command::Prove => prove().await,
        Command::Verify => verify(),
    }
}

fn artifact(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(name)
}

fn message() -> [u8; 32] {
    [0x42; 32]
}

fn keccak256(message: &[u8]) -> [u8; 32] {
    let mut digest = [0u8; 32];
    let mut hasher = Keccak::v256();
    hasher.update(message);
    hasher.finalize(&mut digest);
    digest
}

fn public_keys(secret: &[u8; 32]) -> ([u8; 32], [u8; 32]) {
    (keccak256(secret), poseidon2_bytes(secret))
}

async fn setup() -> Result<()> {
    let proving_key_path = artifact("proving-key.bin");
    let (proving_key, cache_hit, setup_time) =
        load_or_setup_proving_key(&GUEST_ELF, &proving_key_path).await?;
    println!(
        "SP1 proving-key {} took {:.3}s",
        if cache_hit { "load" } else { "setup" },
        setup_time.as_secs_f64()
    );
    let verifying_key_hash = bincode::serialize(&proving_key.verifying_key().hash_koalabear())?;
    fs::write(artifact("verifying-key-hash.bin"), verifying_key_hash)
        .context("writing verifying-key hash")?;
    Ok(())
}

async fn prove() -> Result<()> {
    let message = message();
    let (expected_keccak, expected_poseidon2) = public_keys(&message);
    let mut stdin = SP1Stdin::new();
    stdin.write(&message);
    stdin.write(&expected_keccak);
    stdin.write(&expected_poseidon2);

    let proving_key_path = artifact("proving-key.bin");
    let proving_key = read_proving_key(&proving_key_path, &GUEST_ELF)?;

    let client = ProverClient::builder().cpu().build().await;
    let prove_started = Instant::now();
    let proof = monitor_async("SP1 compressed proof generation", async {
        client
            .prove(&proving_key, stdin)
            .compressed()
            .await
            .context("generating SP1 compressed proof")
    })
    .await?;
    println!(
        "Proof generation took {:.3}s",
        prove_started.elapsed().as_secs_f64()
    );

    if !matches!(&proof.proof, SP1Proof::Compressed(_)) {
        bail!("SP1 prover returned a proof that is not compressed");
    }
    let mut expected_public_values = expected_keccak.to_vec();
    expected_public_values.extend_from_slice(&expected_poseidon2);
    if proof.public_values.as_slice() != expected_public_values {
        bail!("SP1 guest public keys differ from the native hash results");
    }

    let proof_bytes = bincode::serialize(&proof.proof).context("serializing SP1 proof")?;
    fs::write(artifact("proof.bin"), &proof_bytes).context("writing compressed proof")?;
    fs::write(
        artifact("public-values.bin"),
        proof.public_values.as_slice(),
    )
    .context("writing proof public values")?;
    println!("Compressed proof size: {} bytes", proof_bytes.len());
    Ok(())
}

fn verify() -> Result<()> {
    let (expected_keccak, expected_poseidon2) = public_keys(&message());
    let mut expected_public_values = expected_keccak.to_vec();
    expected_public_values.extend_from_slice(&expected_poseidon2);
    let proof = fs::read(artifact("proof.bin")).context("reading compressed proof")?;
    let public_values =
        fs::read(artifact("public-values.bin")).context("reading proof public values")?;
    let verifying_key_hash =
        fs::read(artifact("verifying-key-hash.bin")).context("reading verifying-key hash")?;
    ensure!(
        public_values.as_slice() == expected_public_values,
        "proof public keys do not match the Keccak-256 and SP1 Poseidon2 results"
    );
    let verify_started = Instant::now();
    verify_compressed_proof(&proof, &public_values, &verifying_key_hash)
        .context("locally verifying generated proof")?;
    println!(
        "Proof verification took {:.3}s",
        verify_started.elapsed().as_secs_f64()
    );
    println!("SP1 verified Keccak-256 and Poseidon2 public keys for the same secret.");
    Ok(())
}
