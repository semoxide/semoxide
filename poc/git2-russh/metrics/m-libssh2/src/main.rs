//! git2 + libssh2 (the `ssh` feature): ls-remote with an agent credential callback.
fn main() -> Result<(), git2::Error> {
    let url = std::env::args().nth(1).expect("url");
    let repo = git2::Repository::init_bare(std::env::temp_dir().join("m-bare"))?;
    let mut r = repo.remote_anonymous(&url)?;
    let mut cb = git2::RemoteCallbacks::new();
    cb.credentials(|_, u, _| git2::Cred::ssh_key_from_agent(u.unwrap_or("git")));
    let c = r.connect_auth(git2::Direction::Fetch, Some(cb), None)?;
    for h in c.list()? { println!("{} {}", h.oid(), h.name()); }
    Ok(())
}
