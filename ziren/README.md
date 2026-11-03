# Ziren experiment

This directory contains the Ziren notepad workspace and its Freenet contract
sources. The collection-level [README](../README.md) documents setup, proof
generation, verification, and contract publishing.

Run the complete setup, prove, and verify flow from the collection root with:

```sh
bash ziren/test.sh
```

The script creates a local secret if needed and keeps parameters, proving key,
and candidate artifacts inside this directory. To add a contract benchmark,
provide `contract-benchmark.sh` here; the collection runner can invoke it with
`--include-contract-benchmarks`.
