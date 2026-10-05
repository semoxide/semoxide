# 0009 Commit back to the repo
Status: **open**

Question: may semoxide ever commit to the repo, e.g. to bump manifest versions or write CHANGELOG.md, or do versions live only in tags?

- Tags only is semantic-release core's model. Its git plugin, which does commit back, advises against its own use ([dependencies §6](../research/dependencies.md)).
- Monorepos with units that depend on each other ([ADR 0003](0003-monorepo-scope.md)) need dependents' manifests updated, which requires committing back.
