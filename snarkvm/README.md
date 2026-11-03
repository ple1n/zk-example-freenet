# Leo-to-snarkVM proof

This example compiles [hash_demo.leo](src/hash_demo.leo) into Aleo
bytecode using the Rust API from the sibling `leo` checkout, then uses a Rust
host with snarkVM path dependencies to execute and prove that compiled program.
It uses in-memory storage and does not start an Aleo node or require a ledger
service.

Run it from this repository root, supplying the preimage and optionally the
expected hash as field literals:

```sh
cargo run --release --manifest-path snarkvm/Cargo.toml -- \
  7field <expected-hash.field>
```

Omit the second argument to use the native snarkVM hash of the supplied
preimage as the public expected value. This is convenient for the automated
experiment runner; pass an explicit value to test a particular public claim.

The Leo function accepts a private preimage and a public expected hash, hashes
the preimage with `Poseidon2::hash_to_field`, and asserts that the result equals
the expected hash. The Rust host compiles the source in-process, runs the
program in snarkVM, generates a Varuna execution proof, and verifies it.
This demonstrates proving a Leo/Aleo program from a Rust host; it is not
execution or proving of an arbitrary Rust or SP1 guest binary.

The Leo and snarkVM Poseidon2 interface here is Aleo's Poseidon hash with input
rate 2. It is not SP1's Poseidon2 permutation / `Poseidon2ByteHash`; proofs and
hashes are not compatible with the SP1 experiment.

Both local checkouts must exist at `../leo` and `../snarkVM` relative to this
repository. The proof uses snarkVM's `TestRng` for demonstration; do not reuse
this RNG choice for production proofs.
