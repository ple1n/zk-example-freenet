#!/usr/bin/env bash
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
source setup.sh
cargo run --release --manifest-path Cargo.toml -p zisk-poseidon2-host
