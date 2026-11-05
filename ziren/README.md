# Ziren experiment

This directory contains the Ziren notepad workspace and its Freenet contract
sources. The collection-level [README](../README.md) documents setup, proof
generation, verification, and contract publishing.

Configure the Ziren toolchain separately, then run setup or the complete
workflow:

```sh
source ziren/env.sh
bash ziren/setup.sh
bash ziren/run_all.sh
```

`run_all.sh` repeats setup before proving and verifying and creates a local
secret if needed. Output is printed directly to the terminal. Parameters, proving key, and candidate
artifacts stay inside this directory. To add a contract benchmark, provide
`contract-benchmark.sh` here; the collection runner can invoke it with
`--include-contract-benchmarks`.
