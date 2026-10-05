//! Throwaway PoC: a git2 smart subtransport for `ssh://` and scp-style URLs, with git2 built
//! **without** its `ssh` feature (no libssh2).
//!
//! - [`Backend::Native`] (option f): pure-Rust `russh` client, exec `git-upload-pack`/`git-receive-pack`.
//! - [`Backend::Exec`] (option b1): spawn the system `ssh` binary (BatchMode).
//!
//! Call [`register`] once, then use git2 as usual. Options come from [`with_options`] (thread-local,
//! for the duration of a closure) or from [`SshOptions::default`] (env: `SEMOXIDE_SSH_BACKEND`).

mod bridge;
mod error;
mod exec;
pub mod known_hosts;
mod native;
mod options;
pub mod sshconfig;
pub mod url;

use std::cell::RefCell;
use std::sync::OnceLock;

use git2::transport::{Service, SmartSubtransport, SmartSubtransportStream, Transport};

pub use error::SshError;
pub use options::{AuthSource, BACKEND_ENV, Backend, KnownHosts, SshConfigSource, SshOptions};

thread_local! {
    static CURRENT: RefCell<Option<SshOptions>> = const { RefCell::new(None) };
}

/// Run `f` with `opts` as the SSH options for every git2 connection made on this thread.
pub fn with_options<R>(opts: SshOptions, f: impl FnOnce() -> R) -> R {
    let prev = CURRENT.with(|c| c.replace(Some(opts)));
    struct Restore(Option<SshOptions>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CURRENT.with(|c| *c.borrow_mut() = self.0.take());
        }
    }
    let _r = Restore(prev);
    f()
}

fn current() -> SshOptions {
    CURRENT.with(|c| c.borrow().clone()).unwrap_or_default()
}

struct SshSubtransport {
    opts: SshOptions,
}

impl SmartSubtransport for SshSubtransport {
    fn action(&self, url: &str, action: Service) -> Result<Box<dyn SmartSubtransportStream>, git2::Error> {
        // git2-rs only calls us for the *Ls services when rpc=false; UploadPack/ReceivePack reuse the stream.
        let service = match action {
            Service::UploadPackLs | Service::UploadPack => "git-upload-pack",
            Service::ReceivePackLs | Service::ReceivePack => "git-receive-pack",
        };
        Ok(match self.opts.backend {
            Backend::Native => Box::new(native::open(url, service, &self.opts)?),
            Backend::Exec => Box::new(exec::open(url, service, &self.opts)?),
        })
    }

    fn close(&self) -> Result<(), git2::Error> {
        Ok(())
    }
}

static REGISTERED: OnceLock<Result<(), String>> = OnceLock::new();

/// Register the transport for `ssh://`, `ssh+git://`, `git+ssh://` and (via libgit2's fallback
/// to the `ssh://` transport) scp-style `user@host:path`. Idempotent and process-wide.
pub fn register() -> Result<(), git2::Error> {
    REGISTERED
        .get_or_init(|| {
            // libgit2 appends "://" itself.
            for prefix in ["ssh", "ssh+git", "git+ssh"] {
                // SAFETY: called once per prefix, before any concurrent transport lookup by us;
                // libgit2's registry is global. The factory is 'static + Send + Sync.
                unsafe {
                    git2::transport::register(prefix, |remote| {
                        Transport::smart(remote, false, SshSubtransport { opts: current() })
                    })
                }
                .map_err(|e| e.to_string())?;
            }
            Ok(())
        })
        .clone()
        .map_err(|e| git2::Error::from_str(&e))
}
