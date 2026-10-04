//! Local operating-system interrupts forwarded only to the retained child.
use crate::failure::Failure;
use rustix::process::Signal;
use std::sync::mpsc;

pub(super) struct Listener {
    pub events: mpsc::Receiver<Signal>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Listener {
    pub fn new() -> Result<Self, Failure> {
        let (sender, events) = mpsc::channel();
        let (shutdown, stop) = tokio::sync::oneshot::channel();
        let (ready, initialized) = mpsc::sync_channel(0);
        let thread = std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(_) => {
                    let _ = ready.send(false);
                    return;
                }
            };
            runtime.block_on(async move {
                use tokio::signal::unix::{SignalKind, signal};
                let (Ok(mut interrupt), Ok(mut terminate)) = (
                    signal(SignalKind::interrupt()),
                    signal(SignalKind::terminate()),
                ) else {
                    let _ = ready.send(false);
                    return;
                };
                let _ = ready.send(true);
                tokio::pin!(stop);
                loop {
                    tokio::select! {
                        _ = interrupt.recv() => { let _=sender.send(Signal::INT); }
                        _ = terminate.recv() => { let _=sender.send(Signal::TERM); }
                        _ = &mut stop => break,
                    }
                }
            });
        });
        if initialized.recv() != Ok(true) {
            let _ = thread.join();
            return Err(Failure::internal(
                "cannot install local client interrupt forwarding",
            ));
        }
        Ok(Self {
            events,
            shutdown: Some(shutdown),
            thread: Some(thread),
        })
    }
}
impl Drop for Listener {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
