# SP1 experiments

This is an independent Cargo workspace with its own `Cargo.lock` and `target/`
so SP1 builds and WASM size measurements stay separate from the Ziren project.
The notepad and Merkle programs use the shared `sp1-host-common` crate for
proving-key setup/loading, compressed-proof verification, and Freenet update
submission. Each program still has its own guest ELF, parameters, and contract.

Run commands from the repository root with the normal Rust toolchain, not the
Ziren shell launcher. The experiment scripts are:

```sh
source sp1/env.sh
bash sp1/setup.sh
bash sp1/run_all.sh
```

Sourcing the environment also sets `DIR` to the SP1 experiment directory, so
commands copied from `run_all.sh` that use `"$DIR/..."` work in the same shell.
It sets `TARGET_DIR` to the target directory reported by `cargo metadata` and
requires `jq`. In Fish, use `source sp1/env.fish`.
`setup.sh` prepares the SP1 secrets, parameters, and proving keys.
`run_all.sh` runs setup followed by both proof and verification workflows and
prints output directly to the terminal. `env.sh` sets
`SP1_MERKLE_DEPTH` (default `16`); use `sp1/env.fish` when sourcing the
environment from Fish.

## SP1 notepad

```sh
cargo run --manifest-path sp1/Cargo.toml -p sp1-notepad-client -- setup \
  --secret sp1/secret.bin \
  --output sp1/contract-parameters.json \
  --proving-key sp1/proving-key.bin

cargo run --manifest-path sp1/Cargo.toml -p sp1-notepad-client -- prove \
  --secret sp1/secret.bin \
  --parameters sp1/contract-parameters.json \
  --proving-key sp1/proving-key.bin \
  --counter 1 \
  --text "A note authenticated by SP1" \
  --output sp1/candidate.json

cargo run --manifest-path sp1/Cargo.toml -p sp1-notepad-client -- verify \
  --parameters sp1/contract-parameters.json \
  --candidate sp1/candidate.json
```

The notepad uses SP1's `Poseidon2ByteHash` syscall for its secret and text hashes.
Host-side clients and the contract use the matching SP1 Poseidon2 sponge encoding.
Its proving key and parameters are tied to the notepad guest ELF. The commands
above use files in `sp1/`; client path defaults are relative to the current
working directory. Pass `--count-cycles` to `prove` to run the guest once
before proving and report its SP1 cycle count; this optional diagnostic adds a
second guest execution.

## Poseidon2 Merkle membership

The Merkle experiment creates a complete tree of `2^depth` leaves. Leaf values
are derived from a private 32-byte tree secret and their indices; the leaf and
internal-node commitments use `Poseidon2ByteHash`. Each Merkle input is serialized
as canonical little-endian 32-bit field values before byte hashing. The guest proves
that a private leaf and authentication path reconstruct the public root. The
candidate contains only the compressed proof; the contract's public parameters
bind both the root and tree depth, so the leaf, index, and sibling path are not
published.

Create a separate private tree secret, then set up a depth-16 tree by default
(65,536 leaves). Setup reports the tree construction and SP1 proving-key setup
times independently:

```sh
head -c 32 /dev/urandom > sp1/merkle-tree-secret.bin

cargo run --release --manifest-path sp1/Cargo.toml -p sp1-merkle-client -- setup \
  --tree-secret sp1/merkle-tree-secret.bin \
  --depth 16 \
  --output sp1/merkle-contract-parameters.json \
  --proving-key sp1/merkle-proving-key.bin

cargo run --release --manifest-path sp1/Cargo.toml -p sp1-merkle-client -- prove \
  --tree-secret sp1/merkle-tree-secret.bin \
  --parameters sp1/merkle-contract-parameters.json \
  --proving-key sp1/merkle-proving-key.bin \
  --leaf-index 42 \
  --output sp1/merkle-candidate.json

cargo run --release --manifest-path sp1/Cargo.toml -p sp1-merkle-client -- verify \
  --parameters sp1/merkle-contract-parameters.json \
  --candidate sp1/merkle-candidate.json
```

`--depth` accepts 1 through 20; a larger depth increases tree construction
time and the private authentication path. Keep the tree secret private.
Setup and prove must use the same tree secret and parameters. Offline
verification needs neither the tree secret nor the proving key. The commands
report setup, proving, compressed proof size, and local verification timing.
The commands above use files in `sp1/`, separate from the notepad's files.
Client path defaults are relative to the current working directory. Pass
`--count-cycles` to `prove` to opt into an additional guest execution that
reports the SP1 cycle count.

## Separate Freenet contract sizes

Build each contract independently and compare the resulting WASM files:

