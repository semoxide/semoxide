# 0011 Git backend
Status: accepted (2026-10-05)

- git2 (vendored libgit2, `https` and `ssh` features) is the only git backend. No git CLI and no gix.
- Required product guards: never move an existing remote tag; check every pushed ref's status; cap credential retries.

Evidence: [git2 PoC](../../poc/git2-ops/README.md); gaps are listed there. Real-remote checks run against a private sandbox repo.
