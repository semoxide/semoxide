//! The files a commit changed, for path filters: read on demand, since a diff per commit is the
//! slow part of a log walk.

use std::collections::BTreeSet;
use std::fmt;

use tracing::debug;

use crate::id::to_oid;
use crate::{CommitId, GitError, Repo};

/// A path inside the repository, relative to its top and separated by `/` on every OS, as
/// path filters match it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoPath(String);

impl RepoPath {
    /// The path, e.g. `crates/core/src/lib.rs`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RepoPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Repo {
    /// The paths `commit` changed against its first parent, in git's order (by their bytes); a
    /// root commit lists every file, and a rename lists both the old and the new path. A path
    /// that isn't UTF-8 is decoded with replacement characters.
    ///
    /// # Errors
    ///
    /// Returns [`GitError`] if `commit` doesn't exist or libgit2 fails to diff it.
    pub fn changed_paths(&self, commit: &CommitId) -> Result<Vec<RepoPath>, GitError> {
        let read = |error: git2::Error| GitError::read_failed(&error);
        let repo = self.git();
        let found = repo
            .find_commit(to_oid(commit).map_err(read)?)
            .map_err(read)?;
        let tree = found.tree().map_err(read)?;
        let parent = if found.parent_count() == 0 {
            None
        } else {
            Some(found.parent(0).map_err(read)?.tree().map_err(read)?)
        };
        // No rename detection: a rename is a deletion and an addition, so both paths count.
        let diff = repo
            .diff_tree_to_tree(parent.as_ref(), Some(&tree), None)
            .map_err(read)?;
        // Strings sort by their bytes, as git's paths do.
        let mut paths = BTreeSet::new();
        for delta in diff.deltas() {
            for file in [delta.old_file(), delta.new_file()] {
                if let Some(path) = file.path_bytes() {
                    paths.insert(String::from_utf8_lossy(path).into_owned());
                }
            }
        }
        debug!(target: "semoxide::git", op = "G16", %commit, count = paths.len(), "changed paths read");
        Ok(paths.into_iter().map(RepoPath).collect())
    }
}

#[cfg(test)]
mod tests {
    use semoxide_error::ErrorInfo;
    use semoxide_test_support::git_fixture::GitFixture;

    use super::*;
    use crate::codes::GIT_READ_FAILED;

