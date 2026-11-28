//! Shell commands the way snorun runs them: `child_process.exec`, which is
//! `/bin/sh -c <command>` with stdin, stdout and stderr as pipes, the exit
//! code ignored, and (snorun's `pipe`) stdout and stderr passed through as
//! they come.
//!
//! What exec does with the child's output, and so what users saw:
//! - it reads stdout and stderr as UTF-8 text (`setEncoding('utf8')`), so
//!   what is passed through is that text written out again: every byte that
//!   is not UTF-8 arrives as U+FFFD (EF BF BD), one for each ill-formed part,
//!   and a character cut between two reads is put back together;
//! - it counts what each of the two streams has given, in bytes of that
//!   text, and once one of them passes `maxBuffer` (1 MiB) it closes both
//!   pipes and sends the shell SIGTERM, then waits for the shell only. The
//!   piece that passed the limit is still passed through; nothing after it.
//!   Whatever the shell had started and is still writing then dies of a
//!   broken pipe the next time it writes (a hook that prints 3 MB: no commit).
//!
//! The port runs the very same command lines through the very same shell, so
//! whatever the shell makes of them (word splitting, globbing a `??`, a `#`)
//! is what it made of them before. What the shell would run or redirect is
//! never given to it: `commit::shell_safe` hands such a command back first.

use std::ffi::OsString;
use std::io::Write;
use std::process::{Command, Stdio};

/// exec's `maxBuffer`: how much of the decoded text of one stream a command
/// may write before it is killed (more than this, not this much).
pub const MAX_BUFFER: usize = 1024 * 1024;

/// What a command wrote (the bytes as they came), and how it ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// None when it could not be started, or a signal ended it.
    pub code: Option<i32>,
}

/// Runs `command` with these variables added to (or, for `None`, taken out
/// of) the environment. With `forward`, its stdout and stderr go on to this
/// process's own as they arrive, as exec's text; they are kept as bytes.
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
    let [stdout, stderr] = collect(&mut child, forward);
    let status = child.wait();
    drop(stdin);
    Output { stdout, stderr, code: status.ok().and_then(|s| s.code()) }
}

/// Reads the child's stdout and stderr as exec does (see the top of this
/// file), passing them on when `forward`, until both have ended or one has
/// passed `MAX_BUFFER`; in that case both pipes are closed and the shell is
/// sent SIGTERM. Returns the bytes each pipe gave.
#[cfg(unix)]
fn collect(child: &mut std::process::Child, forward: bool) -> [Vec<u8>; 2] {
    use std::io::Read;
    use std::os::fd::{AsRawFd, OwnedFd};
    let mut pipes: [Option<std::fs::File>; 2] = [
        child.stdout.take().map(|pipe| std::fs::File::from(OwnedFd::from(pipe))),
        child.stderr.take().map(|pipe| std::fs::File::from(OwnedFd::from(pipe))),
    ];
    let mut kept = [Vec::new(), Vec::new()];
    let mut text = [Decoder::default(), Decoder::default()];
    let mut counted = [0usize; 2];
    // As much as libuv reads at a time.
    let mut buf = vec![0u8; 65536];
    let mut over = false;
    'reading: loop {
        let open: Vec<usize> = (0..2).filter(|&i| pipes[i].is_some()).collect();
        if open.is_empty() {
            break;
        }
        let mut fds: Vec<libc::pollfd> =
            open.iter().map(|&i| libc::pollfd { fd: pipes[i].as_ref().map_or(-1, |f| f.as_raw_fd()), events: libc::POLLIN, revents: 0 }).collect();
        // SAFETY: `fds` is a live array of `fds.len()` pollfd structures.
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, -1) };
        if ready < 0 {
            if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            // Cannot wait on them: close them, as exec would on an error.
            break;
        }
        for (fd, &i) in fds.iter().zip(&open) {
            if fd.revents == 0 {
                continue;
            }
            let Some(pipe) = pipes[i].as_mut() else { continue };
            let (piece, ended) = match pipe.read(&mut buf) {
                Ok(0) => (text[i].end(), true),
                Ok(n) => {
                    kept[i].extend_from_slice(&buf[..n]);
                    (text[i].write(&buf[..n]), false)
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => (text[i].end(), true),
            };
            counted[i] += piece.len();
            if forward && !piece.is_empty() {
                pass_on(i, &piece);
            }
            if ended {
                pipes[i] = None;
            }
            if counted[i] > MAX_BUFFER {
                over = true;
                break 'reading;
            }
        }
    }
    // Closes both pipes (exec destroys both streams).
    drop(pipes);
    if over {
        // exec's kill(): SIGTERM to the shell it started, nothing else.
        if let Ok(pid) = libc::pid_t::try_from(child.id()) {
            // SAFETY: a plain system call on the pid of our own child,
            // which has not been waited for yet, so the pid is still its.
            unsafe { libc::kill(pid, libc::SIGTERM) };
        }
    }
    kept
}

