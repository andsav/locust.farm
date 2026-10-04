//! Nonblocking stdio on macOS and Linux. No detached blocking stdin task is
//! left holding the process open when stdout closes or MCP shuts down.
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use tokio::io::unix::AsyncFd;
use tokio::io::{AsyncRead, AsyncWrite, Interest, ReadBuf, Ready};

// The observer owns only a shared descriptor registration. It spawns no task
// or thread, and can remain pending independently of a blocked output write.
pub(super) trait Output: AsyncWrite + Unpin {
    fn closed(&self) -> impl std::future::Future<Output = io::Result<()>> + Send + 'static {
        std::future::pending()
    }
}
#[cfg(test)]
impl Output for tokio::io::DuplexStream {}

pub(super) struct Pipe {
    fd: Arc<AsyncFd<File>>,
    original_flags: OFlags,
}
impl Pipe {
    pub fn new(source: impl AsFd) -> io::Result<Self> {
        let original_flags = fcntl_getfl(&source)?;
        let copy: OwnedFd = rustix::io::dup(&source)?;
        fcntl_setfl(&copy, original_flags | OFlags::NONBLOCK)?;
        match AsyncFd::new(File::from(copy)) {
            Ok(fd) => Ok(Self {
                fd: Arc::new(fd),
                original_flags,
            }),
            Err(error) => {
                let _ = fcntl_setfl(&source, original_flags);
                Err(error)
            }
        }
    }
}
impl Drop for Pipe {
    fn drop(&mut self) {
        let _ = fcntl_setfl(self.fd.get_ref(), self.original_flags);
    }
}
impl AsyncRead for Pipe {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        loop {
            let mut ready = std::task::ready!(self.fd.poll_read_ready(cx))?;
            match ready.try_io(|fd| fd.get_ref().read(buf.initialize_unfilled())) {
                Ok(Ok(count)) => {
                    buf.advance(count);
                    return Poll::Ready(Ok(()));
                }
                Ok(Err(error)) => return Poll::Ready(Err(error)),
                Err(_) => continue,
            }
        }
    }
}
impl AsyncWrite for Pipe {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        loop {
            // The idle closure observer clears ordinary writable readiness.
            // Try the nonblocking syscall first so that this cannot strand a
            // later write waiting for an edge from an already-writable pipe.
            match self.fd.get_ref().write(buf) {
                Ok(count) => return Poll::Ready(Ok(count)),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) => return Poll::Ready(Err(error)),
            }
            let mut ready = std::task::ready!(self.fd.poll_write_ready(cx))?;
            match ready.try_io(|fd| fd.get_ref().write(buf)) {
                Ok(result) => return Poll::Ready(result),
                Err(_) => continue,
            }
        }
    }
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl Output for Pipe {
    fn closed(&self) -> impl std::future::Future<Output = io::Result<()>> + Send + 'static {
        let fd = self.fd.clone();
        async move {
            loop {
                let mut ready = fd.ready(Interest::WRITABLE | Interest::ERROR).await?;
                if ready.ready().is_write_closed() || ready.ready().is_error() {
                    return Ok(());
                }
                // Preserve closure/error bits, including an event that raced
                // this observation; only acknowledge ordinary writability.
                ready.clear_ready_matching(Ready::WRITABLE);
            }
        }
    }
}
