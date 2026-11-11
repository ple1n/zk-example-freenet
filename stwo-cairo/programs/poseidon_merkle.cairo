%builtins output pedersen range_check ecdsa bitwise ec_op keccak poseidon range_check96 add_mod mul_mod

from starkware.cairo.common.builtin_poseidon.poseidon import poseidon_hash
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
    %{ memory[ap] = program_input['leaf'] %}
    tempvar leaf = [ap];
    %{ memory[ap] = program_input['sibling_0'] %}
    tempvar sibling_0 = [ap];
    %{ memory[ap] = program_input['sibling_1'] %}
    tempvar sibling_1 = [ap];
    %{ memory[ap] = program_input['bit_0'] %}
    tempvar bit_0 = [ap];
    %{ memory[ap] = program_input['bit_1'] %}
    tempvar bit_1 = [ap];

    assert bit_0 * (bit_0 - 1) = 0;
    assert bit_1 * (bit_1 - 1) = 0;

    let left_0 = leaf + bit_0 * (sibling_0 - leaf);
    let right_0 = sibling_0 + bit_0 * (leaf - sibling_0);
    let (node_0) = poseidon_hash(left_0, right_0);

    let left_1 = node_0 + bit_1 * (sibling_1 - node_0);
    let right_1 = sibling_1 + bit_1 * (node_0 - sibling_1);
    let (result) = poseidon_hash(left_1, right_1);

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
