use std::env;

mod open_file;
mod process;
mod ps_utils;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: {} <name or pid of target>", args[0]);
        std::process::exit(1);
    }
    #[allow(unused)] // TODO: delete this line for Milestone 1
    let target = &args[1];

    // TODO: Milestone 1: Get the target Process using psutils::get_target()
    unimplemented!();
}

#[cfg(test)]
mod test {
    use std::process::{Child, Command};

    fn start_c_program(program: &str) -> Child {
        Command::new(program)
            .spawn()
            .expect(&format!("Could not find {}. Have you run make?", program))
    }

    #[test]
    fn test_exit_status_valid_target() {
        let mut subprocess = start_c_program("./multi_pipe_test");
        assert_eq!(
            Command::new("./target/debug/inspect-fds")
                .args(&[&subprocess.id().to_string()])
                .status()
                .expect("Could not find target/debug/inspect-fds. Is the binary compiled?")
                .code()
                .expect("Program was unexpectedly terminated by a signal"),
            0,
            "We expected the program to exit normally, but it didn't."
        );
        let _ = subprocess.kill();
    }

    #[test]
    fn test_exit_status_invalid_target() {
        assert_eq!(
            Command::new("./target/debug/inspect-fds")
                .args(&["./nonexistent"])
                .status()
                .expect("Could not find target/debug/inspect-fds. Is the binary compiled?")
                .code()
                .expect("Program was unexpectedly terminated by a signal"),
            1,
            "Program exited with unexpected return code. Make sure you handle the case where \
            ps_utils::get_target returns None and print an error message and return status \
            1."
        );
    }
}
