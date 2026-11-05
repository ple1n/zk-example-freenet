#!/usr/bin/env bash
set -euo pipefail

DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
secret="$DIR/secret.bin"
if [[ ! -s "$secret" ]]; then
    (umask 077; head -c 32 /dev/urandom > "$secret")
fi

cargo run --release --manifest-path "$DIR/Cargo.toml" -p notepad-client -- setup \
    --secret "$secret" --output "$DIR/contract-parameters.json" \
    --proving-key "$DIR/notepad-proving-key.bin"
