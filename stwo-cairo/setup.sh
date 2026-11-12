#!/usr/bin/env bash
set -euo pipefail

DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
cd "$DIR"

SCARB_BIN=${SCARB_BIN:-$(command -v scarb || true)}
if [[ -z "$SCARB_BIN" && -x "$HOME/.asdf/shims/scarb" ]]; then
    SCARB_BIN="$HOME/.asdf/shims/scarb"
fi
if [[ -z "$SCARB_BIN" || ! -x "$SCARB_BIN" ]]; then
    echo "Scarb is required to work with the Cairo 1 project." >&2
    exit 1
fi

CORELIB_PATH=${CAIRO_CORELIB_PATH:-}
if [[ -n "$CORELIB_PATH" ]]; then
    if [[ ! -f "$CORELIB_PATH/lib.cairo" ]]; then
        echo "CAIRO_CORELIB_PATH must point to a directory containing lib.cairo." >&2
        exit 1
    fi
else
    if ! command -v jq >/dev/null 2>&1; then
        echo "jq is required to resolve Scarb's managed corelib." >&2
        exit 1
    fi
    if ! CORELIB_PATH=$(
        cd "$DIR/cairo"
        "$SCARB_BIN" metadata --format-version 1 |
            jq -er '.packages[] | select(.name == "core" and .source == "std") | .root + "/src"'
    ); then
        echo "Could not resolve the Cairo corelib from Scarb metadata." >&2
        exit 1
    fi
    if [[ ! -f "$CORELIB_PATH/lib.cairo" ]]; then
        echo "Scarb's corelib was not found at $CORELIB_PATH." >&2
        exit 1
    fi
fi

rustup target add wasm32-unknown-unknown

mkdir -p build
cargo run --release --manifest-path Cargo.toml -p stwo-cairo-cli -- compile-cairo1 \
    --source cairo/src/poseidon_hash.cairo \
    --corelib "$CORELIB_PATH" \
    --program poseidon_hash \
    --output build/poseidon_hash.json
cargo run --release --manifest-path Cargo.toml -p stwo-cairo-cli -- compile-cairo1 \
    --source cairo/src/poseidon_merkle.cairo \
    --corelib "$CORELIB_PATH" \
    --program poseidon_merkle \
    --output build/poseidon_merkle.json

echo "Cairo 1 executables compiled into $DIR/build"
