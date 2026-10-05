# 0012 Partial failure
Status: accepted (2026-10-05)

- semantic-release's order is kept: the tag is pushed before publish.
- If a later step fails, semoxide runs the `rollback` step ([ADR 0010](0010-plugin-architecture.md)): each plugin undoes its own work.
- **Tag deletion:** the core itself (not a plugin and not the Git service) deletes only the tag it pushed in this run. Plugins can never delete tags.
- **Without delete rights:** rollback still runs for the other plugins, and the run ends as a partial failure. The message names the tag left behind and how to delete it manually.
- Some publishes can't be undone (npm unpublish is restricted). Those plugins log a warning. How a partial failure is reported (exit code etc.) is still undecided.
- The user docs site must document that deleting the tag needs delete rights on the remote.

Why: upstream has no answer for this ([#896](https://github.com/semantic-release/semantic-release/issues/896), [#2381](https://github.com/semantic-release/semantic-release/issues/2381)).
