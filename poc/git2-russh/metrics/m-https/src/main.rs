//! Baseline: git2 with https only (no SSH at all). Prints the remote's refs.
fn main() -> Result<(), git2::Error> {
    let url = std::env::args().nth(1).expect("url");
    let repo = git2::Repository::init_bare(std::env::temp_dir().join("m-bare"))?;
    let mut r = repo.remote_anonymous(&url)?;
    r.connect(git2::Direction::Fetch)?;
    for h in r.list()? { println!("{} {}", h.oid(), h.name()); }
    Ok(())
}
