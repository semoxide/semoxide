//! Native local socket transport for tonic: Unix domain socket / Windows named pipe.
//!
//! tonic has no named-pipe support, so both directions are bridged by hand:
//! - client side: [`channel_over`] hands an already-connected stream to
//!   `Endpoint::connect_with_connector` through a one-shot `tower::service_fn`.
//! - server side: [`Io`] wraps any stream and implements tonic's `Connected`,
//!   so a `stream::once(..)` of it can be fed to `serve_with_incoming*`.
//!
//! Who connects: the host listens, the plugin dials twice and writes a tag byte
//! first: [`TAG_PLUGIN`] (host is the HTTP/2 client) and [`TAG_HOST`] (plugin
//! is the HTTP/2 client). HTTP/2 roles do not depend on who called `accept`.

use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
#[cfg(windows)]
use std::time::{Duration, Instant};

use hyper_util::rt::TokioIo;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::sync::mpsc;
use tonic::transport::server::Connected;
use tonic::transport::{Channel, Endpoint, Uri};

/// Connection carrying the plugin service (host -> plugin calls).
pub const TAG_PLUGIN: u8 = b'P';
/// Connection carrying the host services (plugin -> host calls).
pub const TAG_HOST: u8 = b'H';

#[cfg(windows)]
pub type ClientStream = tokio::net::windows::named_pipe::NamedPipeClient;
#[cfg(unix)]
pub type ClientStream = tokio::net::UnixStream;

#[cfg(windows)]
pub type ServerStream = tokio::net::windows::named_pipe::NamedPipeServer;
#[cfg(unix)]
pub type ServerStream = tokio::net::UnixStream;

/// Stream wrapper that tonic's server accepts. Optionally signals when hyper
/// drops it (= connection closed), which the plugin uses to exit when the host
/// goes away.
pub struct Io<T> {
    inner: T,
    on_drop: Option<mpsc::Sender<()>>,
}

impl<T> Io<T> {
    pub fn new(inner: T) -> Self {
        Self { inner, on_drop: None }
    }
    pub fn notify_on_drop(inner: T, tx: mpsc::Sender<()>) -> Self {
        Self { inner, on_drop: Some(tx) }
    }
}

impl<T> Drop for Io<T> {
    fn drop(&mut self) {
        if let Some(tx) = self.on_drop.take() {
            let _ = tx.try_send(());
        }
    }
}

impl<T> Connected for Io<T> {
    type ConnectInfo = ();
    fn connect_info(&self) -> Self::ConnectInfo {}
}

impl<T: AsyncRead + Unpin> AsyncRead for Io<T> {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<T: AsyncWrite + Unpin> AsyncWrite for Io<T> {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write_vectored(cx, bufs)
    }
    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }
}

/// Turn one already-connected stream into a tonic `Channel` (HTTP/2 client).
/// The connector yields the stream once; a reconnect attempt fails.
pub async fn channel_over<S>(stream: S) -> Result<Channel, tonic::transport::Error>
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let slot = Arc::new(Mutex::new(Some(stream)));
    let connector = tower::service_fn(move |_: Uri| {
        let s = slot.lock().unwrap().take();
        async move {
            s.map(TokioIo::new)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "local socket already consumed"))
        }
    });
    // The URI is required by tonic but never resolved.
    Endpoint::from_static("http://plugin.local").connect_with_connector(connector).await
}

/// Plugin side: connect to the host's socket and write the connection tag.
pub async fn dial(addr: &str, tag: u8) -> io::Result<ClientStream> {
    let mut s = dial_raw(addr).await?;
    s.write_all(&[tag]).await?;
    Ok(s)
}

#[cfg(unix)]
async fn dial_raw(addr: &str) -> io::Result<ClientStream> {
    tokio::net::UnixStream::connect(addr).await
}

