//! A local SSH git server for tests: russh server that runs the real `git upload-pack` /
//! `git receive-pack` for exec requests (so the client side is exercised exactly as against GitHub).
//! A repo path containing "hang" makes the server accept the exec and then never answer.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use russh::keys::{PrivateKey, PublicKey};
use russh::server::{self, Auth, Msg, Session};
use russh::{Channel, ChannelId, ChannelMsg};
use tokio::io::{AsyncWriteExt, copy};
use tokio::process::Command;

#[derive(Clone)]
struct H {
    root: PathBuf,
    authorized: Arc<Vec<PublicKey>>,
}

impl H {
    fn ok(&self, user: &str, key: &PublicKey) -> Auth {
        if user == "git" && self.authorized.iter().any(|k| k.key_data() == key.key_data()) {
            Auth::Accept
        } else {
            Auth::reject()
        }
    }
}

impl server::Handler for H {
    type Error = russh::Error;

    async fn auth_publickey_offered(&mut self, user: &str, key: &PublicKey) -> Result<Auth, Self::Error> {
        Ok(self.ok(user, key))
    }

    async fn auth_publickey(&mut self, user: &str, key: &PublicKey) -> Result<Auth, Self::Error> {
        Ok(self.ok(user, key))
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        tokio::spawn(serve(channel, self.root.clone()));
        Ok(())
    }

    async fn exec_request(&mut self, ch: ChannelId, _data: &[u8], session: &mut Session) -> Result<(), Self::Error> {
        session.channel_success(ch)?;
        Ok(())
    }
}

async fn serve(channel: Channel<Msg>, root: PathBuf) {
    let (mut rd, wr) = channel.split();
    let cmd = loop {
        match rd.wait().await {
            Some(ChannelMsg::Exec { command, .. }) => break String::from_utf8_lossy(&command).to_string(),
            Some(_) => continue,
            None => return,
        }
    };
    let (svc, path) = cmd.split_once(' ').unwrap_or((&cmd, ""));
    let path = path.trim_matches('\'').trim_start_matches('/');
    if path.contains("hang") {
        while rd.wait().await.is_some() {}
        return;
    }
    let mut child = Command::new("git")
        .arg(svc.trim_start_matches("git-"))
        .arg(root.join(path))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("git");
    let (mut stdin, mut stdout, mut stderr) =
        (child.stdin.take().unwrap(), child.stdout.take().unwrap(), child.stderr.take().unwrap());
    let (mut ow, mut ew) = (wr.make_writer(), wr.make_writer_ext(Some(1)));
    let t_out = tokio::spawn(async move {
        let _ = copy(&mut stdout, &mut ow).await;
        let _ = ow.flush().await;
    });
    let t_err = tokio::spawn(async move {
        let _ = copy(&mut stderr, &mut ew).await;
        let _ = ew.flush().await;
    });
    let t_in = tokio::spawn(async move {
        while let Some(m) = rd.wait().await {
            match m {
                ChannelMsg::Data { data } => {
                    if stdin.write_all(&data).await.is_err() {
                        break;
                    }
                }
                ChannelMsg::Eof | ChannelMsg::Close => break,
                _ => {}
            }
        }
    });
    let status = child.wait().await.ok().and_then(|s| s.code()).unwrap_or(255);
    let _ = t_out.await;
    let _ = t_err.await;
    let _ = wr.exit_status(status as u32).await;
    let _ = wr.eof().await;
    let _ = wr.close().await;
    t_in.abort();
}

pub struct TestServer {
    pub port: u16,
    /// known_hosts line for this server's host key.
    pub known_hosts: String,
    _stop: tokio::sync::oneshot::Sender<()>,
}

/// Start on 127.0.0.1:<random> in its own runtime thread (independent of the library's runtime).
pub fn start(root: PathBuf, host_key: PrivateKey, authorized: Vec<PublicKey>) -> TestServer {
    let (port_tx, port_rx) = std::sync::mpsc::channel();
    let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();
    let pubkey = host_key.public_key().to_openssh().unwrap();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        rt.block_on(async move {
            let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            port_tx.send(l.local_addr().unwrap().port()).unwrap();
            let cfg = Arc::new(server::Config {
                keys: vec![host_key],
                auth_rejection_time: std::time::Duration::from_millis(10),
                auth_rejection_time_initial: Some(std::time::Duration::from_millis(0)),
                ..Default::default()
            });
            let h = H { root, authorized: Arc::new(authorized) };
            loop {
                tokio::select! {
                    _ = &mut stop_rx => break,
                    acc = l.accept() => {
                        let Ok((sock, _)) = acc else { continue };
                        let (cfg, h) = (cfg.clone(), h.clone());
                        tokio::spawn(async move {
                            if let Ok(s) = server::run_stream(cfg, sock, h).await {
                                let _ = s.await;
                            }
                        });
                    }
                }
            }
        });
    });
    let port = port_rx.recv().unwrap();
    TestServer { port, known_hosts: format!("[127.0.0.1]:{port} {pubkey}\n"), _stop: stop_tx }
}

/// Accepts TCP and then says nothing (no SSH banner): for timeout tests.
pub fn start_silent() -> SocketAddr {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        let mut held = Vec::new();
        for s in l.incoming().flatten() {
            held.push(s);
        }
    });
    addr
}
