# Proof of Secret: A Private-Key Authentication Method Across Disparate ZK Systems

## Thesis

Knowledge of a secret should be a portable root of identity across zero-knowledge
virtual machines and proof frameworks. A user should be able to authenticate by
proving control of a private identity secret without revealing it, while the
public hash, proof system, or virtual machine used for that authentication can
change over time.

The goal is not to make every VM use the same hash. It is to prove that
different public identifiers are bound to the same hidden secret, and to make
that relationship verifiable across otherwise incompatible systems.

There are problems sharing one same publickey (hash) across different ZK systems.
They tend to be of different native field, and this causes performance degradation. Engineering wise I want to avoid immature low-level cryptographic code.

## Identity model

Let `master_secret` be a uniformly random secret with enough entropy for the
security level. It is the root credential and is never published.

An application-specific identity is derived by an explicit, circuit-suitable
`arbitrary_derivation`:

```text
identity_secret = arbitrary_derivation(
    master_secret,
    "identity/v1" || application_context
)
identity_id     = H(identity_secret)
```

`arbitrary_derivation` names the explicit relation chosen for an identity and
proof system; the paradigm does not prescribe a KDF. The relation should be
deterministic, domain-separated, and practical to prove. Its exact algorithm
and encoding are part of the public protocol definition.

The context can include a service, protocol, chain, or identity purpose. A
different context produces a different identity secret and identifier. The
derivation must be designed so that publishing one identity does not expose the
master secret or provide a practical link to identities derived for other
contexts.

Strictly speaking, a hash digest is an identifier or commitment, not a
cryptographic public key. If an application needs a signing key, derive a
keypair from the identity secret and publish its public key separately. This
proposal uses “public key” informally for the public identity value derived
from a secret.

## Proof of secret

An authentication proof establishes knowledge of the private witness and the
relationship between that witness and the public identity:

```text
prove in zero knowledge:
    I know master_secret and identity_secret such that
    identity_secret = arbitrary_derivation(master_secret, context)
    public_identity = H(Encode(identity_secret))
```

The verifier learns the public identity and the agreed context, but not either
secret. Applications can add statements such as membership, authorization,
expiry, or a scoped nullifier without exposing the underlying credential.

## Equivalence across hashes and VMs

Suppose one system uses a Poseidon v1 hash and another uses SP1's accelerated
Poseidon2 or Keccak. Their hash values are not interchangeable merely because
the algorithms share a family name. A bridge proof must establish that both
public values come from the same private witness:

```text
prove in zero knowledge:
    I know identity_secret such that
    public_old = H_old(Encode(identity_secret))
    public_new = H_new(Encode(identity_secret))
```

The same pattern can bind a VM-specific derived identity to a stable,
versioned master commitment. A verifier in one ecosystem can rely on the
equivalence proof instead of reimplementing another ecosystem's hash inside
its own circuit.

This is the role SP1 could play: generate a bridge proof that computes the
required hash relations and binds their public outputs to the same secret. If a
legacy hash such as Poseidon v1 is not accelerated in SP1, it may still be
computable in the guest, but with a performance cost. Acceleration changes
cost, not the mathematical relation being proved.

“Future-proof” here means **hash agility**, not a proof that automatically
covers unknown future algorithms. When a new hash is adopted, a new relation
and proof implementation must be produced. Versioned algorithm identifiers,
canonical input encoding, domain separation, and published test vectors are
necessary so verifiers know exactly what was proved.

## Unlinkable derived identities

Equivalence proofs intentionally link the public values included in a single
proof. They should not accidentally link every identity to the master
credential. If identities must be unlinkable:

- derive each one with a distinct, explicit context;
- reveal only the identity-specific public value needed by that verifier;
- do not include a common master public value in every proof;
- use scoped nullifiers when repeat-use prevention is needed; and
- treat the proof transcript, context, and public inputs as potential
  linkability signals.

The protocol should make the choice between linkable migration and unlinkable
authentication explicit. A bridge that publishes both old and new identifiers
proves equivalence, and therefore links those two identifiers to each other.

## Quantum-security motivation and limits

Separating the secret from any single hash or proof system makes migration
possible if an assumption weakens. It does not by itself make the scheme
quantum-safe.

There are distinct assumptions to evaluate:

1. **Secret and hash security.** A quantum search can reduce the effective
   preimage security of an `n`-bit hash; output size and security margins must
   account for that.
2. **Proof-system security.** Pairing-based SNARKs and hash-based STARKs rely on
   different assumptions. A quantum threat to a proof system does not make the
   correct public identity impossible to determine; it can undermine the
   guarantee that a proof is sound.
3. **Arbitrary derivation and signatures.** The derivation relation and any
   signing-key algorithm need their own post-quantum analysis.

A system should avoid claiming quantum resistance solely because it uses SP1
or a hash-based proof. The security claim must specify the hash, proof system,
parameters, and threat model.

## Example migration

1. The user generates and backs up `master_secret`.
2. The user derives an application identity using a versioned context.
3. The current VM verifies a proof of knowledge for its supported identity
   hash.
4. During migration, an SP1 guest proves equivalence between the existing
   public identity and a new hash-based identity, with the secret kept private.
5. The receiving system verifies the bridge proof and records the new,
   versioned identity. Future proofs can use that identity without exposing the
   master secret.

The intended primitive is therefore not “one universal hash.” It is a
**proof-of-secret authentication layer** with explicit derivation rules and
zero-knowledge equivalence proofs that let identities move between hashes,
VMs, and proof systems.
