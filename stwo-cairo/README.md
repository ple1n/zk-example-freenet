# Stwo Cairo experiments

This project proves two native-Cairo-field computations with Stwo and checks
the resulting candidates in a Freenet contract:

- `poseidon_hash.cairo` proves one private `felt` input's Poseidon hash.
- `poseidon_merkle.cairo` proves depth-2 membership in a Poseidon Merkle tree.
  The leaf, siblings, and left/right path bits are private; the public output is
  the two-limb encoding of the root.

The source programs use standard Cairo 1 syntax. The host CLI copies each source
into a temporary compiler project and uses Cairo's executable plugin to generate
the input/output ABI Stwo expects. The plugin attribute is added only to that
temporary copy; the editor-visible sources remain standard Cairo. Stwo's Cairo
1.3 bootloader task path runs the artifact and adapts the resulting Cairo VM
trace.
`felt252` values are the Cairo native field, not a byte-string or foreign-field
encoding. Inputs are ordered arrays of decimal strings in JSON to avoid
host-side integer precision loss.

## Setup and local proof workflow

Scarb manages the Cairo core library. From the repository root, run:

```sh
bash stwo-cairo/run_all.sh
```

`setup.sh` resolves the core library source directory from Scarb metadata;
`CAIRO_CORELIB_PATH` can be set to override it. It requires Scarb and `jq`,
compiles both programs into Cairo 1 executable artifacts, and installs the WASM
target for the pinned Rust toolchain. By default,
`run_all.sh` runs both experiments sequentially with the default 96-bit proof
parameters and writes proofs, candidates, and contract parameters under
`stwo-cairo/artifacts/`.
The shared process monitor reports live elapsed time, CPU, and RSS for Cairo
execution, Stwo adaptation, proof generation, and verification.

The workflow uses `dev-prover-parameters.json` to keep local proving practical.
It has only 10 bits of conjectured soundness (`pow_bits + log_blowup_factor *
n_queries`) and is strictly for smoke tests, not production or adversarial
settings. Use it explicitly when you need a faster smoke test:

```sh
bash stwo-cairo/run_all.sh --dev
```

The parameter file's lifting size and preprocessed trace are also
development-specific.

Each command can be run separately:

```sh
cargo +nightly-2026-01-15 run --release \
  --manifest-path stwo-cairo/Cargo.toml -p stwo-cairo-cli -- prove \
  --program stwo-cairo/build/poseidon_hash.json \
  --input stwo-cairo/artifacts/hash-input.json \
  --output stwo-cairo/artifacts/hash-candidate.json \
  --proof stwo-cairo/artifacts/hash-proof.bin

cargo +nightly-2026-01-15 run --release \
  --manifest-path stwo-cairo/Cargo.toml -p stwo-cairo-cli -- setup \
  --candidate stwo-cairo/artifacts/hash-candidate.json \
  --output stwo-cairo/artifacts/hash-contract-parameters.json

cargo +nightly-2026-01-15 run --release \
  --manifest-path stwo-cairo/Cargo.toml -p stwo-cairo-cli -- verify \
  --candidate stwo-cairo/artifacts/hash-candidate.json \
  --parameters stwo-cairo/artifacts/hash-contract-parameters.json
```

`setup` verifies a candidate and pins its Cairo program hash and public output
into the contract parameters. Treat those parameters as the trust anchor: they
must be established by the party deploying the contract. The candidate contains
the compressed proof; private program inputs are not included.

## Freenet contract and UI

These deployment commands use `jq` to resolve Cargo's target directory.

Build the contract for WASM:

```sh
TARGET_DIR=$(cargo metadata --manifest-path stwo-cairo/Cargo.toml \
  --format-version 1 --no-deps | jq -r '.target_directory')
cargo +nightly-2026-01-15 build --release --manifest-path stwo-cairo/Cargo.toml \
  -p stwo-cairo-container --target wasm32-unknown-unknown
```

Start a local-only Freenet node in one terminal:

```sh
freenet local --data-dir "$PWD/.freenet-local"
```

In another terminal, put the contract using parameters for the matching
experiment, then submit and inspect the candidate:

```sh
fdev local execute put \
  --code "$TARGET_DIR/wasm32-unknown-unknown/release/stwo_cairo_container.wasm" \
  --parameters stwo-cairo/artifacts/hash-contract-parameters.json contract

fdev local execute update PASTE_CONTRACT_ID \
  stwo-cairo/artifacts/hash-candidate.json
fdev local execute get PASTE_CONTRACT_ID --output stwo-cairo/artifacts/hash-state.json
```

Use the corresponding `merkle-` candidate and parameters for the Merkle
experiment. Contract parameters are immutable for an instance, so each
experiment requires a contract put with its own parameters.

The upload UI sends a candidate file unchanged to an already-deployed contract.
It does not generate proofs, inspect candidates, or deploy contracts. Build it
with Dioxus CLI and publish it only to the same local-only node:

```sh
command -v dx >/dev/null || cargo install dioxus-cli --version 0.7.10 --locked
(cd stwo-cairo/ui && dx build --platform web --release)
fdev website init stwo-cairo-upload
fdev local website publish --key stwo-cairo-upload \
  "$TARGET_DIR/dx/stwo-cairo-ui/release/web/public"
freenet open freenet://PASTE_UI_CONTRACT_ID/
```

If a configured `sccache` wrapper interferes with Dioxus's WASM build, clear
the wrapper for that command:

```sh
(cd stwo-cairo/ui && RUSTC_WRAPPER= CARGO_BUILD_RUSTC_WRAPPER= \
  dx build --platform web --release)
```

Enter the contract ID printed by `fdev local execute put` and upload the
matching candidate JSON. A sent update is not proof of acceptance; check the
node response and retrieve state with `fdev local execute get`.
