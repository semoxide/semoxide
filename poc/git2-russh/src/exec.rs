//! Option b1: spawn the system `ssh` and pipe its stdin/stdout to libgit2.
//! Program: `GIT_SSH_COMMAND` (split into words) > `GIT_SSH` > `SshOptions::exec_command` > `ssh`.
//! Always `-o BatchMode=yes` (never prompt) and `-o ConnectTimeout=<connect_timeout>`.

use std::io::{self, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::{SshError, SshOptions, url};

pub(crate) struct ExecStream {
    child: Child,
    stdin: Option<ChildStdin>,
    rx: Receiver<io::Result<Vec<u8>>>,
    buf: Vec<u8>,
    pos: usize,
    stderr: Arc<Mutex<Vec<u8>>>,
    io_timeout: Duration,
    program: String,
}

fn split_command(s: &str) -> Vec<String> {
    if cfg!(windows) {
        // Windows paths contain backslashes; only honour double quotes.
        let (mut out, mut cur, mut q) = (Vec::new(), String::new(), false);
        for c in s.chars() {
            match c {
                '"' => q = !q,
                c if c.is_whitespace() && !q => {
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                }
                c => cur.push(c),
            }
        }
        if !cur.is_empty() {
            out.push(cur);
        }
        out
    } else {
        shell_words::split(s).unwrap_or_else(|_| s.split_whitespace().map(String::from).collect())
    }
}

pub(crate) fn ssh_command(opts: &SshOptions) -> Vec<String> {
    if let Ok(c) = std::env::var("GIT_SSH_COMMAND")
        && !c.trim().is_empty()
    {
        return split_command(&c);
    }
    if let Ok(p) = std::env::var("GIT_SSH")
        && !p.is_empty()
    {
        return vec![p];
    }
    opts.exec_command.as_deref().map(split_command).filter(|v| !v.is_empty()).unwrap_or_else(|| vec!["ssh".into()])
}

pub(crate) fn open(raw_url: &str, service: &str, opts: &SshOptions) -> Result<ExecStream, SshError> {
    let u = url::parse(raw_url)?;
    let mut argv = ssh_command(opts);
    let program = argv.remove(0);
    argv.extend(["-o".into(), "BatchMode=yes".into()]);
    argv.extend(["-o".into(), format!("ConnectTimeout={}", opts.connect_timeout.as_secs().max(1))]);
    if let Some(p) = u.port {
        argv.extend(["-p".into(), p.to_string()]);
    }
    argv.push(match &u.user {
        Some(user) => format!("{user}@{}", u.host),
        None => u.host.clone(),
    });
    argv.push(url::remote_command(service, &u.path));

    let mut cmd = Command::new(&program);
    cmd.args(&argv).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => SshError::ExecNotFound { program: program.clone() },
        _ => SshError::Exec(format!("cannot run `{program}`: {e}")),
    })?;

    let mut stdout = child.stdout.take().expect("piped");
    let (tx, rx) = sync_channel(16);
    thread::spawn(move || {
        loop {
            let mut b = vec![0u8; 65536];
            match stdout.read(&mut b) {
                Ok(0) => break,
                Ok(n) => {
                    b.truncate(n);
                    if tx.send(Ok(b)).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(e));
                    break;
                }
            }
        }
    });
    let stderr = Arc::new(Mutex::new(Vec::new()));
    let (mut se, sink) = (child.stderr.take().expect("piped"), stderr.clone());
    thread::spawn(move || {
        let mut b = [0u8; 4096];
        while let Ok(n) = se.read(&mut b) {
            if n == 0 {
                break;
            }
            sink.lock().unwrap().extend_from_slice(&b[..n]);
        }
    });
    Ok(ExecStream {
        stdin: child.stdin.take(),
        child,
        rx,
        buf: vec![],
        pos: 0,
        stderr,
        io_timeout: opts.io_timeout,
        program,
    })
}

impl ExecStream {
    fn stderr_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr.lock().unwrap()).trim().replace('\n', " | ").replace('\r', "")
    }

    fn eof(&mut self) -> io::Result<usize> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            match self.child.try_wait()? {
                Some(s) => break Some(s),
                None if Instant::now() > deadline => break None,
                None => thread::sleep(Duration::from_millis(10)),
            }
        };
        thread::sleep(Duration::from_millis(20)); // let the stderr reader drain
        match status {
            Some(s) if !s.success() => {
                Err(io::Error::other(format!("`{}` failed ({s}): {}", self.program, self.stderr_text())))
            }
            _ => Ok(0),
        }
    }
}

impl Read for ExecStream {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.pos == self.buf.len() {
            match self.rx.recv_timeout(self.io_timeout) {
                Ok(Ok(b)) => {
                    self.buf = b;
                    self.pos = 0;
                }
                Ok(Err(e)) => return Err(e),
                Err(RecvTimeoutError::Disconnected) => return self.eof(),
                Err(RecvTimeoutError::Timeout) => {
                    let _ = self.child.kill();
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!("`{}`: no data for {} s; killed. stderr: {}", self.program, self.io_timeout.as_secs(), self.stderr_text()),
                    ));
                }
            }
        }
        let n = out.len().min(self.buf.len() - self.pos);
        out[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

impl Write for ExecStream {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let s = self.stdin.as_mut().ok_or_else(|| io::Error::other("stdin closed"))?;
        s.write_all(data).map_err(|e| {
            io::Error::new(e.kind(), format!("`{}`: {e}; stderr: {}", self.program, self.stderr_text()))
        })?;
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stdin.as_mut().map_or(Ok(()), |s| s.flush())
    }
}

impl Drop for ExecStream {
    fn drop(&mut self) {
        drop(self.stdin.take());
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn split() {
        let v = super::split_command(r#"ssh -i "/tmp/my key" -o IdentitiesOnly=yes"#);
        assert_eq!(v, ["ssh", "-i", "/tmp/my key", "-o", "IdentitiesOnly=yes"]);
    }
}
