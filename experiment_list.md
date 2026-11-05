# Experiment list

The experiments are independent projects under this repository. Run commands
from the repository root. Each experiment has its own `setup.sh` for preparing
artifacts and `run_all.sh` for its complete workflow. Output is printed directly
to the terminal.

| Experiment | What it demonstrates | Setup | Full run |
|---|---|---|---|
| [snarkvm](./snarkvm/) | Compile a Leo hash-checking program and prove and verify its execution with snarkVM. | `bash snarkvm/setup.sh` | `bash snarkvm/run_all.sh` |
| [snarkvm-keccak-poseidon](./snarkvm-keccak-poseidon/) | Prove that one private field secret yields both its Keccak-256 and Poseidon public keys; exercises trusted and adversarial verification. | `source snarkvm-keccak-poseidon/env.sh`<br>`bash snarkvm-keccak-poseidon/setup.sh` | `bash snarkvm-keccak-poseidon/run_all.sh` |
| [sp1](./sp1/) | SP1 notepad proof and Poseidon2 Merkle-membership proof, each with a separate contract and parameters. | `source sp1/env.sh`<br>`bash sp1/setup.sh` | `bash sp1/run_all.sh` |
| [sp1-keccak](./sp1-keccak/) | Prove and verify a guest that computes Keccak-256 and SP1 Poseidon2 hashes of one secret. | `bash sp1-keccak/setup.sh` | `bash sp1-keccak/run_all.sh` |
| [zisk-poseidon2](./zisk-poseidon2/) | Prove one fixed-input Goldilocks Poseidon2 hash using Zisk's precompile. | `source zisk-poseidon2/env.sh`<br>`bash zisk-poseidon2/setup.sh` | `bash zisk-poseidon2/run_all.sh` |
| [ziren](./ziren/) | Ziren STARK proof for a single-note register authenticated by Poseidon2. | `source ziren/env.sh`<br>`bash ziren/setup.sh` | `bash ziren/run_all.sh` |

For Fish shells, use `env.fish` instead of `env.sh` where provided. Environment
scripts configure the current shell without generating proof artifacts. To run
all experiments sequentially, use:

```sh
bash scripts/run-experiments.sh
```
