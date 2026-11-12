use std::{
    fs,
    path::PathBuf,
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use cairo_air::utils::ProofFormat;
use cairo_lang_compiler::{
    db::RootDatabase, diagnostics::DiagnosticsReporter, project::setup_project,
};
use cairo_lang_executable::{
    compile::{ExecutableConfig, compile_executable_in_prepared_db},
    executable::{EntryPointKind, Executable},
};
use cairo_lang_executable_plugin::executable_plugin_suite;
use cairo_lang_execute_utils::program_and_hints_from_executable;
use cairo_lang_filesystem::{
    cfg::{Cfg, CfgSet},
    db::init_dev_corelib,
    ids::CrateInput,
};
use cairo_program_runner_lib::{
    PRIVACY_SIMPLE_BOOTLOADER_COMPILED_JSON, ProgramInput, SimpleBootloaderInput, Task, TaskSpec,
    cairo_run_program,
    hints::types::{Cairo1Executable, HashFunc, PrivacySimpleBootloaderInput},
};
use cairo_vm::{
    Felt252,
    cairo_run::CairoRunConfig,
    types::{layout_name::LayoutName, program::Program},
};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use stwo_cairo_adapter::adapter::adapt;
use stwo_cairo_protocol::{Candidate, ContractParameters};
use stwo_cairo_prover::prover::create_and_serialize_proof;
use stwo_cairo_verifier::{inspect_candidate, verify_candidate};
use zk_notepad_shared::process::measure;

#[derive(Parser)]
#[command(about = "Run, prove, and verify native-field Cairo experiments with Stwo")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    CompileCairo1 {
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        corelib: PathBuf,
        #[arg(long)]
        program: String,
        #[arg(long)]
        output: PathBuf,
    },
    Run {
        #[arg(long)]
        program: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        prover_input: PathBuf,
    },
    Prove {
        #[arg(long, default_value = "program.json")]
        program: PathBuf,
        #[arg(long, default_value = "input.json")]
        input: PathBuf,
        #[arg(long, default_value = "candidate.json")]
        output: PathBuf,
        #[arg(long, default_value = "stwo-proof.bin")]
        proof: PathBuf,
        #[arg(long)]
        prover_parameters: Option<PathBuf>,
    },
    Setup {
        #[arg(long, default_value = "candidate.json")]
        candidate: PathBuf,
        #[arg(long, default_value = "contract-parameters.json")]
        output: PathBuf,
    },
    Verify {
        #[arg(long, default_value = "candidate.json")]
        candidate: PathBuf,
        #[arg(long, default_value = "contract-parameters.json")]
        parameters: PathBuf,
    },
}

