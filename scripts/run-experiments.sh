#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"
export LC_ALL=C

include_contract_benchmarks=0
if [[ "${1:-}" == "--include-contract-benchmarks" ]]; then
    include_contract_benchmarks=1
    shift
fi
if [[ "$#" -ne 0 ]]; then
    echo "Usage: $0 [--include-contract-benchmarks]" >&2
    exit 2
fi

shopt -s nullglob
run_scripts=("$ROOT"/*/run_all.sh)
if (("${#run_scripts[@]}" == 0)); then
    echo "No experiment run_all.sh scripts found." >&2
    exit 1
fi

for run_script in "${run_scripts[@]}"; do
    experiment=$(basename "$(dirname "$run_script")")
    printf '\n######## %s experiment ########\n' "$experiment"
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
