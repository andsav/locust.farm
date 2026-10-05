//! Standard output and standard error for commands.
//!
//! `println!` and `eprintln!` panic when their stream cannot be written, so
//! `locust contract | head` ended with exit status 101. Commands write through
//! here instead, and [`status`] decides what an unwritten result does to the
//! exit status. The print macros are denied for the whole client in
//! [`super`].

use crate::failure;
use locust_proto::api::ErrorCode;
use std::fmt::Display;
use std::io::{self, Write};

/// Write `text` to standard output exactly as given.
pub(super) fn stdout(text: impl Display) -> io::Result<()> {
    write(&mut io::stdout().lock(), text)
}

/// Write `text` to standard error exactly as given.
pub(super) fn stderr(text: impl Display) -> io::Result<()> {
    write(&mut io::stderr().lock(), text)
}

fn write(stream: &mut impl Write, text: impl Display) -> io::Result<()> {
    write!(stream, "{text}")?;
    stream.flush()
}

/// The exit status of a command that ended with `status`, given how writing
/// its output went.
///
/// The command has already run. A reader that closed the pipe gave the output
/// up, so the command ends quietly with its own status: a committed write does
/// not look failed because of `| head`. Any other error loses output the caller
/// still wanted, so it is named on standard error and changes the status as
/// [`lost`] says.
pub(super) fn status(written: io::Result<()>, status: u8) -> u8 {
    match written {
        Err(error) if error.kind() != io::ErrorKind::BrokenPipe => {
            let (status, what) = lost(status);
            let _ = stderr(format_args!("locust: internal: {what}: {error}\n"));
            status
        }
        _ => status,
    }
}

/// The exit status, and what to say, when output was lost to anything but a
/// closed reader. A command that succeeded exits as `internal`; a status that
/// already says something more specific is kept.
fn lost(status: u8) -> (u8, &'static str) {
    if status == 0 {
        (
            failure::exit_status(ErrorCode::Internal),
            "the command succeeded, but its output could not be written",
        )
    } else {
        (status, "output could not be written")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Accepts `room` bytes, then fails every write the way a closed reader
    /// or a full disk does.
    struct Failing {
        room: usize,
        kind: io::ErrorKind,
    }
    impl Write for Failing {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.room == 0 {
                return Err(self.kind.into());
            }
            let taken = bytes.len().min(self.room);
            self.room -= taken;
            Ok(taken)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_failed_write_is_an_error_and_never_a_panic() {
        for kind in [io::ErrorKind::BrokenPipe, io::ErrorKind::StorageFull] {
            for room in [0, 3] {
                let mut stream = Failing { room, kind };
                let error = write(&mut stream, "one line\n").unwrap_err();
                assert_eq!(error.kind(), kind);
            }
        }
        let mut whole = Vec::new();
        write(&mut whole, format_args!("{}\n", "one line")).unwrap();
        assert_eq!(whole, b"one line\n");
    }

    #[test]
    fn a_closed_reader_keeps_the_commands_own_status() {
        // Success, a quiet `wait` and a refused request each stay what they were.
        for own in [0, 20, 21, 7] {
            assert_eq!(status(Ok(()), own), own);
            assert_eq!(status(Err(io::ErrorKind::BrokenPipe.into()), own), own);
        }
    }

    #[test]
    fn other_lost_output_fails_only_a_command_that_succeeded() {
        assert_eq!(lost(0).0, 1);
        for own in [20, 21, 7] {
            assert_eq!(lost(own).0, own);
        }
    }
}
