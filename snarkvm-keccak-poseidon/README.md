# snarkVM Keccak/Poseidon equivalence

This experiment proves that the same private field secret produces both a
Keccak-256 public key and a Poseidon public key. It uses Leo's built-in
`Keccak256::hash_to_field` and `Poseidon2::hash_to_field` relations and verifies
the resulting snarkVM execution proof. `run_all.sh` launches a prover process
that writes the serialized execution proof to `execution-proof.bin`, followed
by a fresh verifier process. The verifier has no access to the prover's memory or
secret. A separate setup process writes the execution verifying key. The
verifier process loads that key as its trusted verification key and verifies
the proof with it directly—there is no hash fingerprint. It then extracts the
public outputs from the proof and compares them with two independently
supplied expected public keys. The program ID is only a readable routing
identifier; the verification key itself is the trust anchor for the circuit.

In this Leo/snarkVM API, `Poseidon2` is Aleo Poseidon with input rate 2; it is
not the Poseidon2 permutation used by SP1. The proof demonstrates that one
secret underlies both public values, not that the hash values are equal.

Setup, prove, and verify run as separate processes. Copy the two public field
values printed by the prover into the verifier's expected-value arguments:

```sh
cargo run --release --manifest-path snarkvm-keccak-poseidon/Cargo.toml -- setup
cargo run --release --manifest-path snarkvm-keccak-poseidon/Cargo.toml -- prove 7field \
  --keccak-pubkey '<known-keccak-field>' \
  --poseidon-pubkey '<known-poseidon-field>'
cargo run --release --manifest-path snarkvm-keccak-poseidon/Cargo.toml -- verify \
  --keccak-pubkey '<known-keccak-field>' \
  --poseidon-pubkey '<known-poseidon-field>'
```

The verifier must trust/distribute `verifying-key.bin` independently of the
proof. Setup also writes the compiled trusted Aleo program to
`hash_equivalence.aleo`; verification loads this artifact without invoking
the Leo compiler. Distribute and pin both artifacts through trusted
configuration. The demo setup stage generates them locally. The run script
uses fixed expected public keys for its fixed `7field` test secret, so no
environment variables are required. Source `env.sh` to export the
`KECCAK_PUBKEY` and `POSEIDON_PUBKEY` defaults or set either value yourself
before running it.

The trusted circuit takes the expected hashes as public inputs and constrains
them to the secret's Keccak and Poseidon hashes. The adversarial circuit uses
the same inputs and outputs but returns the expected hashes without checking
the secret. The run script proves this weaker circuit using its own proving
key, so the public-output comparison succeeds; its proof must still fail under
the trusted key. `--simulate-wrong-key` separately installs the adversary key
to demonstrate rejection of the honest proof under the wrong key.
The verifier's `--proof` option selects the proof artifact; the adversarial
check passes `wrong-execution-proof.bin` explicitly.

Setup and proving compile the Leo source with the sibling `../leo` checkout;
verification loads the compiled Aleo program. The client links snarkVM from
`../snarkVM`. This uses the demonstration `TestRng`; it is not a production
proving setup.
