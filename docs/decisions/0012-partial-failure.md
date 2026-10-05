# 0012 Partial failure
Status: accepted (2026-10-05)

- semantic-release's order is kept: the tag is pushed before publish.
- If a later step fails, semoxide runs the `rollback` step ([ADR 0010](0010-plugin-architecture.md)): each plugin undoes its own work, and semoxide deletes the pushed tag via its Git service.
- Some publishes can't be undone (npm unpublish is restricted). Those plugins log a warning. How a partial failure is reported (exit code etc.) is still undecided.
- Deleting the tag needs delete rights on the remote. The user docs site must document this.

Why: upstream has no answer for this ([#896](https://github.com/semantic-release/semantic-release/issues/896), [#2381](https://github.com/semantic-release/semantic-release/issues/2381)).