fn compile_cairo1(
    source_path: PathBuf,
    corelib_path: PathBuf,
    program_name: String,
    output_path: PathBuf,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        matches!(program_name.as_str(), "poseidon_hash" | "poseidon_merkle"),
        "unsupported Cairo program name: {program_name}"
    );
    let source_path = source_path
        .canonicalize()
        .with_context(|| format!("resolving Cairo source {}", source_path.display()))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("reading system clock")?
        .as_nanos();
    let build_dir = std::env::temp_dir().join(format!(
        "stwo-cairo-{}-{}-{nonce}",
        std::process::id(),
        program_name
    ));
    fs::create_dir(&build_dir)
        .with_context(|| format!("creating temporary Cairo project {}", build_dir.display()))?;
    let compile_result = (|| {
        let source_dir = build_dir.join("src");
        fs::create_dir(&source_dir)?;
        let source = fs::read_to_string(&source_path)
            .with_context(|| format!("reading Cairo source {}", source_path.display()))?;
        let function_declaration = format!("pub fn {program_name}(");
        anyhow::ensure!(
            source.matches(&function_declaration).count() == 1,
            "expected one public `{program_name}` function in {}",
            source_path.display()
        );
        let source = source.replacen(
            &function_declaration,
            &format!("#[executable]\n{function_declaration}"),
            1,
        );
        fs::write(source_dir.join("lib.cairo"), source)
            .with_context(|| format!("copying Cairo source {}", source_path.display()))?;
        fs::write(
            build_dir.join("cairo_project.toml"),
            "[crate_roots]\nstwo_cairo_compile = \"src\"\n\n[config.global]\nedition = \"2024_07\"\n",
        )
        .with_context(|| format!("writing temporary Cairo project config in {}", build_dir.display()))?;

        let corelib_path = corelib_path
            .canonicalize()
            .with_context(|| format!("resolving Cairo corelib path {}", corelib_path.display()))?;
        anyhow::ensure!(
            corelib_path.join("lib.cairo").is_file(),
            "Cairo corelib path must point to its src directory: {}",
            corelib_path.display()
        );
        let mut db = RootDatabase::builder()
            .skip_auto_withdraw_gas()
            .with_cfg(CfgSet::from_iter([Cfg::kv("gas", "disabled")]))
            .with_default_plugin_suite(executable_plugin_suite())
            .build()
            .context("creating Cairo compiler database")?;
        init_dev_corelib(&mut db, corelib_path);

        let crate_inputs =
            setup_project(&mut db, &build_dir).context("loading temporary Cairo project")?;
        let crate_ids = CrateInput::into_crate_ids(&db, crate_inputs.clone());
        anyhow::ensure!(!crate_ids.is_empty(), "Cairo project has no crate root");
        let compiled = compile_executable_in_prepared_db(
            &db,
            None,
            crate_ids,
            DiagnosticsReporter::stderr().with_crates(&crate_inputs),
            ExecutableConfig::default(),
        )
        .with_context(|| format!("compiling Cairo 1 executable {program_name}"))?;
        Ok::<_, anyhow::Error>(Executable::new(compiled.compiled_function))
    })();
    let cleanup_result = fs::remove_dir_all(&build_dir)
        .with_context(|| format!("removing temporary Cairo project {}", build_dir.display()));
    let executable = compile_result?;
    cleanup_result?;

    write_json(&output_path, &executable)?;
    println!("Wrote Cairo 1 executable to {}", output_path.display());
    Ok(())
}

fn run(
    program_path: PathBuf,
    input_path: PathBuf,
) -> anyhow::Result<stwo_cairo_adapter::ProverInput> {
    let executable: Executable = read_json(&program_path)?;
    let entrypoint = executable
        .entrypoints
        .iter()
        .find(|entrypoint| matches!(entrypoint.kind, EntryPointKind::Bootloader))
        .context("Cairo 1 executable has no bootloader entrypoint")?;
    let (program, string_to_hint) = program_and_hints_from_executable(&executable, entrypoint)
        .context("loading Cairo 1 executable for Cairo VM")?;
    let input: Vec<String> = read_json(&input_path)?;
    let mut input_values = Vec::with_capacity(input.len());
    for (index, value) in input.iter().enumerate() {
        Felt252::from_dec_str(value)
            .with_context(|| format!("input argument {index} is not a valid felt252"))?;
        input_values.push(
            value
                .parse()
                .with_context(|| format!("input argument {index} is not a decimal integer"))?,
        );
    }
    let user_args = cairo_lang_execute_utils::user_args_from_flags(None, &input_values)
        .context("converting input values to Cairo 1 arguments")?;
    let task = Task::Cairo1Program(Cairo1Executable {
        program,
        user_args,
        string_to_hint,
    });
    let task_spec = TaskSpec {
        task: Rc::new(task),
        program_hash_function: HashFunc::Poseidon,
    };
    let bootloader = Program::from_bytes(PRIVACY_SIMPLE_BOOTLOADER_COMPILED_JSON, Some("main"))
        .context("loading Stwo privacy simple bootloader")?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("reading system clock")?
        .as_nanos();
    let output_preimage_dump_path = std::env::temp_dir().join(format!(
        "stwo-cairo-preimage-{}-{nonce}.json",
        std::process::id()
    ));
    let bootloader_input = PrivacySimpleBootloaderInput {
        simple_bootloader_input: SimpleBootloaderInput {
            fact_topologies_path: None,
            single_page: true,
            tasks: vec![task_spec],
        },
        output_preimage_dump_path: output_preimage_dump_path.clone(),
    };
    let config = CairoRunConfig {
        entrypoint: "main",
        trace_enabled: true,
        relocate_mem: false,
        relocate_trace: false,
        layout: LayoutName::all_cairo_stwo,
        proof_mode: true,
        fill_holes: true,
        secure_run: None,
        disable_trace_padding: true,
        allow_missing_builtins: None,
        dynamic_layout_params: None,
    };
    let run_result = measure("Cairo VM execution", || {
        cairo_run_program(
            &bootloader,
            Some(ProgramInput::Value(Box::new(bootloader_input))),
            config,
            None,
        )
        .context("executing Cairo 1 task in proof mode")
    });
    let cleanup_result = if output_preimage_dump_path.try_exists()? {
        fs::remove_file(&output_preimage_dump_path)
            .with_context(|| format!("removing {}", output_preimage_dump_path.display()))
    } else {
        Ok(())
    };
    let runner = run_result?;
    cleanup_result?;
    measure("Stwo input adaptation", || {
        adapt(&runner).context("adapting Cairo execution for Stwo")
    })
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &PathBuf) -> anyhow::Result<T> {
    serde_json::from_slice(&fs::read(path).with_context(|| format!("reading {}", path.display()))?)
        .with_context(|| format!("parsing {}", path.display()))
}

