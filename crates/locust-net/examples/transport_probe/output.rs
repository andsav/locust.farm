//! Blocking terminal I/O is isolated from the async network/deadline runtime.
//!
//! This is a detached ordinary thread, not Tokio's blocking pool: a full pipe
//! must not make runtime shutdown wait for a blocked writer. Each record is
//! acknowledged after flushing; the caller owns cancellation and output grace.

use std::io::{self, Write};

use tokio::sync::{mpsc, oneshot};

use super::{Failure, Result};

#[derive(Clone, Copy)]
enum Destination {
    Stdout,
    Stderr,
}

struct Pending {
    destination: Destination,
    line: String,
    acknowledged: oneshot::Sender<Result<()>>,
}

pub struct Output {
    sender: mpsc::Sender<Pending>,
}

impl Output {
    pub fn start() -> Result<Self> {
        // Records are emitted sequentially and awaited individually. One slot
        // coordinates the active writer; it is not a batch or payload limit.
        let (sender, mut receiver) = mpsc::channel::<Pending>(1);
        std::thread::Builder::new()
            .name("transport-probe-output".to_owned())
            .spawn(move || {
                while let Some(pending) = receiver.blocking_recv() {
                    let written = match pending.destination {
                        Destination::Stdout => flush_line(io::stdout().lock(), &pending.line),
                        Destination::Stderr => flush_line(io::stderr().lock(), &pending.line),
                    };
                    let failed = written.is_err();
                    // Cancellation drops the acknowledgment receiver; that is
                    // normal, and channel closure stops this worker next time.
                    let _ = pending.acknowledged.send(written);
                    if failed {
                        break;
                    }
                }
            })
            .map_err(|_| Failure("output_worker"))?;
        Ok(Self { sender })
    }

    pub async fn record(&self, line: impl Into<String>) -> Result<()> {
        self.write(Destination::Stdout, line.into()).await
    }

    pub async fn error(&self, line: impl Into<String>) -> Result<()> {
        self.write(Destination::Stderr, line.into()).await
    }

    async fn write(&self, destination: Destination, line: String) -> Result<()> {
        let (acknowledged, receiver) = oneshot::channel();
        self.sender
            .send(Pending {
                destination,
                line,
                acknowledged,
            })
            .await
            .map_err(|_| Failure("output"))?;
        receiver.await.map_err(|_| Failure("output"))?
    }
}

fn flush_line(mut writer: impl Write, line: &str) -> Result<()> {
    writeln!(writer, "{line}").map_err(|_| Failure("output"))?;
    writer.flush().map_err(|_| Failure("output"))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Broken;

    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn writer_failure_is_redacted() {
        assert_eq!(flush_line(Broken, "public"), Err(Failure("output")));
    }
}
