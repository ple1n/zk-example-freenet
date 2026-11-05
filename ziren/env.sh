#!/usr/bin/env bash

DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
export ZKM_ENV=${ZKM_ENV:-$HOME/.zkm-toolchain/env}
if [[ ! -r "$ZKM_ENV" ]]; then
    echo "Ziren toolchain environment not found at $ZKM_ENV (set ZKM_ENV to override)." >&2
    return 1
fi
source "$ZKM_ENV"
export ZKM_ENV
