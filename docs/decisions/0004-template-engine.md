# 0004 Template engine
Status: accepted (2026-10-05)

- Release notes and message templates use minijinja.
- `tag_format` uses its own tiny `{version}` syntax, because versions have to be parsed back out of tags.
- No JS expressions, which is a documented break from semantic-release's lodash templates.

Why: see [release-notes-generator](../research/release-notes-generator.md) and [dependencies](../research/dependencies.md).
