#!/usr/bin/env bash

run_stage() {
    local name="$1"
    shift
    local started
    started=$(date +%s)
    printf '\n== %s ==\n' "$name"
    "$@"
    printf '== %s finished in %ss ==\n' "$name" "$(( $(date +%s) - started ))"
}

ensure_secret() {
    local path="$1"
    if [[ ! -s "$path" ]]; then
        mkdir -p "$(dirname "$path")"
        (umask 077; head -c 32 /dev/urandom > "$path")
        printf 'Generated local test secret: %s\n' "$path"
    fi
}
