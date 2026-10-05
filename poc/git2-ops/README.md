# PoC: git2 covers semantic-release's git ops

Throwaway. Question: can `git2` 0.21.0 (libgit2 1.9.7) do every git op from
[semantic-release.md §3](../../docs/research/semantic-release.md) without the git CLI?
**Answer: yes, for everything testable locally.** Run `cargo test` (29 tests, about 3 s).
The ops that need a real remote are proven against GitHub too, on Linux and Windows: see [Real remote](#real-remote-github-sandbox).

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
| G5 | push `--dry-run` | `Remote::push` + `push_negotiation` callback returning `Err` | ✅ local; mechanism ✅ GitHub | Aborts after the ref advertisement and update computation (on GitHub, the tag guard in R2 uses the same `Err` abort, and the remote is unchanged). The mock HTTP server sees no POST. Same depth as `git push --dry-run`: hooks and branch protection are **not** probed. |
| G5' | push-permission probe | `connect_auth(Direction::Push, cb)` + `list()` | ✅ GitHub (Linux, Windows) | write token gives refs; read-only token gives 403, and the message differs per OS (R4b); bad token re-calls the callback on every 401, so the callback must cap retries (libgit2 itself stops after 15 calls, R4c) |
| G6 | ls-remote --heads | `remote_anonymous(url).connect(Fetch)` + `list()` | ✅ | |
| G7 | abbrev-ref HEAD | `head().shorthand()`, `head_detached()` | ✅ | |
| G8a | fetch `--unshallow --tags` | `FetchOptions::depth(i32::MAX)` + `download_tags(All)` + `+refs/tags/*:refs/tags/*` | ✅ (git://), ✅ GitHub HTTPS (Linux, Windows) | depth-1 `RepoBuilder` clone, then unshallow. `is_shallow()` turns false, `shallow` file is removed, `git fsck` is clean |
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
| G19 | push **one** tag | `push(["refs/tags/vX:refs/tags/vX"])` + `push_update_reference` | ✅ ⚠️ (local + GitHub) | Pushes only the named tag (lightweight and annotated). ⚠️ libgit2 **does** push a fast-forward move of an existing remote tag, where git refuses with "already exists", and **GitHub accepts it** (the tag moves). Guard this in `push_negotiation`: reject `refs/tags/*` updates whose `src` is not zero (tested on GitHub, R2). |
| G20 | push notes ref | `push(["refs/notes/semantic-release-<tag>"])` | ✅ (local + GitHub) | |
| G21, G22 | rev-parse --verify / show-ref | `refname_to_id`, `revparse_single(tag).id()` | n/a | dead code in sr; not PoC'd |
| plugin | ls-files -m -o | `statuses(include_untracked, recurse)` | ✅ | ignored files are excluded |
| plugin | add | `Index::add_all(DEFAULT)` respects .gitignore; `IndexAddOption::FORCE` = `--force` (what sr uses) | ✅ | `Index::add_path` ignores .gitignore entirely |
| plugin | commit | `Repository::commit(Some("HEAD"), author, committer, ..)` | ✅ | Explicit bot `Signature`. No hooks and no signing run. |
| plugin | push branch | `push(["HEAD:refs/heads/main"])` | ✅ | Tags are not pushed along with it. |
| reject | non-ff / server reject | client `ErrorCode::NotFastForward`; server message via `push_update_reference(ref, Some(msg))` | ✅ (git://), ✅ GitHub | A pre-receive hook gives `Some("pre-receive hook declined")`; for GitHub see R3. `push()` itself returns Ok, so **always check per-ref status**. |
| auth | HTTPS token | `credentials` cb → `Cred::userpass_plaintext("x-access-token", token)` | ✅ mock, ✅ GitHub `GITHUB_TOKEN` | wire header `Basic base64(x-access-token:<token>)` checked; on GitHub the callback is called once per connection |
| auth | SSH agent | cb → `Cred::ssh_key_from_agent(user)` (`USERNAME` first if URL has none) | ✅ GitHub (Linux ssh-agent, Windows OpenSSH agent pipe) | R7 |
| auth | SSH key file / memory | `Cred::ssh_key(user, None, path, None)` / `Cred::ssh_key_from_memory` | ✅ Linux, ⚠️ Windows | Windows (WinCNG) only works with PEM RSA keys (R7b to R7d) |

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
4. **HTTP error classification** is message-string only, and the string **differs per OS**: WinHTTP gives
   `request failed with status code: 403`, and the Linux (OpenSSL) client gives `unexpected http status code: 403`.
   There's no numeric status API. Match `class=Http` plus `403` in the message.
5. **SSH:** libssh2 only. No `~/.ssh/config`. The agent is reached via the callback. On Windows it reaches
   the OpenSSH agent pipe (proven). On Windows (WinCNG), key **files and memory keys only work in PEM RSA
   format**. OpenSSH-format keys (ed25519, ecdsa, and even RSA) fail from a file and **hang** from memory.
   The WinCNG handshake also fails intermittently (about 1 in 60). See R7.
6. **No commit hooks and no signing** in `Repository::commit`. That's fine for the bot (and safer), but it differs from sr's `git commit`.
7. Message-format nits: `Commit::body()` trims trailing newlines; tag messages are stored verbatim.
8. **No server-side branch protection on the sandbox:** a free account can't use branch protection or rulesets on
   a private repo (`403 Upgrade to GitHub Pro or make this repository public to enable this feature.` from both
   `branches/main/protection` and `rulesets`). So a protected-branch rejection is not reproduced. Two other
   server-side rejections stand in for it (R3), and they take the same per-ref path.
9. **GitHub can answer a bare per-ref `failed`** while `push()` returns Ok. It was seen once: Windows, annotated tag, 1 of
   about 30 tag pushes. A retry succeeded. The product should treat a bare `failed` as retryable once.

## Real remote (GitHub sandbox)

The workflow is `semoxide/semoxide-sandbox` → `.github/workflows/remote-git.yml`, in a private repo. Its `tests/remote.rs`
is a standalone copy of the helpers above. It runs on `workflow_dispatch` and weekly, on two runners:

- **ubuntu-latest:** libgit2 HTTP client with OpenSSL, libssh2 with OpenSSL.
- **windows-latest:** WinHTTP, libssh2 with WinCNG.

HTTPS auth is the job's automatic `GITHUB_TOKEN` as `x-access-token:<token>`. A `contents: write` job runs the
main tests, and a separate `contents: read` job runs the 403 probe. SSH uses write deploy keys stored as
Actions secrets: ed25519, plus rsa-4096 and ecdsa-p256 added to map what WinCNG supports.

Each job uses a unique `t-<run_id>-<attempt>-<os>-` ref prefix and a `gh api` cleanup step with
`if: always()`. After every run, the only ref left besides `main` is the permanent `fixture-root` tag.
The results are from runs 37326384464, 37328472353, 37329509499 and 37330225072 (2026-10-05). The last run was
fully green on both OSes.

| # | check | Linux | Windows | evidence / exact text |
|---|---|---|---|---|
| R1 | HTTPS push of ONE lightweight tag, ONE annotated tag, `refs/notes/semantic-release-<tag>`, and a branch, one push each | ✅ | ✅ | Each push gives per-ref status `None`. `ls-remote` shows `tag`, `tag^{}` (peeled to the commit), the notes ref and the branch. |
| R1b | only the named tag is pushed | ✅ | ✅ | A local decoy tag on the same commit is absent from the remote. |
| R2 | tag-clobber guard (`push_negotiation` refuses `refs/tags/*` with a non-zero `src`) | ✅ | ✅ | `push()` returns `Err` with `class=None code=GenericError msg=semoxide: tag refs/tags/<t> already exists on remote`, and the remote tag is unchanged. **Without** the guard, GitHub accepts the fast-forward move: `Ok([(ref, None)])`, and the tag now points at the new commit. |
| R3 | a server-side rejection arrives via `push_update_reference` while `push()` returns Ok | ✅ | ✅ | (a) A push to `refs/pull/999999/head` gives `Ok([("refs/pull/999999/head", Some("deny updating a hidden ref"))])`. (b) A branch that changes `.github/workflows/*.yml`, pushed with `GITHUB_TOKEN`, gives `Some("refusing to allow a GitHub App to create or update workflow `.github/workflows/<f>.yml` without `workflows` permission")`. Branch protection and rulesets are not available here (gap 8). |
| R3b | non-ff push without `+` | ✅ | ✅ | Refused **client-side**, before upload: `Err class=Reference code=NotFastForward msg=cannot push non-fastforwardable reference`. The remote is unchanged. |
| R4 | push probe with the write token (`connect_auth(Push)` + `list()`) | ✅ | ✅ | `Ok`, refs listed, credential callback called once. |
| R4b | push probe with a read-only token (`permissions: contents: read`) | ✅ 403 | ✅ 403 | Linux: `class=Http code=GenericError msg="unexpected http status code: 403"`. Windows: `class=Http code=GenericError msg="request failed with status code: 403"`. A real push fails with the same error. `connect_auth(Fetch)` with the same token works. |
| R4c | push probe with a bad token | ✅ | ✅ | GitHub answers 401, and libgit2 calls the credential callback again. Capped at 3, the 4th call returns our error: `class=None code=GenericError msg="semoxide: authentication failed, giving up"`. Uncapped, libgit2 stops after **15** callback calls: `class=Http code=GenericError msg="too many redirects or authentication replays"`. No error text contains `401`. With no credentials at all, the callback is called once (for the 401 challenge) and its error is returned. |
| R5 | shallow clone with `depth(1)` over HTTPS, then fetch with `depth(i32::MAX)` + `+refs/tags/*` | ✅ | ✅ | Before: `is_shallow=true`, 1 commit. After: `is_shallow=false`, full history, `shallow` file gone, and the root-commit tag `fixture-root` is fetched and reachable. |
| R6 | rollback: delete only this run's tag (`:refs/tags/<tag>`) | ✅ | ✅ | Status `None`. That tag is gone; the other tag and the branch from the same run are untouched. |
| R7 | SSH to `git@github.com:` with a write deploy key, ed25519, via **agent** | ✅ (ssh-agent) | ✅ (OpenSSH agent pipe) | probe, clone, push tag, delete tag |
| R7b | SSH with ed25519, ecdsa and rsa keys in **OpenSSH** format, from a **file** | ✅ all | ❌ all | Windows: `class=Ssh code=GenericError msg="failed to authenticate SSH session: "` (empty libssh2 detail); callback called once. |
| R7c | the same keys from **memory** | ✅ all | ❌ **hang** | Windows: `connect_auth` never returns. The first run hung for more than 8 minutes and was cancelled. The tests now run each case on a thread with a 30 s watchdog. |
| R7d | RSA key in **PEM** format (PKCS#1, from `ssh-keygen -p -m PEM`), from file and from memory | ✅ | ✅ | the only key form WinCNG accepts |
| R7e | SSH handshake stability (60 × connect + list) | ✅ 60/60 | ⚠️ 59/60 | Windows sometimes fails with `class=Ssh msg="failed to start SSH session: Unable to exchange encryption keys"`. This was seen 3 times in about 200 Windows handshakes and never on Linux. One retry fixed it each time. |
| R7f | GitHub host key checked in `certificate_check` (`CertHostkey::hash_sha256` vs `api.github.com/meta`) | ✅ ECDSA key | ✅ RSA key | WinCNG negotiates the RSA host key and OpenSSL the ECDSA one. Both match the published SHA256 fingerprints. |

What this means for the product:

- Check the per-ref status of every push (R3).
- Treat a bare per-ref `failed` as retryable once (gap 9).
- Keep the tag guard (R2).
- Classify a 403 by `class=Http` plus `403` in the message, never by the exact text (R4b).
- Cap credential retries in our own callback (R4c).
- On Windows, prefer HTTPS or the ssh-agent. For SSH key files there, require PEM RSA keys or document that limit.
- Never pass an OpenSSH-format key from memory on Windows; it hangs (R7c).
- Retry the SSH connect once on `Unable to exchange encryption keys` (R7e).

## Still needs a real remote

- A rejection from branch protection or rulesets. This needs GitHub Pro or a public repo (gap 8).
- A fine-grained PAT. The per-run `GITHUB_TOKEN` covers the same `x-access-token` path.
- GitLab.
- macOS (Secure Transport, libssh2 with OpenSSL).
- Pageant on Windows.
- Proxies: the WinHTTP system proxy vs `http.proxy` via `ProxyOptions`.
