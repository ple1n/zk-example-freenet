use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use leo_ast::{NetworkName, NodeBuilder};
use leo_compiler::Compiler;
use leo_errors::Handler;
use leo_span::create_session_if_not_set_then;
use snarkvm::{
    algorithms::snark::varuna::VarunaVersion,
    circuit::network::AleoV0,
    console::network::{MainnetV0, Network},
    ledger::block::Output,
    ledger::{
        block::Execution,
        query::Query,
        store::{BlockStore, helpers::memory::BlockMemory},
    },
    prelude::{
        Field, Identifier, InclusionVersion, PrivateKey, Process, Program, ProgramID, TestRng,
        Value, VerifyingKey,
    },
    utilities::{FromBytes, ToBytes},
};
use std::{
    fs,
    path::{Path, PathBuf},
    rc::Rc,
    str::FromStr,
};
use zk_notepad_shared::process::measure;

const PROGRAM_NAME: &str = "hash_equivalence.aleo";
const FUNCTION_NAME: &str = "prove_equivalence";

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Setup {
        #[arg(long)]
        wrong_program: bool,
    },
    Prove {
        secret: String,
        #[arg(long)]
        wrong_program: bool,
        #[arg(long)]
        keccak_pubkey: String,
        #[arg(long)]
        poseidon_pubkey: String,
    },
    Verify {
        #[arg(long)]
        keccak_pubkey: String,
        #[arg(long)]
        poseidon_pubkey: String,
        #[arg(long)]
        simulate_wrong_key: bool,
        #[arg(long, default_value = "execution-proof.bin")]
        proof: PathBuf,
    },
}

fn main() -> Result<()> {
    let command = Cli::parse().command;
    let label = match &command {
        Command::Setup {
            wrong_program: false,
        } => "snarkVM verifier-key setup",
        Command::Setup {
            wrong_program: true,
        } => "snarkVM adversary-key setup",
        Command::Prove {
            wrong_program: false,
            ..
        } => "snarkVM Keccak/Poseidon prove",
        Command::Prove {
            wrong_program: true,
            ..
        } => "snarkVM wrong-program prove attempt",
        Command::Verify {
            simulate_wrong_key: true,
            ..
        } => "snarkVM wrong-key rejection",
        Command::Verify { .. } => "snarkVM Keccak/Poseidon verify",
    };
    measure(label, || {
        create_session_if_not_set_then(|_| match command {
            Command::Setup { wrong_program } => setup(wrong_program),
            Command::Prove {
                secret,
                wrong_program,
                keccak_pubkey,
                poseidon_pubkey,
            } => prove(&secret, wrong_program, &keccak_pubkey, &poseidon_pubkey),
            Command::Verify {
                keccak_pubkey,
                poseidon_pubkey,
                simulate_wrong_key,
                proof,
            } => verify(&keccak_pubkey, &poseidon_pubkey, simulate_wrong_key, &proof),
        })
    })
}

fn setup_process(
    rng: &mut TestRng,
    synthesize_key: bool,
    wrong_program: bool,
) -> Result<(
    Process<MainnetV0>,
    String,
    ProgramID<MainnetV0>,
    Identifier<MainnetV0>,
)> {
    let bytecode = compile_program(wrong_program)?;
    let program = Program::<MainnetV0>::from_str(&bytecode)
        .context("parsing compiler output as an Aleo program")?;
    let process = Process::<MainnetV0>::load().context("loading in-memory snarkVM process")?;
    process
        .lock()
        .add_program(&program)
        .context("adding compiled Leo program")?;
    let program_id = *program.id();
    let function = Identifier::<MainnetV0>::from_str(FUNCTION_NAME)?;
    if synthesize_key {
        process
            .synthesize_key::<AleoV0, _>(&program_id, &function, rng)
            .context("synthesizing execution proving/verifying keys")?;
    }
    Ok((process, bytecode, program_id, function))
}

fn compile_program(wrong_program: bool) -> Result<String> {
    let source_file = if wrong_program {
        "wrong_hash_equivalence.leo"
    } else {
        "hash_equivalence.leo"
    };
    let source_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(source_file);
    let handler = Handler::default();
    let mut compiler = Compiler::new(
        Some(PROGRAM_NAME.to_owned()),
        false,
        handler,
        Rc::new(NodeBuilder::default()),
        None,
        indexmap::IndexMap::new(),
        NetworkName::MainnetV0,
    );
    let compiled = compiler
        .compile_from_file(&source_path)
        .with_context(|| format!("compiling Leo source at {}", source_path.display()))?;
    Ok(compiled.primary.bytecode)
}

