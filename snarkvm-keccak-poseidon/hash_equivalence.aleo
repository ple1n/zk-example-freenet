program hash_equivalence.aleo;

function prove_equivalence:
    input r0 as field.private;
    input r1 as field.public;
    input r2 as field.public;
    hash.keccak256 r0 into r3 as field;
    hash.psd2 r0 into r4 as field;
    is.eq r3 r1 into r5;
    assert.eq r5 true;
    is.eq r4 r2 into r6;
    assert.eq r6 true;
    output r3 as field.public;
    output r4 as field.public;

constructor:
    assert.eq edition 0u16;
