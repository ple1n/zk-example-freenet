#!/usr/bin/env bash
set -euo pipefail

DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
CLIENT=(cargo run --release --manifest-path "$DIR/Cargo.toml" --)

"${CLIENT[@]}" setup
"${CLIENT[@]}" setup --wrong-program
