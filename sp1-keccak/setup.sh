#!/usr/bin/env bash
set -euo pipefail

DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
cargo run --release --manifest-path "$DIR/Cargo.toml" -p sp1-keccak-client -- setup
