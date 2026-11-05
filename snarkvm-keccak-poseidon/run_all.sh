#!/usr/bin/env bash
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
source env.sh
source setup.sh

cargo run --release --manifest-path Cargo.toml -- prove 7field --keccak-pubkey "$KECCAK_PUBKEY" --poseidon-pubkey "$POSEIDON_PUBKEY"
cargo run --release --manifest-path Cargo.toml -- prove 7field --wrong-program --keccak-pubkey "$KECCAK_PUBKEY" --poseidon-pubkey "$POSEIDON_PUBKEY"
cargo run --release --manifest-path Cargo.toml -- verify --proof execution-proof.bin --keccak-pubkey "$KECCAK_PUBKEY" --poseidon-pubkey "$POSEIDON_PUBKEY"

if cargo run --release --manifest-path Cargo.toml -- verify --keccak-pubkey "$KECCAK_PUBKEY" --poseidon-pubkey "$POSEIDON_PUBKEY" --proof execution-proof.bin --simulate-wrong-key; then
    echo "Verifier unexpectedly accepted the proof under the wrong key." >&2
    exit 1
else
    echo "Verifier rejected the wrong-key proof as expected."
fi

if cargo run --release --manifest-path Cargo.toml -- verify --keccak-pubkey "$KECCAK_PUBKEY" --poseidon-pubkey "$POSEIDON_PUBKEY" --proof wrong-execution-proof.bin; then
    echo "Verifier unexpectedly accepted a proof from the wrong program." >&2
    exit 1
else
    echo "Verifier rejected the wrong-program proof as expected."
fi