fn write_json(path: &PathBuf, value: &impl Serialize) -> anyhow::Result<()> {
    fs::write(path, serde_json::to_vec_pretty(value)?)
        .with_context(|| format!("writing {}", path.display()))
}

fn prove(
    program_path: PathBuf,
    input_path: PathBuf,
    candidate_path: PathBuf,
    proof_path: PathBuf,
    prover_parameters: Option<PathBuf>,
) -> anyhow::Result<()> {
    let input = run(program_path, input_path)?;
    measure("Stwo proof generation", || {
        create_and_serialize_proof(
            input,
            false,
            proof_path.clone(),
            ProofFormat::Binary,
            prover_parameters,
        )
        .context("creating Stwo proof")
    })?;
    let candidate = Candidate::new(hex::encode(
        fs::read(&proof_path).with_context(|| format!("reading {}", proof_path.display()))?,
    ));
    let output = measure("Stwo proof verification", || {
        inspect_candidate(&candidate).map_err(anyhow::Error::msg)
    })?;
    write_json(&candidate_path, &candidate)?;
    println!("Program hash: {}", output.program_hash);
    println!("Public output: {}", output.public_output.join(", "));
    println!("Wrote proof to {}", proof_path.display());
    println!("Wrote candidate to {}", candidate_path.display());
    Ok(())
}

fn setup(candidate_path: PathBuf, output_path: PathBuf) -> anyhow::Result<()> {
    let candidate: Candidate = read_json(&candidate_path)?;
    let output = measure("Contract parameter setup", || {
        inspect_candidate(&candidate).map_err(anyhow::Error::msg)
    })?;
    let parameters = ContractParameters::new(output.program_hash, output.public_output);
    write_json(&output_path, &parameters)?;
    println!("Wrote contract parameters to {}", output_path.display());
    Ok(())
}

fn verify(candidate_path: PathBuf, parameters_path: PathBuf) -> anyhow::Result<()> {
    let candidate: Candidate = read_json(&candidate_path)?;
    let parameters: ContractParameters = read_json(&parameters_path)?;
    let output = measure("Candidate verification", || {
        verify_candidate(&candidate, &parameters).map_err(anyhow::Error::msg)
    })?;
    println!("Proof is valid.");
    println!("Program hash: {}", output.program_hash);
    println!("Public output: {}", output.public_output.join(", "));
    Ok(())
}

fn run_to_file(program: PathBuf, input: PathBuf, prover_input: PathBuf) -> anyhow::Result<()> {
    let input = run(program, input)?;
    fs::write(&prover_input, serde_json::to_vec(&input)?)
        .with_context(|| format!("writing {}", prover_input.display()))?;
    println!("Wrote Stwo prover input to {}", prover_input.display());
    Ok(())
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::CompileCairo1 {
            source,
            corelib,
            program,
            output,
        } => compile_cairo1(source, corelib, program, output)?,
        Command::Run {
            program,
            input,
            prover_input,
        } => run_to_file(program, input, prover_input)?,
        Command::Prove {
            program,
            input,
            output,
            proof,
            prover_parameters,
        } => prove(program, input, output, proof, prover_parameters)?,
        Command::Setup { candidate, output } => setup(candidate, output)?,
        Command::Verify {
            candidate,
            parameters,
        } => verify(candidate, parameters)?,
    }
    Ok(())
}
