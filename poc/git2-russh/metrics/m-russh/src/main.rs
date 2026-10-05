//! git2 (no `ssh` feature) + git2-russh: ls-remote via the russh transport (default options).
fn main() -> Result<(), git2::Error> {
    git2_russh::register()?;
    let url = std::env::args().nth(1).expect("url");
    let repo = git2::Repository::init_bare(std::env::temp_dir().join("m-bare"))?;
    let mut r = repo.remote_anonymous(&url)?;
    r.connect(git2::Direction::Fetch)?;
    for h in r.list()? { println!("{} {}", h.oid(), h.name()); }
    Ok(())
}
