#!/usr/bin/env bash
set -euo pipefail

DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
source "$DIR/env.sh"

for secret in "$DIR/secret.bin" "$DIR/merkle-tree-secret.bin"; do
    if [[ ! -s "$secret" ]]; then
        (umask 077; head -c 32 /dev/urandom > "$secret")
    fi
done

cargo run --release --manifest-path "$DIR/Cargo.toml" -p sp1-notepad-client -- setup \
    --secret "$DIR/secret.bin" --output "$DIR/contract-parameters.json" \
    --proving-key "$DIR/proving-key.bin"
cargo run --release --manifest-path "$DIR/Cargo.toml" -p sp1-merkle-client -- setup \
    --tree-secret "$DIR/merkle-tree-secret.bin" --depth "$SP1_MERKLE_DEPTH" \
    --output "$DIR/merkle-contract-parameters.json" \
    --proving-key "$DIR/merkle-proving-key.bin"
