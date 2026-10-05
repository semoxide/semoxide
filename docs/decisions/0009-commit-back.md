# 0009 Commit back to the repo
Status: accepted (2026-10-05)

- **Default: tags only.** semoxide never commits to the repo.
- **Exception:** a monorepo whose units depend on each other ([ADR 0003](0003-monorepo-scope.md)). Dependents' manifests have to be updated, so commit-back is required, and it's enabled only for that setup.
- The user docs site ([REQUIREMENTS](../REQUIREMENTS.md)) must document the drawbacks of commit-back:
  - extra release commits in history
  - CI re-trigger loops (needs a `[skip ci]` convention)
  - branch protection must allow the bot to push
  - races with concurrent pushes, which fail the release
  - commit signing requirements

Why: tags only avoids every drawback above. semantic-release's own git plugin advises against committing back ([dependencies §6](../research/dependencies.md)).
