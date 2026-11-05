#!/usr/bin/env bash
set -euo pipefail

DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
source "$DIR/env.sh"
cargo build --release --manifest-path "$DIR/Cargo.toml" -p zisk-poseidon2-host