    /// A root commit with two files, a change, a rename, and a branch with its own file merged
    /// with a merge commit.
    fn fixture() -> GitFixture {
        let fixture = GitFixture::builder().build().unwrap();
        let root = fixture.path();
        let write = |path: &str, text: &str| {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        let commit = |message: &str| {
            fixture.git(&["add", "--all"]).unwrap();
            fixture.git(&["commit", "-m", message]).unwrap();
        };
        write("a.txt", "a\n");
        write("b/c.txt", "c\n");
        commit("feat: root");
        write("a.txt", "a changed\n");
        commit("fix: change");
        fixture.git(&["mv", "b/c.txt", "b/d.txt"]).unwrap();
        commit("refactor: rename");
        fixture.git(&["checkout", "-b", "side"]).unwrap();
        write("side.txt", "side\n");
        commit("feat: side");
        fixture.git(&["checkout", "main"]).unwrap();
        write("a.txt", "a changed again\n");
        commit("fix: on main");
        fixture
            .git(&["merge", "--no-ff", "-m", "merge side", "side"])
            .unwrap();
        fixture
    }

    fn commit(fixture: &GitFixture, rev: &str) -> CommitId {
        CommitId::try_from(fixture.rev_parse(rev).unwrap().as_str()).unwrap()
    }

    /// git's output, one path per line.
    fn lines(output: &str) -> Vec<String> {
        output.lines().map(str::to_owned).collect()
    }

    /// git's names for the fixture's commits on the first-parent line, root last.
    const ROOT: &str = "HEAD~4";
    const CHANGE: &str = "HEAD~3";
    const RENAME: &str = "HEAD~2";
    const ON_MAIN: &str = "HEAD~1";
    const MERGE: &str = "HEAD";

    #[test]
    fn changed_paths_match_git() {
        let fixture = fixture();
        let git = |args: &[&str]| lines(&fixture.git(args).unwrap());
        let diff_tree = |rev: &str| {
            git(&[
                "diff-tree",
                "--no-commit-id",
                "--name-only",
                "-r",
                "--root",
                rev,
            ])
        };
        let against_first_parent = git(&["diff", "--name-only", "--no-renames", "HEAD^1", "HEAD"]);
        // The fixture has what the comparison needs: a root with two files, a rename and a
        // merge that brings in a file.
        assert_eq!(
            (
                git(&["log", "--format=%s", "--first-parent", MERGE]),
                diff_tree(ROOT),
                diff_tree(RENAME),
                against_first_parent.clone()
            ),
            (
                [
                    "merge side",
                    "fix: on main",
                    "refactor: rename",
                    "fix: change",
                    "feat: root"
                ]
                .map(String::from)
                .to_vec(),
                ["a.txt", "b/c.txt"].map(String::from).to_vec(),
                ["b/c.txt", "b/d.txt"].map(String::from).to_vec(),
                vec![String::from("side.txt")]
            )
        );

        let repo = Repo::discover(&fixture.path()).unwrap();
        let ours = |rev: &str| {
            repo.changed_paths(&commit(&fixture, rev)).map(|paths| {
                paths
                    .iter()
                    .map(|path| path.as_str().to_owned())
                    .collect::<Vec<_>>()
            })
        };

        assert_eq!(
            (
                ours(ROOT),
                ours(CHANGE),
                ours(RENAME),
                ours(ON_MAIN),
                ours("side"),
                ours(MERGE)
            ),
            (
                Ok(diff_tree(ROOT)),
                Ok(diff_tree(CHANGE)),
                Ok(diff_tree(RENAME)),
                Ok(diff_tree(ON_MAIN)),
                Ok(diff_tree("side")),
                Ok(against_first_parent)
            )
        );
    }

    /// Names that sort differently as strings and as paths, then a deletion and a change in a
    /// folder.
    #[test]
    fn paths_sort_like_git_and_deletions_count() {
        let fixture = GitFixture::builder().build().unwrap();
        let root = fixture.path();
        for path in ["a-b.txt", "a/b.txt", "a.b", "a0", "A.txt", "a/b/c.txt"] {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "x\n").unwrap();
        }
        fixture.git(&["add", "--all"]).unwrap();
        fixture.git(&["commit", "-m", "feat: root"]).unwrap();
        fixture.git(&["rm", "-q", "a-b.txt"]).unwrap();
        std::fs::write(root.join("a").join("b").join("c.txt"), "y\n").unwrap();
        fixture
            .git(&["commit", "-am", "fix: delete and change"])
            .unwrap();
        let diff_tree = |rev: &str| {
            lines(
                &fixture
                    .git(&[
                        "diff-tree",
                        "--no-commit-id",
                        "--name-only",
                        "-r",
                        "--root",
                        rev,
                    ])
                    .unwrap(),
            )
        };
        let repo = Repo::discover(&fixture.path()).unwrap();
        let ours = |rev: &str| {
            repo.changed_paths(&commit(&fixture, rev)).map(|paths| {
                paths
                    .iter()
                    .map(|path| path.as_str().to_owned())
                    .collect::<Vec<_>>()
            })
        };

        assert_eq!(
            (ours("HEAD~1"), ours("HEAD")),
            (Ok(diff_tree("HEAD~1")), Ok(diff_tree("HEAD")))
        );
    }

    #[test]
    fn a_repo_path_displays_as_its_text() {
        let path = RepoPath(String::from("crates/core/src/lib.rs"));

        assert_eq!(
            (path.to_string(), path.as_str()),
            (
                String::from("crates/core/src/lib.rs"),
                "crates/core/src/lib.rs"
            )
        );
    }

    #[test]
    fn an_unknown_commit_is_a_read_failure() {
        let fixture = fixture();
        let unknown = CommitId::try_from("1111111111111111111111111111111111111111").unwrap();

        assert_eq!(
            Repo::discover(&fixture.path())
                .unwrap()
                .changed_paths(&unknown)
                .map_err(|error| error.code()),
            Err(GIT_READ_FAILED)
        );
    }
}
