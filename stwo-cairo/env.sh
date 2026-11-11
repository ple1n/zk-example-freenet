#!/usr/bin/env bash

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    echo "Source this file: source stwo-cairo/env.sh" >&2
    exit 1
fi

STWO_CAIRO_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
export STWO_CAIRO_DIR
