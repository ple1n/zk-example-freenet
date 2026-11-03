#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
source "$ROOT/scripts/common.sh"

run_stage "snarkVM compile, execute, prove, and verify" \
    cargo run --release --manifest-path "$ROOT/snarkvm/Cargo.toml" -- 7field
