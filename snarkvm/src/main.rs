use anyhow::{Context, Result, ensure};
use leo_ast::{NetworkName, NodeBuilder};
use leo_compiler::Compiler;
use leo_errors::Handler;
use leo_span::create_session_if_not_set_then;
use snarkvm::{
    algorithms::snark::varuna::VarunaVersion,
    circuit::network::AleoV0,
    console::network::{MainnetV0, Network},
    ledger::{
        query::Query,
        store::{BlockStore, helpers::memory::BlockMemory},
    },
    prelude::{Field, Identifier, InclusionVersion, PrivateKey, Process, Program, TestRng, Value},
};
use std::{path::Path, rc::Rc, str::FromStr};

const PROGRAM_NAME: &str = "leo_demo.aleo";
const FUNCTION_NAME: &str = "prove_preimage";

fn main() -> Result<()> {
    create_session_if_not_set_then(|_| run())
}

fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let preimage = args
        .next()
        .context("usage: snarkvm-example <preimage.field> [expected-hash.field]")?;
    let expected_hash = args.next();
    ensure!(
        args.next().is_none(),
        "expected a preimage and at most one expected hash"
    );

    let source_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/hash_demo.leo");
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
    let program = Program::<MainnetV0>::from_str(&compiled.primary.bytecode)
        .context("parsing compiler output as an Aleo program")?;

    let mut rng = TestRng::default();
    let preimage = Field::<MainnetV0>::from_str(&preimage).context("parsing preimage field")?;
    let expected_hash = match expected_hash {
        Some(expected_hash) => Field::<MainnetV0>::from_str(&expected_hash)
            .context("parsing expected public hash field")?,
        None => MainnetV0::hash_psd2(std::slice::from_ref(&preimage))
            .context("computing demo expected hash")?,
    };
    let inputs = [
        Value::<MainnetV0>::from_str(&preimage.to_string())?,
        Value::<MainnetV0>::from_str(&expected_hash.to_string())?,
    ];

    let process = Process::<MainnetV0>::load().context("loading in-memory snarkVM process")?;
    process
        .lock()
        .add_program(&program)
        .context("adding compiled Leo program")?;
    let program_id = program.id();
    let function = Identifier::<MainnetV0>::from_str(FUNCTION_NAME)?;
    process
        .synthesize_key::<AleoV0, _>(program_id, &function, &mut rng)
        .context("synthesizing execution proving/verifying keys")?;

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

    ensure!(
        !response.outputs().is_empty(),
        "Leo function returned no outputs"
    );
    println!("Compiled {PROGRAM_NAME} from Leo source with the embedded Rust compiler.");
    println!("Public expected hash: {expected_hash}");
    println!("Execution output: {}", response.outputs()[0]);
    println!("snarkVM verified the private preimage against the public hash.");
    Ok(())
}