fn verifier_process(program: &Program<MainnetV0>) -> Result<Process<MainnetV0>> {
    let process = Process::<MainnetV0>::load().context("loading in-memory snarkVM process")?;
    process
        .lock()
        .add_program(program)
        .context("adding compiled Leo program")?;
    Ok(process)
}

fn artifact_path(filename: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(filename)
}

fn verifying_key_path(wrong_program: bool) -> PathBuf {
    let filename = if wrong_program {
        "wrong-verifying-key.bin"
    } else {
        "verifying-key.bin"
    };
    artifact_path(filename)
}

fn program_path() -> PathBuf {
    artifact_path("hash_equivalence.aleo")
}

fn proof_path(wrong_program: bool) -> std::path::PathBuf {
    let filename = if wrong_program {
        "wrong-execution-proof.bin"
    } else {
        "execution-proof.bin"
    };
    Path::new(env!("CARGO_MANIFEST_DIR")).join(filename)
}

fn append_field_values(
    plaintext: &snarkvm::prelude::Plaintext<MainnetV0>,
    fields: &mut Vec<Field<MainnetV0>>,
) {
    use snarkvm::prelude::{Literal, Plaintext};

    match plaintext {
        Plaintext::Literal(Literal::Field(value), _) => fields.push(*value),
        Plaintext::Struct(values, _) => {
            for value in values.values() {
                append_field_values(value, fields);
            }
        }
        Plaintext::Array(values, _) => {
            for value in values {
                append_field_values(value, fields);
            }
        }
        Plaintext::Literal(_, _) => {}
    }
}

fn setup(wrong_program: bool) -> Result<()> {
    let mut rng = TestRng::default();
    let (process, bytecode, program_id, function) = setup_process(&mut rng, true, wrong_program)?;
    let verifying_key = process
        .get_verifying_key(program_id, function)
        .context("loading synthesized execution verifying key")?;
    let bytes = verifying_key
        .to_bytes_le()
        .context("serializing execution verifying key")?;
    fs::write(verifying_key_path(wrong_program), bytes)
        .context("writing verifying-key artifact")?;
    if !wrong_program {
        fs::write(program_path(), bytecode).context("writing compiled Aleo program")?;
        println!(
            "Wrote compiled verifier program: {}",
            program_path().display()
        );
    }
    println!(
        "Wrote {} verifier key: {}",
        if wrong_program {
            "wrong-program"
        } else {
            "trusted"
        },
        verifying_key_path(wrong_program).display()
    );
    Ok(())
}

fn prove(
    secret: &str,
    wrong_program: bool,
    expected_keccak: &str,
    expected_poseidon: &str,
) -> Result<()> {
    let secret = Field::<MainnetV0>::from_str(secret).context("parsing secret field")?;
    let expected_keccak = Field::<MainnetV0>::from_str(expected_keccak)
        .context("parsing expected Keccak public key")?;
    let expected_poseidon = Field::<MainnetV0>::from_str(expected_poseidon)
        .context("parsing expected Poseidon public key")?;
    let inputs = [
        Value::<MainnetV0>::from_str(&secret.to_string())?,
        Value::<MainnetV0>::from_str(&expected_keccak.to_string())?,
        Value::<MainnetV0>::from_str(&expected_poseidon.to_string())?,
    ];
    let mut rng = TestRng::default();
    let (process, _, program_id, function) = setup_process(&mut rng, true, wrong_program)?;
    let private_key = PrivateKey::<MainnetV0>::new(&mut rng).context("creating demo caller key")?;
    let authorization = process
        .authorize::<AleoV0, _>(&private_key, program_id, function, inputs.iter(), &mut rng)
        .context("authorizing Leo function call")?;
    let (response, mut trace) = process
        .execute::<AleoV0, _>(authorization, &mut rng)
        .context("executing compiled Leo program")?;

    let block_store = BlockStore::<MainnetV0, BlockMemory<_>>::open(0u16)
        .context("opening in-memory block store")?;
    trace
        .prepare(&Query::from(&block_store))
        .context("preparing execution trace")?;
    let execution = trace
        .prove_execution::<AleoV0, _>(
            &format!("{PROGRAM_NAME}/{FUNCTION_NAME}"),
            VarunaVersion::V2,
            &mut rng,
        )
        .context("proving Leo execution")?;
    let serialized_execution = execution
        .to_bytes_le()
        .context("serializing snarkVM execution proof")?;
    fs::write(proof_path(wrong_program), &serialized_execution)
        .context("writing serialized execution proof")?;
    ensure!(
        response.outputs().len() == 2,
        "expected two public hash outputs"
    );
    println!("Prover public Keccak-256 key: {}", response.outputs()[0]);
    println!("Prover public Poseidon key: {}", response.outputs()[1]);
    println!(
        "Serialized proof: {} ({} bytes)",
        proof_path(wrong_program).display(),
        serialized_execution.len()
    );
    Ok(())
}

