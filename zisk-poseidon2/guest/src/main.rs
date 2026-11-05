#![no_main]
ziskos::entrypoint!(main);

use ziskos::syscalls::syscall_poseidon2;

fn main() {
    let input: u64 = ziskos::io::read();
    let mut state = [0u64; 16];
    state[0] = input;

    unsafe {
        syscall_poseidon2(&mut state);
    }

    ziskos::io::commit(&state[0]);
    println!("Poseidon2 output: {}", state[0]);
}
