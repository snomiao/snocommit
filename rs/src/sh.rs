//! Shell commands the way snorun runs them: `child_process.exec`, which is
//! `/bin/sh -c <command>` with stdin, stdout and stderr as pipes, the exit
//! code ignored, and (snorun's `pipe`) stdout and stderr passed through as
//! they come.
//!
//! The port runs the very same command lines through the very same shell, so
//! whatever the shell makes of them (word splitting, globbing a `??`, a `#`)
//! is what it made of them before. What the shell would run or redirect is
//! never given to it: `commit::shell_safe` hands such a command back first.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::process::{Command, Stdio};

/// What a command wrote, and how it ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// None when it could not be started, or a signal ended it.
    pub code: Option<i32>,
}

/// Runs `command` with these variables added to (or, for `None`, taken out
/// of) the environment. With `forward`, its stdout and stderr go on to this
/// process's own as they arrive, and are also kept.
pub fn run(command: &str, extra: &[(&str, Option<&OsString>)], forward: bool) -> Output {
    let mut child = Command::new("/bin/sh");
    child.arg("-c").arg(command).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    // The JavaScript is started without it (js_fallback::run), and so are
    // the commands it runs.
    child.env_remove(crate::js_fallback::TRACE_ENV);
    for (name, value) in extra {
        match value {
            Some(value) => child.env(name, value),
            None => child.env_remove(name),
        };
    }
    let mut child = match child.spawn() {
        Ok(child) => child,
        // exec's callback gets the error, and snorun goes on with nothing.
        Err(_) => return Output { stdout: Vec::new(), stderr: Vec::new(), code: None },
    };
    // node never writes to the child's stdin and does not close it while the
    // child runs: a program that reads it waits, as it did there.
    let stdin = child.stdin.take();
    let out = child.stdout.take().map(|pipe| std::thread::spawn(move || pump(pipe, forward.then(std::io::stdout))));
    let err = child.stderr.take().map(|pipe| std::thread::spawn(move || pump(pipe, forward.then(std::io::stderr))));
    let stdout = out.map(|t| t.join().unwrap_or_default()).unwrap_or_default();
    let stderr = err.map(|t| t.join().unwrap_or_default()).unwrap_or_default();
    let status = child.wait();
    drop(stdin);
    Output { stdout, stderr, code: status.ok().and_then(|s| s.code()) }
}

/// Reads `pipe` to its end, passing every piece on to `to` at once, and
/// returns all of it. A reader that has gone away is not an error.
fn pump(mut pipe: impl Read, mut to: Option<impl Write>) -> Vec<u8> {
    let mut all = Vec::new();
    let mut buf = [0u8; 65536];
    loop {
        match pipe.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if let Some(to) = to.as_mut() {
                    let _ = to.write_all(&buf[..n]);
                    let _ = to.flush();
                }
                all.extend_from_slice(&buf[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    all
}

/// Writes `text` to stdout now, as `console.log` does. A reader that has
/// gone away is not an error.
pub fn print(text: &str) {
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(text.as_bytes());
    let _ = out.flush();
}

/// The same for stderr.
pub fn eprint(text: &str) {
    let mut err = std::io::stderr().lock();
    let _ = err.write_all(text.as_bytes());
    let _ = err.flush();
}
