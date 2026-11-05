#!/usr/bin/env bash
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
source setup.sh
cargo run --release --manifest-path Cargo.toml -p sp1-keccak-client -- prove
cargo run --release --manifest-path Cargo.toml -p sp1-keccak-client -- verify
