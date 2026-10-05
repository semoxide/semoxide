//! `cargo run --example probe -- <url>`: connect + ls-remote with a random (unregistered) key,
//! GitHub's known host keys. Shows handshake + host-key check + the auth error text.
use git2_russh::{AuthSource, KnownHosts, SshOptions, with_options};

fn main() {
    git2_russh::register().unwrap();
    let url = std::env::args().nth(1).unwrap_or("git@github.com:semoxide/semoxide-sandbox.git".into());
    let k = russh::keys::PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519).unwrap();
    let text = k.to_openssh(russh::keys::ssh_key::LineEnding::LF).unwrap().to_string();
    let o = SshOptions {
        auth: vec![AuthSource::KeyMemory { label: "random".into(), text, passphrase: None }],
        known_hosts: vec![KnownHosts::GitHub],
        ..Default::default()
    };
    let t0 = std::time::Instant::now();
    let r = with_options(o, || {
        let tmp = std::env::temp_dir();
        let repo = git2::Repository::open(&tmp).or_else(|_| git2::Repository::init_bare(tmp.join("probe-bare")))?;
        let mut rm = repo.remote_anonymous(&url)?;
        rm.connect(git2::Direction::Fetch)?;
        Ok::<_, git2::Error>(rm.list()?.len())
    });
    println!("{r:?} in {} ms", t0.elapsed().as_millis());
}
