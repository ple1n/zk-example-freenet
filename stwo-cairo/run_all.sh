#!/usr/bin/env bash
set -euo pipefail

DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
cd "$DIR"

PROFILE=${1:-full}
if (($# > 1)); then
    echo "Usage: bash stwo-cairo/run_all.sh [full|--dev]" >&2
    exit 2
fi

case "$PROFILE" in
    full)
        DEV_PROFILE=false
        ;;
    --dev)
        DEV_PROFILE=true
        ;;
    *)
        echo "Usage: bash stwo-cairo/run_all.sh [full|--dev]" >&2
        exit 2
        ;;
esac

bash "$DIR/setup.sh"
mkdir -p "$DIR/artifacts"
printf '{"secret":"12345"}\n' > "$DIR/artifacts/hash-input.json"
printf '{"leaf":"123","sibling_0":"456","sibling_1":"789","bit_0":"0","bit_1":"1"}\n' \
    > "$DIR/artifacts/merkle-input.json"

run_experiment() {
    local name=$1
    local program=$2
    local input=$3
    local candidate="$DIR/artifacts/$name-candidate.json"
    local parameters="$DIR/artifacts/$name-contract-parameters.json"
    local -a prove_args=(
        run --release --manifest-path "$DIR/Cargo.toml" -p stwo-cairo-cli -- prove
        --program "$DIR/build/$program.json"
        --input "$input"
        --output "$candidate"
        --proof "$DIR/artifacts/$name-proof.bin"
    )

    if [[ "$DEV_PROFILE" == true ]]; then
        prove_args+=(--prover-parameters "$DIR/dev-prover-parameters.json")
    fi
    cargo "${prove_args[@]}"
    cargo run --release --manifest-path "$DIR/Cargo.toml" -p stwo-cairo-cli -- setup \
        --candidate "$candidate" \
        --output "$parameters"
    cargo run --release --manifest-path "$DIR/Cargo.toml" -p stwo-cairo-cli -- verify \
        --candidate "$candidate" \
        --parameters "$parameters"
}

run_experiment hash poseidon_hash "$DIR/artifacts/hash-input.json"
run_experiment merkle poseidon_merkle "$DIR/artifacts/merkle-input.json"