/// Elsewhere (Windows) the binary hands every invocation back before it runs
/// a command (commit::prepare); this only keeps the crate whole there.
#[cfg(not(unix))]
fn collect(child: &mut std::process::Child, _forward: bool) -> [Vec<u8>; 2] {
    use std::io::Read;
    let mut out = Vec::new();
    let mut err = Vec::new();
    if let Some(mut pipe) = child.stdout.take() {
        let _ = pipe.read_to_end(&mut out);
    }
    if let Some(mut pipe) = child.stderr.take() {
        let _ = pipe.read_to_end(&mut err);
    }
    [out, err]
}

/// Writes a piece of a child's stdout (0) or stderr (1) to ours. A reader
/// that has gone away is not an error.
#[cfg(unix)]
fn pass_on(stream: usize, piece: &[u8]) {
    if stream == 0 {
        let mut out = std::io::stdout().lock();
        let _ = out.write_all(piece);
        let _ = out.flush();
    } else {
        let mut err = std::io::stderr().lock();
        let _ = err.write_all(piece);
        let _ = err.flush();
    }
}

/// What node's StringDecoder makes of a stream of bytes as UTF-8, a piece at
/// a time: every ill-formed part becomes U+FFFD (one for each maximal
/// subpart, as V8 decodes and as `String::from_utf8_lossy` does), and the
/// start of a character cut off at the end of a piece waits for the next.
/// The pieces put together are always `from_utf8_lossy` of all the bytes.
#[derive(Default)]
pub struct Decoder {
    pending: Vec<u8>,
}

impl Decoder {
    /// The text of `bytes`, after what an earlier piece left unfinished.
    pub fn write(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.pending.extend_from_slice(bytes);
        let mut out = Vec::with_capacity(self.pending.len());
        let mut rest: &[u8] = &self.pending;
        let unfinished = loop {
            match std::str::from_utf8(rest) {
                Ok(text) => {
                    out.extend_from_slice(text.as_bytes());
                    break 0;
                }
                Err(e) => {
                    let (valid, after) = rest.split_at(e.valid_up_to());
                    out.extend_from_slice(valid);
                    match e.error_len() {
                        Some(bad) => {
                            out.extend_from_slice("\u{fffd}".as_bytes());
                            rest = &after[bad..];
                        }
                        // The start of a character the next piece may finish.
                        None => break after.len(),
                    }
                }
            }
        };
        let keep = self.pending.len() - unfinished;
        self.pending.drain(..keep);
        out
    }

    /// At the end of the stream: an unfinished character is one U+FFFD.
    pub fn end(&mut self) -> Vec<u8> {
        if self.pending.is_empty() {
            return Vec::new();
        }
        self.pending.clear();
        "\u{fffd}".as_bytes().to_vec()
    }
}

/// How long `bytes` are once exec has decoded them, in bytes of UTF-8.
pub fn decoded_len(bytes: &[u8]) -> usize {
    String::from_utf8_lossy(bytes).len()
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every way of cutting `bytes` into three pieces decodes as the whole
    /// does.
    fn decodes_as_a_whole(bytes: &[u8]) {
        let whole = String::from_utf8_lossy(bytes).into_owned().into_bytes();
        for first in 0..=bytes.len() {
            for second in first..=bytes.len() {
                let mut d = Decoder::default();
                let mut out = d.write(&bytes[..first]);
                out.extend(d.write(&bytes[first..second]));
                out.extend(d.write(&bytes[second..]));
                out.extend(d.end());
                assert_eq!(out, whole, "{bytes:x?} cut at {first} and {second}");
            }
        }
    }

    #[test]
    fn the_decoder_replaces_as_exec_does_wherever_the_pieces_end() {
        for bytes in [
            &b"caf\xe9 hook\n"[..],
            b"na\xc3\xafve",
            "😀é日".as_bytes(),
            b"\xf0\x9f\x98",
            b"\xf0\x9f\x98x",
            b"\xe0\x80\x80",
            b"\xed\xa0\x80",
            b"\xc0\xaf\xff\xfe",
            b"\xf4\x90\x80\x80",
            b"ign\xff\n",
            b"\x80\x80\xe2\x82",
            b"",
        ] {
            decodes_as_a_whole(bytes);
        }
    }

    #[test]
    fn an_unfinished_character_at_the_end_is_one_replacement() {
        let mut d = Decoder::default();
        assert_eq!(d.write(b"a\xe2\x82"), b"a");
        assert_eq!(d.end(), "\u{fffd}".as_bytes());
        assert_eq!(d.end(), b"");
        assert_eq!(decoded_len(b"\xff\xff"), 6);
    }

    #[cfg(unix)]
    #[test]
    fn exec_stops_a_command_past_one_mebibyte_and_not_at_it() {
        let at = run(&format!("head -c {MAX_BUFFER} /dev/zero | tr '\\000' x; echo done >&2"), &[], false);
        assert_eq!((at.stdout.len(), at.stderr.as_slice(), at.code), (MAX_BUFFER, &b"done\n"[..], Some(0)));
        // Bytes that are not UTF-8 count three times once decoded.
        let over = run("head -c 3000000 /dev/zero | tr '\\000' '\\377'; echo done >&2", &[], false);
        assert!(over.stdout.len() > MAX_BUFFER / 3 && over.stdout.len() < 3_000_000, "{}", over.stdout.len());
        assert_eq!(over.code, None);
        assert!(!over.stderr.ends_with(b"done\n"));
    }
}
