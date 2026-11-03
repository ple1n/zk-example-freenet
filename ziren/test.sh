#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
source "$ROOT/scripts/common.sh"

zkm_env="${ZKM_ENV:-$HOME/.zkm-toolchain/env}"
if [[ ! -r "$zkm_env" ]]; then
    echo "Ziren toolchain environment not found at $zkm_env (set ZKM_ENV to override)." >&2
    exit 1
fi
# Load Ziren's toolchain only in this experiment process; SP1 and snarkVM keep
# their own toolchains when the collection runner proceeds to the next script.
source "$zkm_env"

secret="$ROOT/ziren/secret.bin"
parameters="$ROOT/ziren/contract-parameters.json"
proving_key="$ROOT/ziren/notepad-proving-key.bin"
candidate="$ROOT/ziren/candidate.json"

ensure_secret "$secret"
run_stage "Ziren setup" cargo run --release --manifest-path "$ROOT/ziren/Cargo.toml" \
    -p notepad-client -- setup --secret "$secret" --output "$parameters" \
    --proving-key "$proving_key"
run_stage "Ziren prove" cargo run --release --manifest-path "$ROOT/ziren/Cargo.toml" \
    -p notepad-client -- prove --secret "$secret" --parameters "$parameters" \
    --proving-key "$proving_key" --counter 1 --text "Experiment workflow test" \
    --output "$candidate"
run_stage "Ziren verify" cargo run --release --manifest-path "$ROOT/ziren/Cargo.toml" \
    -p notepad-client -- verify --parameters "$parameters" --candidate "$candidate"
