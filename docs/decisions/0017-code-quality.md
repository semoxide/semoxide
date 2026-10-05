# 0017 Code quality
Status: accepted (2026-10-05). Decided step by step. Findings are fixed in the code. Rules and thresholds are never loosened without the user's explicit approval.

## Decided
- **Git hooks via qlty** (`qlty githooks install`): pre-commit auto-formats; pre-push runs qlty's checks plus `cargo nextest run`. Hooks are local conveniences; CI is the gate ([ADR 0015](0015-testing.md)). The qlty CLI is BSL 1.1 licensed (fine for use as a dev tool).
- **Tools and where they run:**

| Tool | Catches | Runs |
|---|---|---|
| rustfmt | formatting | pre-commit (qlty) + CI |
| clippy: `pedantic` on, selected `restriction` lints, `clippy.toml` bans with reasons (e.g. `std::env::var`, printing in the library, bare `Command::new`) | bugs, style, architecture rules | pre-push (qlty) + CI, warnings as errors |
| rustc + rustdoc lints (`missing_docs` on published crates, broken doc links) | undocumented API | CI |
| qlty maintainability (complexity, duplication, smells) | complex or duplicated code | pre-push + CI |
| qlty security: Gitleaks/TruffleHog, OSV-Scanner/Trivy, Semgrep | secrets, vulnerable deps, SAST | CI (secrets also pre-push) |
| cargo-deny | licenses, RustSec, banned/duplicate deps, sources | CI |
| cargo-shear | unused deps | CI |
| dependency budget (max `Cargo.lock` packages; number set once code exists) | dependency bloat | CI |
| typos | spelling (code, docs, messages) | pre-commit + CI |
| cargo-semver-checks | breaking changes in published crates | CI |
| cargo-mutants | tests that test nothing | `--in-diff` on PRs for the pure crates (gating once the baseline is clean, [CODE-ARCHITECTURE A2](../CODE-ARCHITECTURE.md#10-approaches)); full runs scheduled |
| zizmor | insecure GitHub Actions workflows | CI |
