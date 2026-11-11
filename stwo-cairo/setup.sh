#!/usr/bin/env bash
set -euo pipefail

DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
cd "$DIR"

if ! command -v python3.12 >/dev/null 2>&1; then
    echo "Python 3.12 is required to install cairo-lang 0.14.0.1." >&2
    exit 1
fi

python3.12 -m venv .venv
.venv/bin/python -m pip install -r requirements.txt
rustup target add wasm32-unknown-unknown

mkdir -p build
.venv/bin/cairo-compile --proof_mode programs/poseidon_hash.cairo \
    --output build/poseidon_hash.json
.venv/bin/cairo-compile --proof_mode programs/poseidon_merkle.cairo \
    --output build/poseidon_merkle.json

echo "Cairo programs compiled into $DIR/build"
