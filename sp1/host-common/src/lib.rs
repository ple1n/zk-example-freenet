use std::{
    fs,
    future::Future,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use anyhow::{bail, Context};
use freenet_stdlib::{
    client_api::{ClientRequest, ContractRequest, ContractResponse, HostResponse, WebApi},
    prelude::{ContractCode, ContractKey, Parameters, StateDelta, UpdateData},
};
use sp1_sdk::{Elf, Prover, ProverClient, ProvingKey, SP1ProvingKey, SP1Stdin};
use sp1_verifier::compressed::SP1CompressedVerifierRaw;
use sysinfo::{Pid, ProcessRefreshKind, System};

const RESOURCE_SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Default)]
struct ResourceSample {
    cpu_percent: Option<f32>,
    memory_bytes: u64,
    peak_memory_bytes: u64,
}

pub struct ResourceMonitor {
    label: String,
    started: Instant,
    stopped: Arc<AtomicBool>,
    sample: Arc<Mutex<ResourceSample>>,
    ticker: Option<JoinHandle<()>>,
}

impl ResourceMonitor {
    pub fn start(label: &str) -> anyhow::Result<Self> {
        let started = Instant::now();
        let stopped = Arc::new(AtomicBool::new(false));
        let sample = Arc::new(Mutex::new(ResourceSample::default()));
        let thread_stopped = Arc::clone(&stopped);
        let thread_sample = Arc::clone(&sample);
        let thread_label = label.to_owned();
        let pid = Pid::from_u32(std::process::id());
        let ticker = thread::Builder::new()
            .name("sp1-resource-monitor".to_owned())
            .spawn(move || {
                let mut system = System::new();
                let refresh_kind =
                    ProcessRefreshKind::new().with_memory().with_cpu();
                system.refresh_process_specifics(pid, refresh_kind);
                let mut process_found = system.process(pid).is_some();
                if !process_found {
                    eprintln!(
                        "[{thread_label}] resource monitor could not read this process's metrics"
                    );
                }

                while !thread_stopped.load(Ordering::Relaxed) {
                    if process_found {
                        system.refresh_process_specifics(pid, refresh_kind);
                        if let Some(process) = system.process(pid) {
                            let mut current = thread_sample
                                .lock()
                                .expect("resource sample lock poisoned");
                            current.cpu_percent = Some(process.cpu_usage());
                            current.memory_bytes = process.memory();
                            current.peak_memory_bytes =
                                current.peak_memory_bytes.max(current.memory_bytes);
                            print_resource_tick(
                                &thread_label,
                                started.elapsed(),
                                &current,
                            );
                        } else {
                            process_found = false;
                            eprintln!(
                                "[{thread_label}] resource monitor lost access to this process's metrics"
                            );
                        }
                    }
                    thread::park_timeout(RESOURCE_SAMPLE_INTERVAL);
                }
            })
            .context("starting SP1 resource monitor thread")?;

        Ok(Self {
            label: label.to_owned(),
            started,
            stopped,
            sample,
            ticker: Some(ticker),
        })
    }

    fn finish(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        if let Some(ticker) = self.ticker.take() {
            ticker.thread().unpark();
            if ticker.join().is_err() {
                eprintln!(
                    "[{}] resource monitor thread terminated unexpectedly",
                    self.label
                );
            }
        }

        let elapsed = self.started.elapsed();
        let sample = self.sample.lock().expect("resource sample lock poisoned");
        println!(
            "\n[{}] finished in {:.1}s | peak memory {:.1} MiB",
            self.label,
            elapsed.as_secs_f64(),
            bytes_to_mib(sample.peak_memory_bytes),
        );
    }
}

impl Drop for ResourceMonitor {
    fn drop(&mut self) {
        self.finish();
    }
}

