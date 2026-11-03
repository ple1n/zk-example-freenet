#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
source "$ROOT/scripts/common.sh"

secret="$ROOT/sp1/secret.bin"
parameters="$ROOT/sp1/contract-parameters.json"
proving_key="$ROOT/sp1/proving-key.bin"
candidate="$ROOT/sp1/candidate.json"

ensure_secret "$secret"
run_stage "SP1 setup" cargo run --release --manifest-path "$ROOT/sp1/Cargo.toml" \
    -p sp1-notepad-client -- setup --secret "$secret" --output "$parameters" \
    --proving-key "$proving_key"
run_stage "SP1 prove" cargo run --release --manifest-path "$ROOT/sp1/Cargo.toml" \
    -p sp1-notepad-client -- prove --secret "$secret" --parameters "$parameters" \
    --proving-key "$proving_key" --counter 1 --text "Experiment workflow test" \
    --output "$candidate"
run_stage "SP1 verify" cargo run --release --manifest-path "$ROOT/sp1/Cargo.toml" \
    -p sp1-notepad-client -- verify --parameters "$parameters" --candidate "$candidate"

tree_secret="$ROOT/sp1/merkle-tree-secret.bin"
tree_parameters="$ROOT/sp1/merkle-contract-parameters.json"
tree_proving_key="$ROOT/sp1/merkle-proving-key.bin"
tree_candidate="$ROOT/sp1/merkle-candidate.json"

ensure_secret "$tree_secret"
run_stage "SP1 Merkle setup" cargo run --release --manifest-path "$ROOT/sp1/Cargo.toml" \
    -p sp1-merkle-client -- setup --tree-secret "$tree_secret" --depth 16 \
    --output "$tree_parameters" --proving-key "$tree_proving_key"
run_stage "SP1 Merkle prove" cargo run --release --manifest-path "$ROOT/sp1/Cargo.toml" \
    -p sp1-merkle-client -- prove --tree-secret "$tree_secret" \
    --parameters "$tree_parameters" --proving-key "$tree_proving_key" \
    --leaf-index 42 --output "$tree_candidate"
run_stage "SP1 Merkle verify" cargo run --release --manifest-path "$ROOT/sp1/Cargo.toml" \
    -p sp1-merkle-client -- verify --parameters "$tree_parameters" \
    --candidate "$tree_candidate"