#[cfg(windows)]
async fn dial_raw(addr: &str) -> io::Result<ClientStream> {
    use tokio::net::windows::named_pipe::ClientOptions;
    const ERROR_FILE_NOT_FOUND: i32 = 2;
    const ERROR_PIPE_BUSY: i32 = 231;
    let give_up = Instant::now() + Duration::from_secs(5);
    loop {
        match ClientOptions::new().open(addr) {
            Ok(c) => return Ok(c),
            // BUSY: the listening instance was just taken and the host has
            // not created the next one yet. Do NOT `sleep(1ms)`: the Windows
            // timer granularity makes that ~15 ms (measured: 24 ms vs 9 ms
            // spawn-to-handshake). WaitNamedPipeW wakes as soon as an
            // instance is free.
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) && Instant::now() < give_up => {
                let wide: Vec<u16> = addr.encode_utf16().chain([0]).collect();
                tokio::task::spawn_blocking(move || unsafe {
                    windows_sys::Win32::System::Pipes::WaitNamedPipeW(wide.as_ptr(), 100);
                })
                .await
                .ok();
            }
            Err(e) if e.raw_os_error() == Some(ERROR_FILE_NOT_FOUND) && Instant::now() < give_up => {
                tokio::task::yield_now().await;
            }
            Err(e) => return Err(e),
        }
    }
}

/// Host side listener. Created before the plugin is spawned; dropped once the
/// plugin has connected, so nobody else can connect later.
#[cfg(feature = "host")]
pub struct Listener {
    addr: String,
    #[cfg(windows)]
    server: tokio::net::windows::named_pipe::NamedPipeServer,
    #[cfg(unix)]
    listener: tokio::net::UnixListener,
    #[cfg(unix)]
    dir: std::path::PathBuf,
}

#[cfg(feature = "host")]
fn unique_suffix() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("{}-{}-{:x}", std::process::id(), N.fetch_add(1, Ordering::Relaxed), nanos)
}

#[cfg(feature = "host")]
impl Listener {
    #[cfg(windows)]
    pub fn bind_unique() -> io::Result<Self> {
        use tokio::net::windows::named_pipe::ServerOptions;
        let addr = format!(r"\\.\pipe\semoxide-{}", unique_suffix());
        // first_pipe_instance: fail if someone already squats on the name.
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .reject_remote_clients(true)
            .create(&addr)?;
        Ok(Self { addr, server })
    }

    #[cfg(unix)]
    pub fn bind_unique() -> io::Result<Self> {
        use std::os::unix::fs::DirBuilderExt;
        // Private 0700 dir; keep the path short (sun_path limit ~108 bytes).
        let dir = std::env::temp_dir().join(format!("semoxide-{}", unique_suffix()));
        std::fs::DirBuilder::new().mode(0o700).create(&dir)?;
        let path = dir.join("p.sock");
        let listener = tokio::net::UnixListener::bind(&path)?;
        Ok(Self { addr: path.to_string_lossy().into_owned(), listener, dir })
    }

    pub fn addr(&self) -> &str {
        &self.addr
    }

    /// Accept one connection and read its tag byte.
    pub async fn accept(&mut self) -> io::Result<(u8, ServerStream)> {
        use tokio::io::AsyncReadExt;
        let mut s = self.accept_raw().await?;
        let mut tag = [0u8; 1];
        s.read_exact(&mut tag).await?;
        Ok((tag[0], s))
    }

    #[cfg(windows)]
    async fn accept_raw(&mut self) -> io::Result<ServerStream> {
        use tokio::net::windows::named_pipe::ServerOptions;
        self.server.connect().await?;
        // A named pipe instance serves one client: create the next instance.
        let next = ServerOptions::new().reject_remote_clients(true).create(&self.addr)?;
        Ok(std::mem::replace(&mut self.server, next))
    }

    #[cfg(unix)]
    async fn accept_raw(&mut self) -> io::Result<ServerStream> {
        Ok(self.listener.accept().await?.0)
    }
}

#[cfg(all(feature = "host", unix))]
impl Drop for Listener {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
