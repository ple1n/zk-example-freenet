# Ziren-authenticated Freenet notepad

This webapp demonstrates a single-note register authenticated by a Ziren
compressed STARK. The native Rust client holds the 32-byte secret and creates
proofs using a reusable proving-key artifact. The contract author generates
the keys once and distributes the proving key to app users. The browser only
accepts a candidate proof and submits it to the Freenet node; the contract
verifies it.

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

The Ziren and Plonky3 Git dependencies are pinned in `Cargo.toml`. The guest
must be rebuilt with that same Ziren revision because its ELF determines the
verifying key in the contract parameters.

Link zkm as toolchain 

```sh
rustup toolchain link zkm "$HOME/.zkm-toolchain/rust-toolchain-x86-64-unknown-linux-gnu-20260917"
```

To start a fish shell with the Ziren toolchain environment loaded, run this
from the project root (you usually dont need this):

```sh
./scripts/zkm-fish
```

The script sources `~/.zkm-toolchain/env` in Bash, then starts fish with those
environment variables. Exit the fish shell to return to the original shell.

## Prepare a secret and contract parameters

Create a 32-byte secret file and keep it private:

```sh
head -c 32 /dev/urandom > secret.bin
```

Generate `contract-parameters.json` and `notepad-proving-key.bin`. This derives
the public `y` value from the secret and computes the Ziren proving and
verifying keys. If the proving-key file already exists, setup reuses it
instead of recomputing the keys:

```sh
cargo run -r -p notepad-client -- setup \
  --secret secret.bin \
  --output contract-parameters.json \
  --proving-key notepad-proving-key.bin
```

Publish the contract with those parameters. `contract-parameters.json` is
public and must be kept alongside the contract instance ID. Distribute
`notepad-proving-key.bin` to users who need to create proofs; it is not a
secret, but it must match the guest ELF and pinned Ziren version. Keep
`secret.bin` private.

Add `--cuda` to `setup` to generate a new key with the CUDA prover, or to
`prove` to generate the proof with CUDA. The key and proof formats and local
verification remain the same; omit the flag to use the configured default
prover. The client prints elapsed times for key setup/cache loading, proof
generation, and local verification.

## Build and publish the webapp

```sh
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
cargo run -p notepad-client -- prove \
  --secret secret.bin \
  --parameters contract-parameters.json \
  --proving-key notepad-proving-key.bin \
  --counter 1 \
  --text "A note authenticated by Ziren" \
  --output candidate.json
```

Verify a candidate offline against the contract parameters. This needs neither
the secret nor the proving key:

```sh
cargo run -p notepad-client -- verify \
  --parameters contract-parameters.json \
  --candidate candidate.json
```

Open the webapp, load `candidate.json`, and submit it. The contract stores the
candidate only if its proof verifies and its counter wins.

The same candidate can be submitted directly from the native client:

```sh
cargo run -p notepad-client -- submit \
  --endpoint ws://127.0.0.1:50509/v1/contract/command?encodingProtocol=native \
  --contract-code build/freenet/container \
  --parameters contract-parameters.json \
  --candidate candidate.json
```

## Checks

```sh
cargo test -p zk-notepad-shared --features hash
cargo test -p container
npm run build
```

The shared hash test uses the reference vector from Ziren's Poseidon2 guest,
covering its padding, byte packing, and output encoding.

## Test run on my machine 

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


cargo run -r -p notepad-client -- prove \
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


cargo run -r -p notepad-client -- verify \
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