fn print_resource_tick(label: &str, elapsed: Duration, sample: &ResourceSample) {
    let cpu = sample
        .cpu_percent
        .map_or_else(|| "n/a".to_owned(), |usage| format!("{usage:.1}%"));
    print!(
        "\r[{label}] elapsed {:.1}s | CPU {cpu} | memory {:.1} MiB | peak {:.1} MiB",
        elapsed.as_secs_f64(),
        bytes_to_mib(sample.memory_bytes),
        bytes_to_mib(sample.peak_memory_bytes),
    );
    use std::io::Write;
    let _ = std::io::stdout().flush();
}

fn bytes_to_mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

pub async fn monitor_async<T>(
    label: &str,
    operation: impl Future<Output = anyhow::Result<T>>,
) -> anyhow::Result<T> {
    let _monitor = ResourceMonitor::start(label)?;
    operation.await
}

pub fn monitor_sync<T>(
    label: &str,
    operation: impl FnOnce() -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let _monitor = ResourceMonitor::start(label)?;
    operation()
}

pub async fn count_guest_cycles(elf: &Elf, stdin: &SP1Stdin) -> anyhow::Result<u64> {
    let elf_bytes = (&**elf).to_vec();
    let stdin = stdin.clone();
    monitor_async("SP1 guest cycle count", async move {
        tokio::task::spawn_blocking(move || sp1_prover::utils::get_cycles(&elf_bytes, &stdin))
            .await
            .context("SP1 cycle-count worker failed")
    })
    .await
}

pub fn read_proving_key(path: &Path, elf: &Elf) -> anyhow::Result<SP1ProvingKey> {
    let bytes =
        fs::read(path).with_context(|| format!("reading proving key {}", path.display()))?;
    let proving_key: SP1ProvingKey = bincode::deserialize(&bytes)
        .with_context(|| format!("deserializing proving key {}", path.display()))?;
    if &**proving_key.elf() != &**elf {
        bail!(
            "proving key {} was generated for a different guest ELF; remove it and rerun setup",
            path.display()
        );
    }
    Ok(proving_key)
}

pub async fn load_or_setup_proving_key(
    elf: &Elf,
    path: &Path,
) -> anyhow::Result<(SP1ProvingKey, bool, Duration)> {
    let started = Instant::now();
    let cache_hit = path.exists();
    let proving_key = monitor_async("SP1 proving-key setup", async {
        if cache_hit {
            read_proving_key(path, elf)
        } else {
            let client = ProverClient::builder().cpu().build().await;
            let proving_key = client
                .setup(elf.clone())
                .await
                .context("setting up SP1 proving key")?;
            let bytes = bincode::serialize(&proving_key).context("serializing SP1 proving key")?;
            fs::write(path, bytes)
                .with_context(|| format!("writing proving key {}", path.display()))?;
            Ok(proving_key)
        }
    })
    .await?;
    Ok((proving_key, cache_hit, started.elapsed()))
}

pub fn verify_compressed_proof(
    proof: &[u8],
    public_values: &[u8],
    verifying_key_hash: &[u8],
) -> anyhow::Result<()> {
    monitor_sync("SP1 compressed proof verification", || {
        SP1CompressedVerifierRaw::verify_with_public_values(
            proof,
            public_values,
            verifying_key_hash,
        )
        .context("verifying SP1 compressed proof with the WASM verifier")
    })
}

pub async fn submit_update(
    endpoint: &str,
    contract_code_path: &Path,
    parameters_path: &Path,
    delta_path: &Path,
) -> anyhow::Result<()> {
    let contract_code = ContractCode::load_raw(contract_code_path)
        .with_context(|| format!("reading contract wasm {}", contract_code_path.display()))?;
    let parameters = Parameters::from(
        fs::read(parameters_path)
            .with_context(|| format!("reading {}", parameters_path.display()))?,
    );
    let key = ContractKey::from_params_and_code(&parameters, &contract_code);
    let delta =
        fs::read(delta_path).with_context(|| format!("reading update {}", delta_path.display()))?;

    let (socket, _) = tokio_tungstenite::connect_async(endpoint)
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

    let response = tokio::time::timeout(Duration::from_secs(120), async {
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
