use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::{
    future::Future,
    io::{self, IsTerminal},
    sync::{
        mpsc::{self, Receiver, Sender},
        OnceLock,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use sysinfo::{Pid, System};

const SAMPLE_INTERVAL: Duration = Duration::from_millis(500);
const LABEL_ENV: &str = "ZK_MEASURE_LABEL";
static PROGRESS: OnceLock<MultiProgress> = OnceLock::new();

#[derive(Default)]
struct Metrics {
    cpu: f32,
    peak_cpu: f32,
    memory: u64,
    peak_memory: u64,
}

struct Measurement {
    label: String,
    started: Instant,
    stop: Sender<()>,
    monitor: JoinHandle<Metrics>,
}

impl Measurement {
    fn start(label: &str) -> io::Result<Self> {
        let label = std::env::var(LABEL_ENV)
            .ok()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| label.to_owned());
        let (stop, receiver) = mpsc::channel();
        let monitor_label = label.clone();
        let started = Instant::now();
        let progress_bars = progress_bars();
        let monitor = thread::Builder::new()
            .name("process-resource-monitor".into())
            .spawn(move || monitor_process(receiver, monitor_label, started, progress_bars))?;
        marker(&format!("== {label} started =="), "36");
        Ok(Self {
            label,
            started,
            stop,
            monitor,
        })
    }

    fn finish<T, E>(self, result: Result<T, E>) -> Result<T, E>
    where
        E: From<io::Error>,
    {
        let elapsed = self.started.elapsed();
        self.stop
            .send(())
            .map_err(|error| E::from(io::Error::other(error)))?;
        let metrics = self
            .monitor
            .join()
            .map_err(|_| E::from(io::Error::other("resource monitor thread panicked")))?;
        marker(
            &format!(
                "== {} {} in {:.1}s | CPU {:.1}% (peak {:.1}%) | memory {} MiB (peak {} MiB) ==",
                self.label,
                if result.is_ok() { "finished" } else { "failed" },
                elapsed.as_secs_f64(),
                metrics.cpu,
                metrics.peak_cpu,
                metrics.memory / 1_048_576,
                metrics.peak_memory / 1_048_576,
            ),
            if result.is_ok() { "32" } else { "31" },
        );
        result
    }
}

pub fn measure<T, E>(label: &str, operation: impl FnOnce() -> Result<T, E>) -> Result<T, E>
where
    E: From<io::Error>,
{
    Measurement::start(label)?.finish(operation())
}

pub async fn measure_async<T, E>(
    label: &str,
    operation: impl Future<Output = Result<T, E>>,
) -> Result<T, E>
where
    E: From<io::Error>,
{
    let measurement = Measurement::start(label)?;
    measurement.finish(operation.await)
}

fn monitor_process(
    receiver: Receiver<()>,
    label: String,
    started: Instant,
    progress_bars: Vec<ProgressBar>,
) -> Metrics {
    let pid = Pid::from_u32(std::process::id());
    let mut system = System::new();
    system.refresh_processes();
    let mut metrics = Metrics::default();

    loop {
        match receiver.recv_timeout(SAMPLE_INTERVAL) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                sample(&mut system, pid, &mut metrics);
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                sample(&mut system, pid, &mut metrics);
                update_progress(&progress_bars, &label, started.elapsed(), &metrics);
            }
        }
    }
    for bar in progress_bars {
        bar.finish_and_clear();
    }
    metrics
}

fn sample(system: &mut System, pid: Pid, metrics: &mut Metrics) {
    system.refresh_processes();
    if let Some(process) = system.process(pid) {
        metrics.cpu = process.cpu_usage();
        metrics.memory = process.memory();
        metrics.peak_cpu = metrics.peak_cpu.max(metrics.cpu);
        metrics.peak_memory = metrics.peak_memory.max(metrics.memory);
    }
}

fn marker(text: &str, color: &str) {
    let text = if io::stderr().is_terminal() {
        format!("\x1b[1;{color}m{text}\x1b[0m")
    } else {
        text.to_owned()
    };
    if let Err(error) = progress().println(text) {
        eprintln!("failed to print resource measurement: {error}");
    }
}

fn progress() -> &'static MultiProgress {
    PROGRESS.get_or_init(MultiProgress::new)
}

fn progress_bars() -> Vec<ProgressBar> {
    (0..4)
        .map(|_| {
            let bar = progress().add(ProgressBar::new_spinner());
            bar.set_style(ProgressStyle::with_template("{msg}").expect("static progress template"));
            bar
        })
        .collect()
}

fn update_progress(bars: &[ProgressBar], label: &str, elapsed: Duration, metrics: &Metrics) {
    let bar = ".".repeat(12);
    bars[0].set_message(label.to_owned());
    bars[1].set_message(format!(
        "[{}#{}] elapsed {:.1}s",
        &bar[..(elapsed.as_millis() / 500) as usize % 12],
        &bar[(elapsed.as_millis() / 500) as usize % 12 + 1..],
        elapsed.as_secs_f64(),
    ));
    bars[2].set_message(format!(
        "CPU {:.1}% (peak {:.1}%)",
        metrics.cpu, metrics.peak_cpu
    ));
    bars[3].set_message(format!(
        "memory {} MiB (peak {} MiB)",
        metrics.memory / 1_048_576,
        metrics.peak_memory / 1_048_576,
    ));
}
