use std::fmt::Debug;

use anyhow::{anyhow, Result};
use futures::channel::oneshot;
use futures::channel::oneshot::{Receiver, Sender};
use futures::Future;
use tokio::runtime::Handle;

/// A thread that does asynchronous work
/// using a Tokio runtime spawned on a
/// separate thread.
pub struct WorkThread {
    runtime_handle: Handle,
    stop_notifier: Sender<()>,
}

impl WorkThread {
    /// Builds a new WorkThread with an internal Tokio runtime.
    pub fn new() -> Result<Self> {
        let (handle_tx, mut handle_rx) = oneshot::channel();
        let (stop_tx, stop_rx) = oneshot::channel();
        let parent_thread = std::thread::current();

        std::thread::spawn(move || {
            // build runtime
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            // send parent thread runtime and wake parent up
            handle_tx.send(runtime.handle().clone()).unwrap();
            parent_thread.unpark();

            // force runtime to block until either stop_rx
            // sends something or the parent panics
            runtime.block_on(async {
                let _ = stop_rx.await;
            });
        });

        loop {
            match handle_rx.try_recv() {
                // still waiting
                Ok(None) => std::thread::park(),
                // received error
                Err(_) => return Err(anyhow!("Work thread panicked!")),
                // succeeded
                Ok(Some(handle)) => {
                    return Ok(Self {
                        runtime_handle: handle,
                        stop_notifier: stop_tx,
                    })
                }
            }
        }
    }

    /// Spawns a task on this thread's internal runtime, sending
    /// the result of the task to the returned Receiver.
    pub fn spawn_task<F>(&self, f: F) -> Receiver<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + Debug + 'static,
    {
        let (tx, rx) = oneshot::channel();
        self.runtime_handle.spawn(async {
            tx.send(f.await).unwrap();
        });
        rx
    }

    /// Gets a handle to this thread's internal runtime.
    pub fn handle(&self) -> Handle {
        self.runtime_handle.clone()
    }
}
