# Decisions (ADRs)

- [0001](decisions/0001-compatibility-stance.md): familiar, not compatible; `semoxide migrate` for `.releaserc`
- [0002](decisions/0002-config-format.md): TOML only, layered config
- [0003](decisions/0003-monorepo-scope.md): hybrid monorepo model; v1 handles independent units only
- [0004](decisions/0004-template-engine.md): minijinja; `{version}` for `tag_format`
- [0005](decisions/0005-dry-run.md): dry-run needs no push rights; `--verify-push` opt-in
- [0006](decisions/0006-cc-parser.md): wrap `git-conventional` first
- [0007](decisions/0007-bump-defaults.md): semantic-release default bump table
- [0008](decisions/0008-initial-version.md): first release 1.0.0, configurable
- [0009](decisions/0009-commit-back.md): tags only by default; configuring the `git` plugin turns commit-back on
- [0010](decisions/0010-plugin-architecture.md): plugins in separate repos; socket protocol in its own versioned repo 
- [0011](decisions/0011-git-backend.md): git2 only, with tag and push-status guards
- [0012](decisions/0012-partial-failure.md): tag first; `rollback` step on later failure
