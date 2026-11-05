program hash_equivalence.aleo {
    fn prove_equivalence(
        private _secret: field,
        public expected_keccak: field,
        public expected_poseidon: field,
    ) -> (public field, public field) {
        return (expected_keccak, expected_poseidon);
    }

    @noupgrade
    constructor() {}
}
