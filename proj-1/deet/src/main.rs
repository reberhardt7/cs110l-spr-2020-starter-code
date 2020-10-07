mod debugger;
mod debugger_command;
mod inferior;

use crate::debugger::Debugger;
use nix::sys::signal::{signal, SigHandler, Signal};
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: {} <target program>", args[0]);
        std::process::exit(1);
    }
    let target = &args[1];

    // Disable handling of ctrl+c in this process (so that ctrl+c only gets delivered to child
    // processes)
    unsafe { signal(Signal::SIGINT, SigHandler::SigIgn) }.expect("Error disabling SIGINT handling");

    Debugger::new(target).run();
}
