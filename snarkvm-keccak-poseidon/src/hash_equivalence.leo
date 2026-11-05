program hash_equivalence.aleo {
    fn prove_equivalence(
        private secret: field,
        public expected_keccak: field,
        public expected_poseidon: field,
    ) -> (public field, public field) {
        let keccak_pubkey: field = Keccak256::hash_to_field(secret);
        let poseidon_pubkey: field = Poseidon2::hash_to_field(secret);
        assert(keccak_pubkey == expected_keccak);
        assert(poseidon_pubkey == expected_poseidon);
        return (keccak_pubkey, poseidon_pubkey);
    }

    @noupgrade
    constructor() {}
}
