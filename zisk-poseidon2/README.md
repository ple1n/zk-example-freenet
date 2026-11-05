# Zisk Poseidon2 experiment

This experiment proves one fixed-input Goldilocks Poseidon2 hash using Zisk's
`syscall_poseidon2` precompile. The guest places the input in state lane zero,
zeros the remaining lanes, applies the width-16 permutation once, and commits
lane zero as the digest. This is a fixed one-shot hash convention, not a
variable-length sponge.

The Zisk checkout is expected at `../zisk` relative to this repository.
Sourcing `env.sh` sets `DIR` to this experiment directory.

```sh
source zisk-poseidon2/env.sh
bash zisk-poseidon2/setup.sh
bash zisk-poseidon2/run_all.sh
```

`setup.sh` builds the host and guest; `run_all.sh` repeats that build and proves
and verifies the guest, printing output directly to the terminal.
