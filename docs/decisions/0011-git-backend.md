# 0011 Git backend
Status: accepted (2026-10-05); SSH section added the same day

- git2 (vendored libgit2, `https` feature) is the only git library. No git CLI and no gix.
- Required product guards (proven in the [git2 PoC](../../poc/git2-ops/README.md) against real GitHub):
  - never move an existing remote tag
  - check every pushed ref's status
  - cap credential retries
  - match HTTP errors by class and status, not by message text
- **SSH (v1):** git2 is built **without** its `ssh` feature, so there is no libssh2.
  - **Default:** a pure-Rust `russh` transport registered with git2 for `ssh://` and `git@host:` URLs. It supports every key type on every OS, the OpenSSH agent pipe and Pageant on Windows, known_hosts verification, `~/.ssh/config`, and bounded timeouts, and needs no external binary (works in minimal images).
  - **Opt-in:** a transport that runs the system `ssh` (BatchMode, honors `GIT_SSH_COMMAND` / `GIT_SSH`) for OpenSSH-only features such as ProxyJump and FIDO keys.
  - Evidence: [git2-russh PoC](../../poc/git2-russh/README.md), [cost](../../poc/ssh-cost/README.md) (+~4 MB, +~120 crates, +~50% clean build, ~1.2k lines of our code, pre-1.0 and release-candidate crypto dependencies).

Why libssh2 was dropped: on Windows it only accepts PEM RSA key files, it hangs on others, and its handshake fails now and then (libssh2 #804). jj dropped git2 for the same SSH reasons ([revisit research](../research/git-libraries.md)).
