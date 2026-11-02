# SP1 experiments

This is an independent Cargo workspace with its own `Cargo.lock` and `target/`
so SP1 builds and WASM size measurements stay separate from the Ziren project.
The notepad and Merkle programs use the shared `sp1-host-common` crate for
proving-key setup/loading, compressed-proof verification, and Freenet update
submission. Each program still has its own guest ELF, parameters, and contract.

Run commands from the repository root with the normal Rust toolchain, not the
Ziren shell launcher.

## SP1 notepad

```sh
cargo run --manifest-path sp1/Cargo.toml -p sp1-notepad-client -- setup \
  --secret secret.bin

cargo run --manifest-path sp1/Cargo.toml -p sp1-notepad-client -- prove \
  --counter 1 \
  --text "A note authenticated by SP1"

cargo run --manifest-path sp1/Cargo.toml -p sp1-notepad-client -- verify
```

The notepad uses SP1's `Poseidon2ByteHash` syscall for its secret and text hashes.
Host-side clients and the contract use the matching SP1 Poseidon2 sponge encoding.
Its proving key and parameters are tied to the notepad guest ELF. By default, the client reads
`secret.bin` and writes `sp1-contract-parameters.json`, `sp1-proving-key.bin`,
and `sp1-candidate.json`. Pass `--count-cycles` to `prove` to run the guest
once before proving and report its SP1 cycle count; this optional diagnostic
adds a second guest execution.

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
head -c 32 /dev/urandom > sp1-merkle-tree-secret.bin

cargo run --release --manifest-path sp1/Cargo.toml -p sp1-merkle-client -- setup \
  --depth 16

cargo run --release --manifest-path sp1/Cargo.toml -p sp1-merkle-client -- prove \
  --leaf-index 42

cargo run --release --manifest-path sp1/Cargo.toml -p sp1-merkle-client -- verify
```

`--depth` accepts 1 through 20; a larger depth increases tree construction
time and the private authentication path. Keep the tree secret private.
Setup and prove must use the same tree secret and parameters. Offline
verification needs neither the tree secret nor the proving key. The commands
report setup, proving, compressed proof size, and local verification timing.
By default, the client reads `sp1-merkle-tree-secret.bin` and writes
`sp1-merkle-contract-parameters.json`, `sp1-merkle-proving-key.bin`, and
`sp1-merkle-candidate.json`. These paths are separate from the notepad's and
can be overridden with the corresponding path options. Pass `--count-cycles`
to `prove` to opt into an additional guest execution that reports the SP1 cycle
count.

## Separate Freenet contract sizes

Build each contract independently and compare the resulting WASM files:

```sh
cargo build --release --manifest-path sp1/Cargo.toml -p sp1-container \
  --target wasm32-unknown-unknown
cargo build --release --manifest-path sp1/Cargo.toml -p sp1-merkle-container \
  --target wasm32-unknown-unknown

SP1_TARGET_DIR="${CARGO_TARGET_DIR:-sp1/target}"
wc -c "$SP1_TARGET_DIR/wasm32-unknown-unknown/release/sp1_container.wasm" \
  "$SP1_TARGET_DIR/wasm32-unknown-unknown/release/sp1_merkle_container.wasm"
```

Publish the Merkle contract with its matching parameters:

```sh
fdev publish \
  --code "$SP1_TARGET_DIR/wasm32-unknown-unknown/release/sp1_merkle_container.wasm" \
  --parameters sp1-merkle-contract-parameters.json contract
```

Submit an offline-verified Merkle candidate to a Freenet node with the native
client:

```sh
cargo run --release --manifest-path sp1/Cargo.toml -p sp1-merkle-client -- submit \
  --endpoint ws://127.0.0.1:50509/v1/contract/command?encodingProtocol=native \
  --contract-code "$SP1_TARGET_DIR/wasm32-unknown-unknown/release/sp1_merkle_container.wasm"
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
