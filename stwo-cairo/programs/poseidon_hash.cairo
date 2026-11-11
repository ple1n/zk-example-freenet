%builtins output pedersen range_check ecdsa bitwise ec_op keccak poseidon range_check96 add_mod mul_mod

from starkware.cairo.common.builtin_poseidon.poseidon import poseidon_hash_single
from starkware.cairo.common.cairo_builtins import (
    BitwiseBuiltin,
    EcOpBuiltin,
    HashBuiltin,
    KeccakBuiltin,
    ModBuiltin,
    PoseidonBuiltin,
    SignatureBuiltin,
)

func main{
    output_ptr: felt*,
    pedersen_ptr: HashBuiltin*,
    range_check_ptr: felt*,
    ecdsa_ptr: SignatureBuiltin*,
    bitwise_ptr: BitwiseBuiltin*,
    ec_op_ptr: EcOpBuiltin*,
    keccak_ptr: KeccakBuiltin*,
    poseidon_ptr: PoseidonBuiltin*,
    range_check96_ptr: felt*,
    add_mod_ptr: ModBuiltin*,
    mul_mod_ptr: ModBuiltin*,
}() -> () {
    %{ memory[ap] = program_input['secret'] %}
    tempvar secret = [ap];
    let (result) = poseidon_hash_single(secret);
    %{ memory[ap] = (ids.result % 2**128) %}
    tempvar low = [ap];
    %{ memory[ap] = (ids.result // 2**128) %}
    tempvar high = [ap];
    assert [range_check_ptr] = low;
    let range_check_ptr = range_check_ptr + 1;
    assert [range_check_ptr] = high;
    let range_check_ptr = range_check_ptr + 1;
    [output_ptr] = low;
    let output_ptr = output_ptr + 1;
    [output_ptr] = high;
    let output_ptr = output_ptr + 1;
    return ();
}