fn verify(
    keccak_pubkey: &str,
    poseidon_pubkey: &str,
    simulate_wrong_key: bool,
    proof: &Path,
) -> Result<()> {
    let expected_keccak = Field::<MainnetV0>::from_str(keccak_pubkey)
        .context("parsing expected Keccak public key")?;
    let expected_poseidon = Field::<MainnetV0>::from_str(poseidon_pubkey)
        .context("parsing expected Poseidon public key")?;
    let bytecode = fs::read_to_string(program_path()).with_context(|| {
        format!(
            "reading compiled verifier program at {}",
            program_path().display()
        )
    })?;
    let program =
        Program::<MainnetV0>::from_str(&bytecode).context("parsing compiled verifier program")?;
    let expected_program_id = *program.id();
    ensure!(
        expected_program_id.to_string() == PROGRAM_NAME,
        "compiled verifier program has unexpected ID {expected_program_id}"
    );
    let process = verifier_process(&program)?;
    let expected_function = Identifier::<MainnetV0>::from_str(FUNCTION_NAME)?;
    let key_bytes = fs::read(verifying_key_path(simulate_wrong_key))
        .context("reading verifier key artifact")?;
    let mut key_reader = key_bytes.as_slice();
    let verifying_key = VerifyingKey::<MainnetV0>::read_le(&mut key_reader)
        .context("deserializing trusted verifying key")?;
    ensure!(
        key_reader.is_empty(),
        "verifying-key artifact contains trailing bytes"
    );
    // Verification under this key proves execution of its circuit; trust that the key belongs to this program.
    process
        .insert_verifying_key(&expected_program_id, &expected_function, verifying_key)
        .context("installing trusted verifying key")?;
    
    let proof_path = if proof.is_absolute() {
        proof.to_path_buf()
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(proof)
    };
    let serialized_execution = fs::read(&proof_path)
        .with_context(|| format!("reading proof at {}", proof_path.display()))?;
    let mut proof_reader = serialized_execution.as_slice();
    let execution = Execution::<MainnetV0>::read_le(&mut proof_reader)
        .context("deserializing snarkVM execution proof")?;
    ensure!(
        proof_reader.is_empty(),
        "serialized proof contains trailing bytes"
    );
    ensure!(
        execution.len() == 1,
        "proof must contain exactly one transition for the expected circuit"
    );
    let transition = execution.get(0)?;
    ensure!(
        transition.program_id() == &expected_program_id
            && transition.function_name() == &expected_function,
        "proof is not for the verifier's expected Keccak/Poseidon circuit"
    );

    let stacks = process
        .get_stacks(execution.transitions(), true)
        .context("loading execution stacks for verification")?;
    Process::<MainnetV0>::verify_execution(
        MainnetV0::CONSENSUS_VERSION(0)?,
        VarunaVersion::V2,
        InclusionVersion::V0,
        &execution,
        &stacks,
    )
    .context("verifying Leo execution proof")?;

    let mut public_outputs = Vec::new();
    for output in transition.outputs() {
        if let Output::Public(_, Some(plaintext)) | Output::Constant(_, Some(plaintext)) = output {
            append_field_values(plaintext, &mut public_outputs);
        }
    }
    ensure!(
        public_outputs.len() == 2,
        "proof must contain both public hash outputs; transition outputs: {}",
        transition
            .outputs()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    );
    ensure!(
        public_outputs[0] == expected_keccak,
        "proof's Keccak-256 public key does not match the verifier's expected key"
    );
    ensure!(
        public_outputs[1] == expected_poseidon,
        "proof's Poseidon public key does not match the verifier's expected key"
    );
    println!("Execution proof verified against verifier-supplied public keys.");
    println!("Keccak-256 public key: {expected_keccak}");
    println!("Poseidon public key: {expected_poseidon}");
    println!("Both expected keys match the proof's public outputs; trust established.");
    Ok(())
}
