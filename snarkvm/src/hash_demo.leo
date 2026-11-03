program leo_demo.aleo {
    fn prove_preimage(
        private preimage: field,
        public expected_hash: field,
    ) -> public field {
        let computed: field = Poseidon2::hash_to_field(preimage);
        assert(computed == expected_hash);
        return computed;
    }

    @noupgrade
    constructor() {}
}
