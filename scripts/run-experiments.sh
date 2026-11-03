#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
source "$ROOT/scripts/common.sh"
cd "$ROOT"

include_contract_benchmarks=0
if [[ "${1:-}" == "--include-contract-benchmarks" ]]; then
    include_contract_benchmarks=1
    shift
fi
if [[ "$#" -ne 0 ]]; then
    echo "Usage: $0 [--include-contract-benchmarks]" >&2
    exit 2
fi

for experiment in ziren sp1 snarkvm; do
    printf '\n######## %s experiment ########\n' "$experiment"
    run_script="$ROOT/$experiment/test.sh"
    if [[ ! -f "$run_script" ]]; then
        echo "Missing experiment runner: $run_script" >&2
        exit 1
    fi
    bash "$run_script"

    if (( include_contract_benchmarks )); then
        benchmark_script="$ROOT/$experiment/contract-benchmark.sh"
        if [[ -f "$benchmark_script" ]]; then
            bash "$benchmark_script"
        else
            printf 'No contract benchmark configured for %s; skipping.\n' "$experiment"
        fi
    fi
done
