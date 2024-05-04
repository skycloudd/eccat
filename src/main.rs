#![deny(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::std_instead_of_core)]
#![warn(clippy::alloc_instead_of_core)]

use eccat::Engine;
use std::process::ExitCode;

fn main() -> ExitCode {
    match Engine::new().main_loop() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;