```sh
cargo build --release --manifest-path sp1/Cargo.toml -p sp1-container \
  --target wasm32-unknown-unknown
cargo build --release --manifest-path sp1/Cargo.toml -p sp1-merkle-container \
  --target wasm32-unknown-unknown

wc -c "$TARGET_DIR/wasm32-unknown-unknown/release/sp1_container.wasm" \
  "$TARGET_DIR/wasm32-unknown-unknown/release/sp1_merkle_container.wasm"
```

## Local-only contract testing

To test either contract without publishing it to the Freenet network, start an
isolated local-only node from the repository root:

```sh
freenet local --data-dir "$PWD/.freenet-local"
```

In another terminal, build the contracts as above, then put the contract on the
local node with its matching parameters. In that terminal, source the SP1
environment (`source sp1/env.sh` in Bash/Zsh or `source sp1/env.fish` in Fish).
For example, to test the Merkle contract:

```sh
fdev local execute put \
  --code "$TARGET_DIR/wasm32-unknown-unknown/release/sp1_merkle_container.wasm" \
  --parameters sp1/merkle-contract-parameters.json \
  contract
```

Use the contract ID printed by the command to submit and inspect the candidate:

```sh
CONTRACT_ID="paste the contract ID printed by fdev local execute put"
fdev local execute update "$CONTRACT_ID" sp1/merkle-candidate.json
fdev local execute get "$CONTRACT_ID" --output state.json
```

For the notepad contract, use `sp1_container.wasm`,
`sp1/contract-parameters.json`, and `sp1/candidate.json` instead. A local
`put` stores the contract only on this node; do not use `fdev publish` for local
testing.

### Put and open the Dioxus upload UI on the local node

The UI submits an uploaded file's bytes unchanged as a contract update delta.
It does not parse the candidate or generate or verify proofs. The trace shows
the WebSocket operations and the node's low-level response details.

Install Dioxus CLI if needed:

```sh
cargo install dioxus-cli --version 0.7.10 --locked
```

Build the static UI bundle from the repository root:

```sh
(cd sp1/ui && dx build --platform web --release)
```

Create a website key once. The private key is saved in Freenet's user config
directory, not in this repository:

```sh
fdev website init sp1-upload-ui
```

With the local-only Freenet node and the SP1 contract already put, publish the
UI bundle to that same local node:

```sh
fdev local website publish --key sp1-upload-ui \
  "$TARGET_DIR/dx/sp1-ui/release/web/public"
```

Open the UI website using the website contract ID printed by the publish
command:

```sh
UI_CONTRACT_ID="paste the ID printed by fdev local website publish"
freenet open "freenet://$UI_CONTRACT_ID/"
```

In the page, enter the SP1 contract ID printed by `fdev local execute put` and
upload the corresponding candidate from `sp1/` (`candidate.json` or
`merkle-candidate.json`). The default WebSocket endpoint is
`ws://127.0.0.1:7509/v2/contract/command?encodingProtocol=native`; edit it in
the UI if the local node uses a different endpoint. Sending the update request
is not confirmation that the contract accepted it; inspect the displayed node
response.

For iterative UI development without putting the UI bundle on the node, run
`bash sp1/ui/serve.sh` instead; it serves at `http://127.0.0.1:58080`.

Publish the Merkle contract with its matching parameters:

```sh
fdev publish \
  --code "$TARGET_DIR/wasm32-unknown-unknown/release/sp1_merkle_container.wasm" \
  --parameters sp1/merkle-contract-parameters.json contract
```

Submit an offline-verified Merkle candidate to a Freenet node with the native
client:

```sh
cargo run --release --manifest-path sp1/Cargo.toml -p sp1-merkle-client -- submit \
  --endpoint ws://127.0.0.1:50509/v1/contract/command?encodingProtocol=native \
  --contract-code "$TARGET_DIR/wasm32-unknown-unknown/release/sp1_merkle_container.wasm"
```

One release-mode depth-16 run measured 0.491s for tree construction, 35.798s
for fresh proving-key setup, 126.259s for proof generation, and 0.044s for
local verification. The serialized proof was 1,272,546 bytes. In the same
build, the notepad contract WASM was 547,155 bytes and the Merkle contract
WASM was 547,162 bytes. These are sample measurements; the commands above
print fresh timings and `wc -c` reports sizes for the current toolchain/build.

The client uses SP1's CPU prover and reusable serializable proving keys. The
contract verifies SP1 compressed proofs with the same raw verifier used by the
client's offline verification path. Setup, proof generation, and verification
display a live process CPU/RSS and elapsed-time
ticker, followed by peak RSS for that stage. CPU use is process-wide and can
exceed 100% when multiple cores are busy. SP1's compressed proof result does
not expose guest cycle counts directly; `--count-cycles` opts into a separate
guest execution to collect that statistic.
