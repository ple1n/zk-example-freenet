use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::Context;
use cairo_air::utils::ProofFormat;
use cairo_program_runner_lib::{ProgramInput, cairo_run_program, utils::get_program};
use cairo_vm::{
    Felt252,
    cairo_run::CairoRunConfig,
    hint_processor::builtin_hint_processor::{
        builtin_hint_processor_definition::HintProcessorData, hint_utils::get_integer_from_var_name,
    },
    hint_processor::hint_processor_definition::HintProcessorLogic,
    types::{exec_scope::ExecutionScopes, layout_name::LayoutName},
    vm::{
        errors::hint_errors::HintError,
        runners::cairo_runner::{ResourceTracker, RunResources},
        vm_core::VirtualMachine,
    },
};
use clap::{Parser, Subcommand};
use num_bigint::BigUint;
use serde::{Deserialize, Serialize};
use stwo_cairo_adapter::adapter::adapt;
use stwo_cairo_protocol::{Candidate, ContractParameters};
use stwo_cairo_prover::prover::create_and_serialize_proof;
use stwo_cairo_verifier::{inspect_candidate, verify_candidate};
use zk_notepad_shared::process::measure;

const INPUT_HINT_PREFIX: &str = "memory[ap] = program_input['";
const LOW_OUTPUT_HINT: &str = "memory[ap] = (ids.result % 2**128)";
const HIGH_OUTPUT_HINT: &str = "memory[ap] = (ids.result // 2**128)";

#[derive(Parser)]
#[command(about = "Run, prove, and verify native-field Cairo experiments with Stwo")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
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

struct ProgramInputHintProcessor {
    resources: RunResources,
    input: Option<HashMap<String, String>>,
}

impl ProgramInputHintProcessor {
    fn new() -> Self {
        Self {
            resources: RunResources::default(),
            input: None,
        }
    }

    fn read_input(exec_scopes: &ExecutionScopes) -> Result<HashMap<String, String>, HintError> {
        let input = exec_scopes
            .get_ref::<ProgramInput>("program_input")
            .map_err(|_| HintError::CustomHint("program_input is unavailable".into()))?;
        let bytes = match input {
            ProgramInput::Path(path) => fs::read(path)
                .map_err(|error| HintError::CustomHint(format!("reading input: {error}").into()))?,
            ProgramInput::Json(json) => json.as_bytes().to_vec(),
            ProgramInput::Value(_) => {
                return Err(HintError::CustomHint(
                    "in-memory program input is unsupported".into(),
                ));
            }
        };
        serde_json::from_slice(&bytes)
            .map_err(|error| HintError::CustomHint(format!("parsing input: {error}").into()))
    }

    fn insert_value(vm: &mut VirtualMachine, value: &str) -> Result<(), HintError> {
        let felt = Felt252::from_dec_str(value)
            .map_err(|error| HintError::CustomHint(format!("invalid felt252: {error}").into()))?;
        vm.insert_value(vm.get_ap(), felt)
            .map_err(|error| HintError::CustomHint(format!("writing input felt: {error}").into()))
    }

    fn insert_result_limb(
        vm: &mut VirtualMachine,
        data: &HintProcessorData,
        shift: usize,
    ) -> Result<(), HintError> {
        let value = get_integer_from_var_name("result", vm, &data.ids_data, &data.ap_tracking)?;
        let encoded = BigUint::from_bytes_be(&value.to_bytes_be());
        let limb = if shift == 0 {
            encoded & ((BigUint::from(1u8) << 128) - 1u8)
        } else {
            encoded >> shift
        };
        let felt = Felt252::from_dec_str(&limb.to_string()).map_err(|error| {
            HintError::CustomHint(format!("invalid output limb: {error}").into())
        })?;
        vm.insert_value(vm.get_ap(), felt)
            .map_err(|error| HintError::CustomHint(format!("writing output limb: {error}").into()))
    }
}

impl HintProcessorLogic for ProgramInputHintProcessor {
    fn execute_hint(
        &mut self,
        vm: &mut VirtualMachine,
        exec_scopes: &mut ExecutionScopes,
        hint_data: &Box<dyn std::any::Any>,
    ) -> Result<(), HintError> {
        let data = hint_data
            .downcast_ref::<HintProcessorData>()
            .ok_or(HintError::WrongHintData)?;
        match data.code.as_str() {
            LOW_OUTPUT_HINT => Self::insert_result_limb(vm, data, 0),
            HIGH_OUTPUT_HINT => Self::insert_result_limb(vm, data, 128),
            code => {
                let Some(key) = code
                    .strip_prefix(INPUT_HINT_PREFIX)
                    .and_then(|code| code.strip_suffix("']"))
                else {
                    return Err(HintError::UnknownHint(code.to_owned().into_boxed_str()));
                };
                if self.input.is_none() {
                    self.input = Some(Self::read_input(exec_scopes)?);
                }
                let value = self
                    .input
                    .as_ref()
                    .and_then(|input| input.get(key))
                    .ok_or_else(|| {
                        HintError::CustomHint(format!("input field '{key}' is missing").into())
                    })?;
                Self::insert_value(vm, value)
            }
        }
    }
}

impl ResourceTracker for ProgramInputHintProcessor {
    fn consumed(&self) -> bool {
        self.resources.consumed()
    }

    fn consume_step(&mut self) {
        self.resources.consume_step();
    }

    fn get_n_steps(&self) -> Option<usize> {
        self.resources.get_n_steps()
    }

    fn run_resources(&self) -> &RunResources {
        &self.resources
    }
}

fn run(
    program_path: PathBuf,
    input_path: PathBuf,
) -> anyhow::Result<stwo_cairo_adapter::ProverInput> {
    let program = get_program(&program_path)
        .with_context(|| format!("loading Cairo program {}", program_path.display()))?;
    let program_input = ProgramInput::Path(input_path);
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
    let mut hints = ProgramInputHintProcessor::new();
    let runner = measure("Cairo VM execution", || {
        cairo_run_program(&program, Some(program_input), config, Some(&mut hints))
            .context("executing Cairo program in proof mode")
    })?;
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
