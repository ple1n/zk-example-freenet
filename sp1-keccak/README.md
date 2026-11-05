# SP1 Keccak-256 experiment

The guest computes Keccak-256 and SP1 Poseidon2 hashes of the same private
32-byte secret, checks both against expected public keys, and commits both as
public outputs. This proves their equivalence through the shared secret. The
experiment invokes setup, prove, and verify as separate commands. It reports
proving-key setup, compressed
proof generation, verification, and proof size. The host reuses common SP1
helpers from `sp1/host-common`.

Run setup separately, or run the complete experiment from the collection root:

```sh
bash sp1-keccak/setup.sh
bash sp1-keccak/run_all.sh
```

The experiment has its own Cargo workspace and lockfile, and is automatically
discovered by `scripts/run-experiments.sh`. `run_all.sh` repeats setup and
prints output directly to the terminal. Its proving key is stored locally in this directory.
This is an SP1 guest/proof benchmark, not an R1CS constraint count.
