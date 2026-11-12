# ZK experiment collection

This repository contains independent Ziren, SP1, and snarkVM experiments,
organized as peer directories. The Ziren experiment demonstrates a single-note
register authenticated by a compressed STARK. The native Rust client holds the
32-byte secret and creates proofs using a reusable proving-key artifact. The
contract author generates the keys once and distributes the proving key to app
users. The browser only accepts a candidate proof and submits it to the Freenet
node; the contract verifies it.

The guest proves `Poseidon2(secret) == public_y` and commits
`(public_y, counter, Poseidon2(UTF-8(text)))`. The contract checks the public
key parameter, recomputes the text hash, and verifies those exact public values
against the proof using Ziren's `StarkVerifier`.

The contract stores the candidate with the greatest counter. Equal counters
are ordered deterministically by `(text hash, text bytes, proof bytes)` so
concurrent updates converge.

## Requirements

- Rust and the Ziren guest toolchain, as described in
  [Ziren's installation guide](https://github.com/ProjectZKM/Ziren/blob/0e2d38388e580f3e061e9bb13d4173d6a614b6a6/docs/src/introduction/installation.md).
- The active Rust toolchain's `wasm32-unknown-unknown` target
  (`rustup target add wasm32-unknown-unknown`).
- `fdev` and a running local Freenet node.
- Node.js and npm.

The Ziren and Plonky3 Git dependencies are pinned in
[`ziren/Cargo.toml`](./ziren/Cargo.toml). The guest must be rebuilt with that
same Ziren revision because its ELF determines the verifying key in the
contract parameters.

## Experiment layout and runner

The experiment workspaces are siblings: [`ziren/`](./ziren/), [`sp1/`](./sp1/),
[`snarkvm/`](./snarkvm/), [`sp1-keccak/`](./sp1-keccak/),
[`snarkvm-keccak-poseidon/`](./snarkvm-keccak-poseidon/), and
[`stwo-cairo/`](./stwo-cairo/). Each keeps its own Cargo workspace or package
and lockfile so toolchains and dependency graphs remain independent. The root
[`shared/`](./shared/) crate contains code reused by Ziren and SP1, plus an
in-process resource measurement library used by the native clients.

Each experiment has its own `setup.sh` and `run_all.sh`. Experiments with
environment configuration keep that in a separate `env.sh` (or `env.fish`).
Source the matching environment script when you want to configure your current
shell without generating artifacts; run `setup.sh` when you want to prepare the
experiment without the full workflow:

```sh
source snarkvm-keccak-poseidon/env.sh
bash snarkvm-keccak-poseidon/setup.sh

# Fish
source snarkvm-keccak-poseidon/env.fish
source ziren/env.fish

bash sp1/setup.sh
bash sp1/run_all.sh
```

`run_all.sh` loads its `env.sh` if present, runs setup, then the experiment's
prove/verify steps. Output is printed directly to the terminal.
Run all experiments sequentially from the collection root with:

```sh
bash scripts/run-experiments.sh
```

The runner discovers and executes every experiment directory's `run_all.sh` in
alphabetical order. Native clients use the shared measurer for their commands
or resource-intensive phases, reporting elapsed time, CPU use, and resident
memory; Cargo compilation happens outside the measured runtime. Local secrets
and outputs are stored within their experiment directories. To include a
contract-specific benchmark, add `contract-benchmark.sh` to each
experiment that supports it, then use `--include-contract-benchmarks`. Contract
scripts are separate from proof correctness checks so they can build or
benchmark a Freenet contract without coupling those measurements to proving.

To add another experiment, give it a sibling directory, its own Cargo manifest
and lockfile, and `setup.sh` and `run_all.sh` scripts. Keep the scripts simple:
use the native client for resource measurements, not shell wrappers. The
collection runner discovers `run_all.sh` automatically. Keep protocol-specific
setup and proof code in that experiment. Put optional contract build/submission
measurements in its `contract-benchmark.sh` so proof benchmarks and Freenet
contract benchmarks can be run independently.

Link zkm as toolchain 

```sh
rustup toolchain link zkm "$HOME/.zkm-toolchain/rust-toolchain-x86-64-unknown-linux-gnu-20260917"
```

To start a fish shell with the Ziren toolchain environment loaded, run this
from the project root (you usually dont need this):

```sh
./ziren/scripts/zkm-fish
```

The script sources `~/.zkm-toolchain/env` in Bash, then starts fish with those
environment variables. Exit the fish shell to return to the original shell.

## Prepare a secret and contract parameters

Create a 32-byte secret file and keep it private:

```sh
head -c 32 /dev/urandom > ziren/secret.bin
```

Generate `ziren/contract-parameters.json` and
`ziren/notepad-proving-key.bin`. This derives
the public `y` value from the secret and computes the Ziren proving and
verifying keys. If the proving-key file already exists, setup reuses it
instead of recomputing the keys:

```sh
cargo run --release --manifest-path ziren/Cargo.toml -p notepad-client -- setup \
  --secret ziren/secret.bin \
  --output ziren/contract-parameters.json \
  --proving-key ziren/notepad-proving-key.bin
```

Publish the contract with those parameters. `ziren/contract-parameters.json` is
public and must be kept alongside the contract instance ID. Distribute
`ziren/notepad-proving-key.bin` to users who need to create proofs; it is not a
secret, but it must match the guest ELF and pinned Ziren version. Keep
`ziren/secret.bin` private.

Add `--cuda` to `setup` to generate a new key with the CUDA prover, or to
`prove` to generate the proof with CUDA. The key and proof formats and local
verification remain the same; omit the flag to use the configured default
prover. The client prints elapsed times for key setup/cache loading, proof
generation, and local verification.

## Build and publish the webapp

```sh
cd ziren
npm install
PATH="$PWD/node_modules/.bin:$PATH" fdev build
fdev publish --code build/freenet/container \
  --parameters contract-parameters.json contract
```

The `PATH` prefix lets `fdev` find the project's local webpack executable.
The packaged contract is written to `build/freenet/container`. Use the
contract instance ID returned by `fdev publish` in the browser page.
The webapp connects to the hosting node's `/contract/command/` WebSocket
endpoint.

## Create and submit an update

The counter must increase for a new note to become current:

```sh
cargo run --manifest-path ziren/Cargo.toml -p notepad-client -- prove \
  --secret ziren/secret.bin \
  --parameters ziren/contract-parameters.json \
  --proving-key ziren/notepad-proving-key.bin \
  --counter 1 \
  --text "A note authenticated by Ziren" \
  --output ziren/candidate.json
```

Verify a candidate offline against the contract parameters. This needs neither
the secret nor the proving key:

```sh
cargo run --manifest-path ziren/Cargo.toml -p notepad-client -- verify \
  --parameters ziren/contract-parameters.json \
  --candidate ziren/candidate.json
```

Open the webapp, load `ziren/candidate.json`, and submit it. The contract stores
the candidate only if its proof verifies and its counter wins.

The same candidate can be submitted directly from the native client:

```sh
cargo run --manifest-path ziren/Cargo.toml -p notepad-client -- submit \
  --endpoint ws://127.0.0.1:50509/v1/contract/command?encodingProtocol=native \
  --contract-code ziren/build/freenet/container \
  --parameters ziren/contract-parameters.json \
  --candidate ziren/candidate.json
```

## Checks

```sh
cargo test --manifest-path shared/Cargo.toml --features hash
cargo test --manifest-path ziren/Cargo.toml -p container
(cd ziren && npm run build)
```

The shared hash test uses the reference vector from Ziren's Poseidon2 guest,
covering its padding, byte packing, and output encoding.

## SP1 implementation

An independent SP1 prover and Freenet contract live in [sp1/](./sp1/). This
is a separate Cargo workspace with its own lockfile and target directory, so
SP1 builds and WASM size measurements do not include the Ziren dependencies.
The workspace shares host setup, proving-key loading, compressed-proof
verification, and Freenet submission between its notepad and Poseidon2 Merkle
membership experiments. See [sp1/README.md](./sp1/README.md) for proof
generation, offline verification, and separate contract-size measurements.

## Stwo Cairo performance

The [`stwo-cairo/`](./stwo-cairo/) experiment runs Cairo 1 programs through
Stwo's Cairo 1.3 bootloader. A single local release-mode run of
`bash stwo-cairo/run_all.sh --dev` produced and verified both proofs:

| Workload | Cairo VM | Stwo adaptation | Proof generation | Verification | Peak proving RSS |
| --- | ---: | ---: | ---: | ---: | ---: |
| Poseidon hash | 2 ms | 45 ms | 20.6 s | 39 ms | 14,515 MiB |
| Depth-2 Poseidon Merkle | 4 ms | 45 ms | 20.9 s | 21 ms | 14,632 MiB |

These are smoke-test measurements using the development parameters (10 bits of
conjectured soundness), not production proving results. Host hardware details
were not recorded, so these numbers should not be compared directly with the
historical Ziren and SP1 runs below. The default Stwo profile was not measured.
See [`stwo-cairo/README.md`](./stwo-cairo/README.md) for setup and proof
commands.

## Historical test runs

The transcripts below were recorded before the experiment directories were
introduced, so their command paths reflect the old layout. Use the current
commands above or `scripts/run-experiments.sh` for the reorganized workspace.

## Test run for Ziren on my machine 

Lanuch local-only freenet node with

```sh
freenet local --data-dir "$PWD/.freenet-local" 
```

This was run on CPU (debug build, single core)

```sh
/e/p/zk-example-freenet [2]> cargo run -p notepad-client -- setup \
                                   --secret secret.bin \
                                   --output contract-parameters.json \
                                   --proving-key notepad-proving-key.bin
warning: notepad-client@0.1.0: notepad-guest built at 2026-11-02 09:54:14
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s
     Running `/s2a/cargo-target/debug/notepad-client setup --secret secret.bin --output contract-parameters.json --proving-keynotepad-proving-key.bin`
Proving-key setup and cache write took 602.287s
Wrote public contract parameters to contract-parameters.json
Wrote reusable proving key to notepad-proving-key.bin
Public y: f2138227f062911d164e764f7475546dfd7dd26dec56197cce6a5e5a51492479
Setup command took 602.287
```

I re-ran this with 6 cores

It had

```rs
        .prove(&proving_key, stdin)
        .compressed()
```

```sh
set -gx RAYON_NUM_THREADS 6
set -gx ZIREN_PREWARM_THREADS 6


    Finished `release` profile [optimized] target(s) in 2m 30s
     Running `/s2a/cargo-target/release/notepad-client setup --secret secret.bin --output contract-parameters.json --proving-key notepad-proving-key.bin`
Proving-key setup and cache write took 79.885s
Wrote public contract parameters to contract-parameters.json
Wrote reusable proving key to notepad-proving-key.bin
Public y: f2138227f062911d164e764f7475546dfd7dd26dec56197cce6a5e5a51492479
Setup command took 79.886s


cargo run --release --manifest-path ziren/Cargo.toml -p notepad-client -- prove \
  --secret secret.bin \
  --parameters contract-parameters.json \
  --proving-key notepad-proving-key.bin \
  --counter 1 \
  --text "A note authenticated by Ziren" \
  --output candidate.json

      Finished `release` profile [optimized] target(s) in 0.29s
     Running `/s2a/cargo-target/release/notepad-client prove --secret secret.bin --parameters contract-parameters.json --proving-key notepad-proving-key.bin --counter 1 --text 'A note authenticated by Ziren' --output candidate.json`
Proving-key load took 0.012s
Proof generation took 523.488s
Proof verification took 0.028s
Wrote verified update candidate to candidate.json
Prove command took 523.530s


cargo run --release --manifest-path ziren/Cargo.toml -p notepad-client -- verify \
                               --parameters contract-parameters.json \
                               --candidate candidate.json
   Compiling notepad-client v0.1.0 (/e/proj/zk-example-freenet/client)
warning: notepad-client@0.1.0: notepad-guest built at 2026-11-02 10:36:25
    Finished `release` profile [optimized] target(s) in 5.17s
     Running `/s2a/cargo-target/release/notepad-client verify --parameters contract-parameters.json --candidate candidate.json`
Proof verification took 0.030s
Candidate proof is valid.
Verify command took 0.033s
```

## Test run with SP1 

I started this experiment with Ziren because SP1 verifier that can run in WASM does not support single-shard proofs (short proofs)

But given how slow Ziren is I'm giving SP1 a try.

Install `protobuf-devel`

This run used sha256

```sh
/e/p/zk-example-freenet (master) [101]> cargo run -r --manifest-path sp1/Cargo.toml -p sp1-notepad-client -- setup \
                                              --secret secret.bin \
                                              --output sp1-contract-parameters.json \
                                              --proving-key sp1-proving-key.bin
warning: sp1-notepad-client@0.1.0: rustc +succinct --version: "rustc 1.96.0-dev\n"
warning: sp1-notepad-client@0.1.0: sp1-notepad-guest built at 2026-11-02 11:18:53
   Compiling sp1-notepad-client v0.1.0 (/e/proj/zk-example-freenet/sp1/client)
    Finished `release` profile [optimized] target(s) in 40.13s
warning: the following packages contain code that will be rejected by a future version of Rust: proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 4`
     Running `/s2a/cargo-target/release/sp1-notepad-client setup --secret secret.bin --output sp1-contract-parameters.json --proving-key sp1-proving-key.bin`
Proving-key setup and cache write took 37.772s
Wrote public contract parameters to sp1-contract-parameters.json
Wrote reusable proving key to sp1-proving-key.bin
Public y: bb3c54e79e088a7214c811618d58b96b8cf8d16a3f348700ae386e6c5505ff69
Setup command took 37.773s

/e/p/zk-example-freenet (master) [SIGINT]> cargo run -r --manifest-path sp1/Cargo.toml -p sp1-notepad-client -- prove \
                                                 --secret secret.bin \
                                                 --parameters sp1-contract-parameters.json \
                                                 --proving-key sp1-proving-key.bin \
                                                 --counter 1 \
                                                 --text "A note authenticated by SP1" \
                                                 --output sp1-candidate.json
warning: sp1-notepad-client@0.1.0: rustc +succinct --version: "rustc 1.96.0-dev\n"
warning: sp1-notepad-client@0.1.0: sp1-notepad-guest built at 2026-11-02 11:18:53
   Compiling sp1-notepad-client v0.1.0 (/e/proj/zk-example-freenet/sp1/client)
    Finished `release` profile [optimized] target(s) in 37.58s
warning: the following packages contain code that will be rejected by a future version of Rust: proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 4`
     Running `/s2a/cargo-target/release/sp1-notepad-client prove --secret secret.bin --parameters sp1-contract-parameters.json --proving-key sp1-proving-key.bin --counter 1 --text 'A note authenticated by SP1' --output sp1-candidate.json`
Proving-key load took 0.001s
Proof generation took 94.417s
Proof verification took 0.043s
WASM verifier accepts this proof: yes
Wrote verified update candidate to sp1-candidate.json
Prove command took 125.704s

/e/p/zk-example-freenet (master) [SIGINT]> 
                                           cargo run -r --manifest-path sp1/Cargo.toml -p sp1-notepad-client -- verify \
                                                 --parameters sp1-contract-parameters.json \
                                                 --candidate sp1-candidate.json
warning: sp1-notepad-client@0.1.0: rustc +succinct --version: "rustc 1.96.0-dev\n"
warning: sp1-notepad-client@0.1.0: sp1-notepad-guest built at 2026-11-02 11:18:53
    Finished `release` profile [optimized] target(s) in 0.23s
warning: the following packages contain code that will be rejected by a future version of Rust: proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 4`
     Running `/s2a/cargo-target/release/sp1-notepad-client verify --parameters sp1-contract-parameters.json --candidate sp1-candidate.json`
Proof verification took 0.046s
WASM verifier accepts this proof: yes
Candidate proof is valid.
Verify command took 0.058s
```

The notepad defaults to `secret.bin`, `sp1-contract-parameters.json`,
`sp1-proving-key.bin`, and `sp1-candidate.json`.

This merkle example is useful for anonymous membership proofs. In practice we prefer Sparse Merkle Tree instead of this simple one.

The same SP1 test workflow also exercises the Poseidon2 Merkle membership
program. Generate its independent tree secret once, then run setup, prove, and
verify:

```sh
head -c 32 /dev/urandom > sp1-merkle-tree-secret.bin

cargo run -r --manifest-path sp1/Cargo.toml -p sp1-merkle-client -- setup
cargo run -r --manifest-path sp1/Cargo.toml -p sp1-merkle-client -- prove \
  --leaf-index 42 --count-cycles
cargo run -r --manifest-path sp1/Cargo.toml -p sp1-merkle-client -- verify
```

By default, the Merkle client writes
`sp1-merkle-contract-parameters.json`, `sp1-merkle-proving-key.bin`, and
`sp1-merkle-candidate.json`; its tree secret defaults to
`sp1-merkle-tree-secret.bin`. These paths are separate from the notepad's and
can be overridden with their corresponding options.

```sh
/e/p/zk-example-freenet (master)> cargo run -r --manifest-path sp1/Cargo.toml -p sp1-merkle-client -- setup
   Compiling sp1-merkle-client v0.1.0 (/e/proj/zk-example-freenet/sp1/merkle-client)
   Compiling sp1-host-common v0.1.0 (/e/proj/zk-example-freenet/sp1/host-common)
warning: sp1-merkle-client@0.1.0: rustc +succinct --version: "rustc 1.96.0-dev\n"
warning: sp1-merkle-client@0.1.0: sp1-merkle-guest built at 2026-11-03 04:30:08
    Finished `release` profile [optimized] target(s) in 1m 06s
warning: the following packages contain code that will be rejected by a future version of Rust: proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 4`
     Running `/s2a/cargo-target/release/sp1-merkle-client setup`
[Poseidon2 tree construction] elapsed 0.0s | CPU 0.0% | memory 7.9 MiB | peak 7.9 MiB
[Poseidon2 tree construction] finished in 0.5s | peak memory 7.9 MiB
Poseidon2 tree construction took 0.485s
[SP1 proving-key setup] elapsed 35.1s | CPU 735.4% | memory 7048.3 MiB | peak 7048.3 MiBB
[SP1 proving-key setup] finished in 36.0s | peak memory 7048.3 MiB
Proving-key setup and cache write took 35.992s
Wrote public contract parameters to sp1-merkle-contract-parameters.json
Wrote reusable proving key to sp1-merkle-proving-key.bin
Public Merkle root: [1132742922, 2083440627, 2042222920, 932570131, 1604652421, 1170845756, 1223358478, 736266596]
Setup command took 36.477s

/e/p/zk-example-freenet (master)> cargo run -r --manifest-path sp1/Cargo.toml -p sp1-merkle-client -- prove \
                                        --leaf-index 42
warning: sp1-merkle-client@0.1.0: rustc +succinct --version: "rustc 1.96.0-dev\n"
warning: sp1-merkle-client@0.1.0: sp1-merkle-guest built at 2026-11-03 04:30:08
   Compiling sp1-merkle-client v0.1.0 (/e/proj/zk-example-freenet/sp1/merkle-client)
    Finished `release` profile [optimized] target(s) in 40.82s
warning: the following packages contain code that will be rejected by a future version of Rust: proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 4`
     Running `/s2a/cargo-target/release/sp1-merkle-client prove --leaf-index 42`
Proving-key load took 0.001s
[SP1 compressed proof generation] elapsed 123.1s | CPU 807.3% | memory 10579.5 MiB | peak 18412.0 MiBB
[SP1 compressed proof generation] finished in 123.9s | peak memory 18412.0 MiB
Proof generation took 123.874s
[SP1 compressed proof verification] elapsed 0.0s | CPU 0.0% | memory 9123.7 MiB | peak 9123.7 MiB
[SP1 compressed proof verification] finished in 0.0s | peak memory 9123.7 MiB
Proof verification took 0.045s
Serialized proof size: 1272546 bytes
Wrote verified candidate to sp1-merkle-candidate.json
Prove command took 155.488s

/e/p/zk-example-freenet (master)> cargo run -r --manifest-path sp1/Cargo.toml -p sp1-merkle-client -- verify
warning: sp1-merkle-client@0.1.0: rustc +succinct --version: "rustc 1.96.0-dev\n"
warning: sp1-merkle-client@0.1.0: sp1-merkle-guest built at 2026-11-03 04:30:08
    Finished `release` profile [optimized] target(s) in 0.34s
warning: the following packages contain code that will be rejected by a future version of Rust: proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 4`
     Running `/s2a/cargo-target/release/sp1-merkle-client verify`
[SP1 compressed proof verification] elapsed 0.0s | CPU 0.0% | memory 12.3 MiB | peak 12.3 MiB
[SP1 compressed proof verification] finished in 0.1s | peak memory 12.3 MiB
Proof verification took 0.057s
Candidate proof is valid.
Verify command took 0.076s

/e/p/zk-example-freenet (master)> cargo run -r --manifest-path sp1/Cargo.toml -p sp1-merkle-client -- prove \
                                        --leaf-index 42 --count-cycles
warning: sp1-merkle-client@0.1.0: rustc +succinct --version: "rustc 1.96.0-dev\n"
warning: sp1-merkle-client@0.1.0: sp1-merkle-guest built at 2026-11-03 04:30:08
    Finished `release` profile [optimized] target(s) in 0.23s
warning: the following packages contain code that will be rejected by a future version of Rust: proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 4`
     Running `/s2a/cargo-target/release/sp1-merkle-client prove --leaf-index 42 --count-cycles`
Proving-key load took 0.001s
[SP1 guest cycle count] elapsed 0.0s | CPU 0.0% | memory 6590.5 MiB | peak 6590.5 MiB
[SP1 guest cycle count] finished in 0.0s | peak memory 6590.5 MiB
SP1 guest cycle count: 1912850
```

## Test run with SP1, corrected.

Previous code was using SHA256 in notepad and non-accelerated Poseidon in merkle client. 

This run have all AI misalignment corrected.

```sh
/e/p/zk-example-freenet (master)> cargo run -r --manifest-path sp1/Cargo.toml -p sp1-notepad-client -- setup
warning: sp1-notepad-client@0.1.0: rustc +succinct --version: "rustc 1.96.0-dev\n"
warning: sp1-notepad-client@0.1.0: sp1-notepad-guest built at 2026-11-03 05:15:39
    Finished `release` profile [optimized] target(s) in 0.25s
warning: the following packages contain code that will be rejected by a future version of Rust: proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 4`
     Running `/s2a/cargo-target/release/sp1-notepad-client setup`
[SP1 proving-key setup] elapsed 35.0s | CPU 951.4% | memory 7133.3 MiB | peak 7133.3 MiBB
[SP1 proving-key setup] finished in 35.2s | peak memory 7133.3 MiB
Proving-key setup and cache write took 35.225s
Wrote public contract parameters to sp1-contract-parameters.json
Wrote reusable proving key to sp1-proving-key.bin
Public y: de92bd78e36ec44fe5ee471494c0195d3532f45a7bd8a65dda6ece483cb21d4d
Setup command took 35.226s

/e/p/zk-example-freenet (master)> cargo run -r --manifest-path sp1/Cargo.toml -p sp1-notepad-client -- prove --count-cycles --counter 10 --text "Testing" 
warning: sp1-notepad-client@0.1.0: rustc +succinct --version: "rustc 1.96.0-dev\n"
warning: sp1-notepad-client@0.1.0: sp1-notepad-guest built at 2026-11-03 05:15:39
    Finished `release` profile [optimized] target(s) in 0.22s
warning: the following packages contain code that will be rejected by a future version of Rust: proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 4`
     Running `/s2a/cargo-target/release/sp1-notepad-client prove --count-cycles --counter 10 --text Testing`
Proving-key load took 0.001s
[SP1 guest cycle count] elapsed 0.0s | CPU 0.0% | memory 6603.8 MiB | peak 6603.8 MiB
[SP1 guest cycle count] finished in 0.0s | peak memory 6603.8 MiB
SP1 guest cycle count: 21577
[SP1 compressed proof generation] elapsed 101.1s | CPU 853.7% | memory 8855.8 MiB | peak 16194.5 MiBB
[SP1 compressed proof generation] finished in 102.0s | peak memory 16194.5 MiB
Proof generation took 102.020s
[SP1 compressed proof verification] elapsed 0.0s | CPU 0.0% | memory 7329.6 MiB | peak 7329.6 MiB
[SP1 compressed proof verification] finished in 0.0s | peak memory 7329.6 MiB
Proof verification took 0.045s
WASM verifier accepts this proof: yes
Wrote verified update candidate to sp1-candidate.json
Prove command took 133.708s

/e/p/zk-example-freenet (master)> cargo run -r --manifest-path sp1/Cargo.toml -p sp1-notepad-client -- verify
warning: sp1-notepad-client@0.1.0: rustc +succinct --version: "rustc 1.96.0-dev\n"
warning: sp1-notepad-client@0.1.0: sp1-notepad-guest built at 2026-11-03 05:15:39
    Finished `release` profile [optimized] target(s) in 0.24s
warning: the following packages contain code that will be rejected by a future version of Rust: proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 4`
     Running `/s2a/cargo-target/release/sp1-notepad-client verify`
[SP1 compressed proof verification] elapsed 0.0s | CPU 0.0% | memory 12.2 MiB | peak 12.2 MiB
[SP1 compressed proof verification] finished in 0.0s | peak memory 12.2 MiB
Proof verification took 0.044s
WASM verifier accepts this proof: yes
Candidate proof is valid.
Verify command took 0.056s
```

```sh
/e/p/z/sp1 (master)> cd merkle-guest/
/e/p/z/s/merkle-guest (master)> cargo prove build
[sp1]      Finished `release` profile [optimized] target(s) in 0.09s
cargo:rustc-env=SP1_ELF_sp1-merkle-guest=/s2a/cargo-target/elf-compilation/riscv64im-succinct-zkvm-elf/release/sp1-merkle-guest

/e/p/zk-example-freenet (master) [2]> cargo run -r --manifest-path sp1/Cargo.toml -p sp1-merkle-client -- clear
/e/p/zk-example-freenet (master)> head -c 32 /dev/urandom > sp1-merkle-tree-secret.bin

/e/p/zk-example-freenet (master)> cargo run -r --manifest-path sp1/Cargo.toml -p sp1-merkle-client -- setup
warning: sp1-merkle-client@0.1.0: rustc +succinct --version: "rustc 1.96.0-dev\n"
warning: sp1-merkle-client@0.1.0: sp1-merkle-guest built at 2026-11-03 05:21:39
    Finished `release` profile [optimized] target(s) in 0.25s
warning: the following packages contain code that will be rejected by a future version of Rust: proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 4`
     Running `/s2a/cargo-target/release/sp1-merkle-client setup`
Expected Poseidon2 hashes: 196607 (full-tree construction)
[Poseidon2 tree construction] elapsed 0.0s | CPU 0.0% | memory 7.0 MiB | peak 7.0 MiB
[Poseidon2 tree construction] finished in 0.7s | peak memory 7.0 MiB
Poseidon2 tree construction took 0.726s
[SP1 proving-key setup] elapsed 34.0s | CPU 894.8% | memory 7157.6 MiB | peak 7157.6 MiBB
[SP1 proving-key setup] finished in 34.4s | peak memory 7157.6 MiB
Proving-key setup and cache write took 34.375s
Wrote public contract parameters to sp1-merkle-contract-parameters.json
Wrote reusable proving key to sp1-merkle-proving-key.bin
Public Merkle root: [55345880, 1636996592, 410100043, 1704962001, 1025493057, 772939018, 1787655375, 393311365]
Setup command took 35.100s

/e/p/zk-example-freenet (master)> cargo run -r --manifest-path sp1/Cargo.toml -p sp1-merkle-client -- prove \
                                        --leaf-index 42 --count-cycles
warning: sp1-merkle-client@0.1.0: rustc +succinct --version: "rustc 1.96.0-dev\n"
warning: sp1-merkle-client@0.1.0: sp1-merkle-guest built at 2026-11-03 05:21:39
    Finished `release` profile [optimized] target(s) in 0.22s
warning: the following packages contain code that will be rejected by a future version of Rust: proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 4`
     Running `/s2a/cargo-target/release/sp1-merkle-client prove --leaf-index 42 --count-cycles`
Expected Poseidon2 hashes: 196607 (membership witness construction)
Expected Poseidon2 hashes: 17 (guest membership path: one leaf hash + 16 sibling hashes)
Proving-key load took 0.000s
[SP1 guest cycle count] elapsed 0.0s | CPU 0.0% | memory 6592.1 MiB | peak 6592.1 MiB
[SP1 guest cycle count] finished in 0.0s | peak memory 6592.1 MiB
SP1 guest cycle count: 64774
[SP1 compressed proof generation] elapsed 96.1s | CPU 950.0% | memory 9463.7 MiB | peak 17103.9 MiBBB
[SP1 compressed proof generation] finished in 96.6s | peak memory 17103.9 MiB
Proof generation took 96.600s
[SP1 compressed proof verification] elapsed 0.0s | CPU 0.0% | memory 8103.9 MiB | peak 8103.9 MiB
[SP1 compressed proof verification] finished in 0.0s | peak memory 8103.9 MiB
Proof verification took 0.045s
Serialized proof size: 1272546 bytes
Wrote verified candidate to sp1-merkle-candidate.json
Prove command took 129.442s

/e/p/zk-example-freenet (master)> cargo run -r --manifest-path sp1/Cargo.toml -p sp1-merkle-client -- verify
warning: sp1-merkle-client@0.1.0: rustc +succinct --version: "rustc 1.96.0-dev\n"
warning: sp1-merkle-client@0.1.0: sp1-merkle-guest built at 2026-11-03 05:21:39
    Finished `release` profile [optimized] target(s) in 0.26s
warning: the following packages contain code that will be rejected by a future version of Rust: proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 4`
     Running `/s2a/cargo-target/release/sp1-merkle-client verify`
[SP1 compressed proof verification] elapsed 0.0s | CPU 0.0% | memory 11.8 MiB | peak 11.8 MiB
[SP1 compressed proof verification] finished in 0.0s | peak memory 11.8 MiB
Proof verification took 0.044s
Candidate proof is valid.
Verify command took 0.057s
```

# Sp1-notepad run, with fixed hash path

```sh
== SP1 proving-key setup started ==                                                                                          
SP1 proving-key setup
== SP1 proving-key setup finished in 45.6s | CPU 629.9% (peak 892.1%) | memory 6632 MiB (peak 7183 MiB) ==Proving-key setup and cache write took 45.673s
Wrote public contract parameters to /e/proj/zk-example-freenet/sp1/contract-parameters.json
Wrote reusable proving key to /e/proj/zk-example-freenet/sp1/proving-key.bin
Public y: 66494739c6a16801f9416b0c571a2e3cafb29d52076fcd7524177a046b625b72
Setup command took 45.674s


```