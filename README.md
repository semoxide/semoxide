# semoxide

A fully automated release tool written in Rust, inspired by [semantic-release](https://github.com/semantic-release/semantic-release): it reads your commits, picks the next version, tags it, writes release notes and publishes through plugins. It is library-first (embeddable crate + CLI) and opinionated, and will feel familiar to semantic-release users.

> **Status: planning.** Nothing usable yet. Design and decisions live in [`docs/`](docs/), starting from [`CLAUDE.md`](CLAUDE.md).

semoxide is not affiliated with semantic-release.

## Development

Setup per clone:

1. [rustup](https://rustup.rs/): the toolchain is pinned in `rust-toolchain.toml` and installs itself.
2. Install [qlty](https://docs.qlty.sh/), [typos](https://github.com/crate-ci/typos), [cargo-nextest](https://nexte.st/) and [lefthook](https://lefthook.dev/), then run `lefthook install` (git hooks: format, lint, test).
3. Optional, for [Zed](https://zed.dev/): the extensions markdownlint, typos, toml and github-actions. Project settings are in `.zed/`.

Contributing (people and AI agents): [AGENTS.md](AGENTS.md).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in semoxide by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
