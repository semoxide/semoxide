# Specifications

- [semantic-release](specs/SEMANTIC-RELEASE-SPEC.md): workflow, steps, config, plugins, channels
- [SemVer 2.0.0](specs/SEMVER-SPEC.md): version format, precedence, bump rules
- [Conventional Commits 1.0.0](specs/CONVENTIONAL-COMMITS-SPEC.md): commit message grammar

Upstream implementation references (how the code actually behaves):

- [Core](specs/UPSTREAM-CORE.md): pipeline, git operations G1–G22, errors, tests
- [commit-analyzer](specs/UPSTREAM-COMMIT-ANALYZER.md): parsing, release rules, bump algorithm
- [release-notes-generator](specs/UPSTREAM-RELEASE-NOTES.md): context, writer, templates
- [Dependencies](specs/UPSTREAM-DEPENDENCIES.md): conventional-changelog, env-ci, git/changelog/exec plugins
- [github](specs/UPSTREAM-GITHUB.md): API calls, publish/success/fail
- [npm](specs/UPSTREAM-NPM.md): auth, registry, per-step commands
