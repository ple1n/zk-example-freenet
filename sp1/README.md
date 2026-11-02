# SP1 notepad workspace

This is an independent Cargo workspace for the SP1 implementation. Its own
`Cargo.lock` and `target/` keep SP1 builds separate from the Ziren workspace
and its build-size measurements. The app uses the same candidate and parameter
JSON types as the Ziren implementation, but SP1 derives `public_y` and the
text hash with SHA-256. The client currently uses SP1's CPU prover so it can
persist and reuse a serializable proving key.

Run these commands from the repository root with the normal Rust toolchain,
not the Ziren shell launcher:

```sh
cargo run --manifest-path sp1/Cargo.toml -p sp1-notepad-client -- setup \
  --secret secret.bin \
  --output sp1-contract-parameters.json \
  --proving-key sp1-proving-key.bin

cargo run --manifest-path sp1/Cargo.toml -p sp1-notepad-client -- prove \
  --secret secret.bin \
  --parameters sp1-contract-parameters.json \
  --proving-key sp1-proving-key.bin \
  --counter 1 \
  --text "A note authenticated by SP1" \
  --output sp1-candidate.json

cargo run --manifest-path sp1/Cargo.toml -p sp1-notepad-client -- verify \
  --parameters sp1-contract-parameters.json \
  --candidate sp1-candidate.json
```

`sp1-proving-key.bin` can be distributed to proof-generating app users; it is
not a secret. The contract parameters and proving key are tied to the SP1 guest
ELF. `secret.bin` must remain private.

Build only the SP1 contract for an isolated WASM size measurement:

```sh
cargo build --manifest-path sp1/Cargo.toml -p sp1-container \
  --release --target wasm32-unknown-unknown
```

The output is `sp1/target/wasm32-unknown-unknown/release/sp1_container.wasm`.
The SP1 contract verifies compressed proofs using the matching SP1 verifier.
SP1's compressed verifier requires a multi-shard proof; use a workload large
enough to produce multiple shards. The client verifies each generated proof with
the same `SP1CompressedVerifierRaw::verify_with_public_values` function used by
the WASM verifier and reports whether it accepts the proof.
