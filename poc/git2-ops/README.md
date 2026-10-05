# PoC: git2 covers semantic-release's git ops

Throwaway. Question: can `git2` 0.21.0 (libgit2 1.9.7) do every git op from
[semantic-release.md §3](../../docs/research/semantic-release.md) without the git CLI?
**Answer: yes, for everything testable locally.** Run `cargo test` (29 tests, about 3 s).

- `tests/ops.rs`: one test per op over `file://` (bare remote plus a working clone in a tempdir).
- `tests/daemon.rs`: shallow, unshallow, and server-side rejection. libgit2's local transport refuses
  shallow, so these tests run a `git daemon` (`git://`) as the **server**. Every client op is still git2.
  If `git` is not on PATH, these tests are skipped.
- `tests/http_auth.rs`: a mock smart-HTTP server on 127.0.0.1 (401 challenge, 403, ref advertisement).
  It tests the credentials callback and the G5 probe.
- The git CLI appears in tests only as a cross-check (`git log`, `tag --merged`, `notes show`, `fsck`).

## Results

| G# | op | git2 API used | status | notes |
|---|---|---|---|---|
| G1 | `git --version` | n/a | n/a | drop |
| G2 | remote URL | `find_remote("origin").url()`, `config().get_str("remote.origin.url")` | ✅ | |
| G3 | is git repo | `Repository::discover` / `open_ext(ceiling_dirs)` | ✅ | walks up like `rev-parse --git-dir` |
| G4, G12 | check-ref-format | `Reference::is_valid_name` | ✅ | pure |
| G5 | push `--dry-run` | `Remote::push` + `push_negotiation` callback returning `Err` | ✅ local, needs sandbox | Aborts after the ref advertisement and update computation. The mock HTTP server sees no POST. Same depth as `git push --dry-run`: hooks and branch protection are **not** probed. |
| G5' | push-permission probe | `connect_auth(Direction::Push, cb)` + `list()` | ✅ mock, needs sandbox | write token gives refs; read-only gives `403` (`class=Http`, status only in the message string); bad token re-calls the callback on every 401, so the callback must cap retries |
| G6 | ls-remote --heads | `remote_anonymous(url).connect(Fetch)` + `list()` | ✅ | |
| G7 | abbrev-ref HEAD | `head().shorthand()`, `head_detached()` | ✅ | |
| G8a | fetch `--unshallow --tags` | `FetchOptions::depth(i32::MAX)` + `download_tags(All)` + `+refs/tags/*:refs/tags/*` | ✅ (git://) | depth-1 `RepoBuilder` clone, then unshallow. `is_shallow()` turns false, `shallow` file is removed, `git fsck` is clean |
| G8b | fetch `+refs/heads/B:refs/heads/B` into the checked-out B | `Remote::fetch` | ✅ | No "refusing to fetch into current branch" guard, so this already behaves like `--update-head-ok`. The ref moves; the worktree does not. Also passes on a shallow clone. |
| G8c | unshallow fallback | n/a | ✅ | `depth(i32::MAX)` on a complete repo returns Ok, so no fallback is needed |
| G9 | fetch notes | `fetch(["+refs/notes/*:refs/notes/*"])` | ✅ | |
| G10 | tag → note | `tag_names` + `find_note(Some("refs/notes/semantic-release-<tag>"), commit)`, then legacy ref | ✅ | per-ref read, no #4073 concatenation |
| G11 | `tag --merged` | one `Revwalk` from the branch into a `HashSet` + peel each tag | ✅ | matches `git tag --merged` |
| G13 | branch up to date | `list()` remote tip + `fetch` + `graph_descendant_of` | ✅ | an unfetched remote tip is not in the local odb, so fetch it first |
| G14 | rev-parse HEAD | `head().peel_to_commit()` / `revparse_single("HEAD")` | ✅ | |
| G15 | tag → commit | `revparse_single(tag).peel_to_commit()` | ✅ | lightweight and annotated |
| G16 | log from..to | `Revwalk` `set_sorting(TIME)`, `push(to)`, `hide(from)` | ✅ | Order matches `git log` with a merge in range. `body()` trims trailing `\n` (`%b` doesn't); `message()` is full. |
| G17 | create tag | `tag_lightweight`, `tag` (annotated) | ✅ | annotated messages are stored verbatim; `message_prettify` gives `git tag -m` parity |
| G18 | write note `-f` | `note(.., Some("refs/notes/semantic-release-<tag>"), oid, json, force=true)` | ✅ | `git notes show` reads it back byte-identical |
| G19 | push **one** tag | `push(["refs/tags/vX:refs/tags/vX"])` + `push_update_reference` | ✅ ⚠️ | Pushes only the named tag (lightweight and annotated). ⚠️ libgit2 **does** push a fast-forward move of an existing remote tag, where git refuses with "already exists". Guard this in `push_negotiation`: reject `refs/tags/*` updates whose `src` is not zero (tested). |
| G20 | push notes ref | `push(["refs/notes/semantic-release-<tag>"])` | ✅ | |
| G21, G22 | rev-parse --verify / show-ref | `refname_to_id`, `revparse_single(tag).id()` | n/a | dead code in sr; not PoC'd |
| plugin | ls-files -m -o | `statuses(include_untracked, recurse)` | ✅ | ignored files are excluded |
| plugin | add | `Index::add_all(DEFAULT)` respects .gitignore; `IndexAddOption::FORCE` = `--force` (what sr uses) | ✅ | `Index::add_path` ignores .gitignore entirely |
| plugin | commit | `Repository::commit(Some("HEAD"), author, committer, ..)` | ✅ | Explicit bot `Signature`. No hooks and no signing run. |
| plugin | push branch | `push(["HEAD:refs/heads/main"])` | ✅ | Tags are not pushed along with it. |
| reject | non-ff / server reject | client `ErrorCode::NotFastForward`; server message via `push_update_reference(ref, Some(msg))` | ✅ (git://) | A pre-receive hook gives `Some("pre-receive hook declined")`; `push()` itself returns Ok, so **always check per-ref status**. |
| auth | HTTPS token | `credentials` cb → `Cred::userpass_plaintext("x-access-token", token)` | ✅ mock, needs sandbox | wire header `Basic base64(x-access-token:<token>)` checked |
| auth | SSH agent | cb → `Cred::ssh_key_from_agent(user)` (`USERNAME` first if URL has none) | needs sandbox | only the callback branch is tested; no local sshd |

## Build notes (Windows 11, rustc 1.99.0 MSVC)

- `git2 = { version = "0.21", default-features = false, features = ["vendored-libgit2", "https", "ssh"] }`.
  Default features are `[]`, so https and ssh must be enabled explicitly.
- **No OpenSSL on Windows and nothing to solve:** https uses **WinHTTP** (libgit2 build.rs defines
  `GIT_WINHTTP`), and libssh2 uses **WinCNG** (`openssl-on-win32` is off). Both build with `cc` only:
  no perl, no cmake, no vcpkg. `cargo tree -i openssl-sys` is empty.
- On Linux, `https` and `ssh` pull in `openssl-sys`. For static/musl builds use `vendored-openssl` (needs perl + make).
- Clean build: dev 23 s, release 32 s. 53 crates in the normal graph (`url`/`idna`/ICU make up most of them).
  A release test binary is about 2.5 MB, versus 0.7 MB for an empty test binary, so libgit2 + libssh2 + zlib add about 1.8 MB.

## Gaps

1. **Shallow over `file://` is unsupported** in libgit2 (`shallow fetch is not supported by the local
   transport`, [libgit2#6634](https://github.com/libgit2/libgit2/issues/6634)). This doesn't matter for CI,
   which uses https/ssh, but tests need a daemon or HTTP server.
2. **No real dry-run push.** The `push_negotiation` abort matches `git push --dry-run`, but neither probes
   branch protection or hooks. Only the real push reports those (per-ref status).
3. **Tag clobber:** libgit2 allows a fast-forward move of an existing remote tag, so the product needs the negotiation guard.
4. **HTTP error classification** is message-string only (`request failed with status code: 403`). There's no numeric status API.
5. **SSH:** libssh2 only. No `~/.ssh/config`. The agent is reached via the callback. It's untested whether
   `agent_win.c` reaches the Windows OpenSSH agent pipe.
6. **No commit hooks and no signing** in `Repository::commit`. That's fine for the bot (and safer), but it differs from sr's `git commit`.
7. Message-format nits: `Commit::body()` trims trailing newlines; tag messages are stored verbatim.

## Needs a real remote (sandbox)

- HTTPS + GitHub token (`x-access-token:<token>` and PAT): `connect(Push)` with write, read-only, and bad
  tokens, to confirm GitHub returns 403 vs 401 on `info/refs?service=git-receive-pack`. Then a real push of
  tag + notes ref + branch, a protected-branch rejection arriving via `push_update_reference`, and the same on GitLab.
- Shallow/unshallow against GitHub's smart-HTTP (the git:// path is proven here).
- SSH: libssh2 + ssh-agent on Linux, and Windows OpenSSH agent pipe / Pageant, push via `git@github.com:`.
- Proxies (WinHTTP system proxy vs `http.proxy` via `ProxyOptions`).
