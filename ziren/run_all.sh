#!/usr/bin/env bash
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
source env.sh
source setup.sh

cargo run --release --manifest-path Cargo.toml -p notepad-client -- prove \
    --secret secret.bin --parameters contract-parameters.json \
    --proving-key notepad-proving-key.bin --counter 1 \
    --text "Experiment workflow test" --output candidate.json
cargo run --release --manifest-path Cargo.toml -p notepad-client -- verify \
    --parameters contract-parameters.json --candidate candidate.json
