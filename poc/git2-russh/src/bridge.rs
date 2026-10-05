//! Sync <-> async bridge. git2 calls the subtransport synchronously on the caller's thread;
//! russh needs a tokio runtime. One process-wide multi-thread runtime (2 workers) owns the
//! sockets, the russh session tasks and the channel pumps. The caller's thread enters that
//! runtime's context (so tokio timers / sockets / `tokio::spawn` resolve to it) and drives the
//! future with `futures::executor::block_on`. This works from plain threads and also from inside
//! another tokio runtime (no "Cannot start a runtime from within a runtime" panic, unlike
//! `Runtime::block_on`), at the price of blocking that thread.

use std::future::Future;
use std::sync::LazyLock;
use std::time::Duration;

use tokio::runtime::{Builder, Runtime};

static RT: LazyLock<Runtime> = LazyLock::new(|| {
    Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("git2-russh")
        .enable_all()
        .build()
        .expect("git2-russh: cannot start tokio runtime")
});

pub fn block_on<F: Future>(f: F) -> F::Output {
    let _ctx = RT.enter();
    futures::executor::block_on(f)
}

pub fn block_on_timeout<F: Future>(d: Duration, f: F) -> Result<F::Output, tokio::time::error::Elapsed> {
    block_on(async move { tokio::time::timeout(d, f).await })
}

pub fn spawn<F: Future<Output = ()> + Send + 'static>(f: F) {
    RT.spawn(f);
}
