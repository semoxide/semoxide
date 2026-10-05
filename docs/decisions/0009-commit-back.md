# 0009 Commit back to the repo
Status: accepted (2026-10-05), clarified the same day

- **Default: tags only.** Without the `git` plugin, semoxide never commits to the repo.
- **Configuring the `git` plugin turns commit-back on:** it commits and pushes its `assets` ([ADR 0010](0010-plugin-architecture.md)). Configuring it is the user's explicit opt-in.
- **Required case:** a monorepo whose units depend on each other ([ADR 0003](0003-monorepo-scope.md)) needs commit-back to update dependents' manifests.
- The user docs site ([REQUIREMENTS](../REQUIREMENTS.md)) must document the drawbacks of commit-back:
  - extra release commits in history
  - CI re-trigger loops (needs a `[skip ci]` convention)
  - branch protection must allow the bot to push
  - races with concurrent pushes, which fail the release
  - commit signing requirements

Why: tags only avoids every drawback above; semantic-release's own git plugin advises against committing back ([dependencies §6](../research/dependencies.md)).
