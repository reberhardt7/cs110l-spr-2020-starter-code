use crate::process::Process;
use nix::unistd::getuid;
use std::fmt;
use std::process::Command;

/// This enum represents the possible causes that an error might occur. It's useful because it
/// allows a caller of an API to have fine-grained control over error handling based on the
/// specifics of what went wrong. You'll find similar ideas in Rust libraries, such as std::io:
/// https://doc.rust-lang.org/std/io/enum.ErrorKind.html However, you won't need to do anything
/// with this (or like this) in your own code.
#[derive(Debug)]
pub enum Error {
    ExecutableError(std::io::Error),
    OutputFormatError(&'static str),
}

// Generate readable representations of Error
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self {
            Error::ExecutableError(err) => write!(f, "Error executing ps: {}", err),
            Error::OutputFormatError(err) => write!(f, "ps printed malformed output: {}", err),
        }
    }
}

// Make it possible to automatically convert std::io::Error to our Error type
impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Error {
        Error::ExecutableError(error)
    }
}

// Make it possible to automatically convert std::string::FromUtf8Error to our Error type
impl From<std::string::FromUtf8Error> for Error {
    fn from(_error: std::string::FromUtf8Error) -> Error {
        Error::OutputFormatError("Output is not utf-8")
    }
}

// Make it possible to automatically convert std::string::ParseIntError to our Error type
impl From<std::num::ParseIntError> for Error {
    fn from(_error: std::num::ParseIntError) -> Error {
        Error::OutputFormatError("Error parsing integer")
    }
}

/// This function takes a line of ps output formatted with -o "pid= ppid= command=" and returns a
/// Process struct initialized from the parsed output.
///
/// Example line:
/// "  578   577 emacs inode.c"
fn parse_ps_line(line: &str) -> Result<Process, Error> {
    // ps doesn't output a very nice machine-readable output, so we do some wonky things here to
    // deal with variable amounts of whitespace.
    let mut remainder = line.trim();
    let first_token_end = remainder
        .find(char::is_whitespace)
        .ok_or(Error::OutputFormatError("Missing second column"))?;
    let pid = remainder[0..first_token_end].parse::<usize>()?;
    remainder = remainder[first_token_end..].trim_start();
    let second_token_end = remainder
        .find(char::is_whitespace)
        .ok_or(Error::OutputFormatError("Missing third column"))?;
    let ppid = remainder[0..second_token_end].parse::<usize>()?;
    remainder = remainder[second_token_end..].trim_start();
    Ok(Process::new(pid, ppid, String::from(remainder)))
}

/// This function takes a pid and returns a Process struct for the specified process, or None if
/// the specified pid doesn't exist. An Error is only returned if ps cannot be executed or
/// produces unexpected output format.
fn get_process(pid: usize) -> Result<Option<Process>, Error> {
    // Run ps to find the specified pid. We use the ? operator to return an Error if executing ps
    // fails, or if it returns non-utf-8 output. (The extra Error traits above are used to
    // automatically convert errors like std::io::Error or std::string::FromUtf8Error into our
    // custom error type.)
    let output = String::from_utf8(
        Command::new("ps")
            .args(&["--pid", &pid.to_string(), "-o", "pid= ppid= command="])
            .output()?
            .stdout,
    )?;
    // Return Some if the process was found and output parsing succeeds, or None if ps produced no
    // output (indicating there is no matching process). Note the use of ? to propagate Error if an
    // error occured in parsing the output.
    if output.trim().len() > 0 {
        Ok(Some(parse_ps_line(output.trim())?))
    } else {
        Ok(None)
    }
}

/// This function takes a pid and returns a list of Process structs for processes that have the
/// specified pid as their parent process. An Error is returned if ps cannot be executed or
/// produces unexpected output format.
#[allow(unused)] // TODO: delete this line for Milestone 5
pub fn get_child_processes(pid: usize) -> Result<Vec<Process>, Error> {
    let ps_output = Command::new("ps")
        .args(&["--ppid", &pid.to_string(), "-o", "pid= ppid= command="])
        .output()?;
    let mut output = Vec::new();
    for line in String::from_utf8(ps_output.stdout)?.lines() {
        output.push(parse_ps_line(line)?);
    }
    Ok(output)
}

/// This function takes a command name (e.g. "sort" or "./multi_pipe_test") and returns the first
/// matching process's pid, or None if no matching process is found. It returns an Error if there
/// is an error running pgrep or parsing pgrep's output.
fn get_pid_by_command_name(name: &str) -> Result<Option<usize>, Error> {
    let output = String::from_utf8(
        Command::new("pgrep")
            .args(&["-xU", getuid().to_string().as_str(), name])
            .output()?
            .stdout,
    )?;
    Ok(match output.lines().next() {
        Some(line) => Some(line.parse::<usize>()?),
        None => None,
    })
}

/// This program finds a target process on the system. The specified query can either be a
/// command name (e.g. "./subprocess_test") or a PID (e.g. "5612"). This function returns a
/// Process struct if the specified process was found, None if no matching processes were found, or
/// Error if an error was encountered in running ps or pgrep.
pub fn get_target(query: &str) -> Result<Option<Process>, Error> {
    let pid_by_command = get_pid_by_command_name(query)?;
    if pid_by_command.is_some() {
        return get_process(pid_by_command.unwrap());
    }
    // If searching for the query as a command name failed, let's see if it's a valid pid
    match query.parse() {
        Ok(pid) => return get_process(pid),
        Err(_) => return Ok(None),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::process::Child;

    fn start_c_program(program: &str) -> Child {
        Command::new(program)
            .spawn()
            .expect(&format!("Could not find {}. Have you run make?", program))
    }

    #[test]
    fn test_get_target_success() {
        let mut subprocess = start_c_program("./multi_pipe_test");
        let found = get_target("multi_pipe_test")
            .expect("Passed valid \"multi_pipe_test\" to get_target, but it returned an error")
            .expect("Passed valid \"multi_pipe_test\" to get_target, but it returned None");
        assert_eq!(found.command, "./multi_pipe_test");
        let _ = subprocess.kill();
    }

    #[test]
    fn test_get_target_invalid_command() {
        let found = get_target("asdflksadfasdf")
            .expect("get_target returned an error, even though ps and pgrep should be working");
        assert!(
            found.is_none(),
            "Passed invalid target to get_target, but it returned Some"
        );
    }

    #[test]
    fn test_get_target_invalid_pid() {
        let found = get_target("1234567890")
            .expect("get_target returned an error, even though ps and pgrep should be working");
        assert!(
            found.is_none(),
            "Passed invalid target to get_target, but it returned Some"
        );
    }
}
