use std::error::Error;

use zisk_sdk::{GuestProgram, ProverClient, ZiskStdin, load_program};
use ziskos::syscalls::syscall_poseidon2;

static PROGRAM: GuestProgram = load_program!("zisk-poseidon2-guest");
const INPUT: u64 = 7;

fn main() -> Result<(), Box<dyn Error>> {
    let client = ProverClient::embedded().build()?;
    let mut state = [0u64; 16];
    state[0] = INPUT;
    unsafe {
        syscall_poseidon2(&mut state);
    }
    let expected = state[0];

    let stdin = ZiskStdin::new();
    stdin.write(&INPUT);
    client.setup(&PROGRAM).run_sync()?;
    let proof = client.prove(&PROGRAM, stdin).run_sync()?;
    proof.with_program_vk(&PROGRAM.vk()?).verify()?;

    let output = proof.get_publics().read::<u64>()?;
    assert_eq!(output, expected);
    println!("Verified one Goldilocks Poseidon2 hash: {output}");
    Ok(())
}
