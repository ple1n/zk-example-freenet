#!/usr/bin/env bash
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
source setup.sh

cargo run --release --manifest-path Cargo.toml -p sp1-notepad-client -- prove \
    --secret secret.bin --parameters contract-parameters.json \
    --proving-key proving-key.bin --counter 1 \
    --text "Experiment workflow test" --output candidate.json
cargo run --release --manifest-path Cargo.toml -p sp1-notepad-client -- verify \
    --parameters contract-parameters.json --candidate candidate.json

cargo run --release --manifest-path Cargo.toml -p sp1-merkle-client -- prove \
    --tree-secret merkle-tree-secret.bin \
    --parameters merkle-contract-parameters.json \
    --proving-key merkle-proving-key.bin --leaf-index 42 \
    --output merkle-candidate.json
cargo run --release --manifest-path Cargo.toml -p sp1-merkle-client -- verify \
    --parameters merkle-contract-parameters.json \
    --candidate merkle-candidate.json
