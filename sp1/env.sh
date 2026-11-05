#!/usr/bin/env bash

export DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
export SP1_MERKLE_DEPTH=${SP1_MERKLE_DEPTH:-16}
