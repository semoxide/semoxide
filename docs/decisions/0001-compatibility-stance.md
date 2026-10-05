# 0001 Compatibility stance
Status: accepted (2026-10-05)

- Familiar, not compatible: the same concepts and step names as semantic-release, but semoxide's own config and plugin protocol.
- `semoxide migrate` converts `.releaserc` to [semoxide.toml](0002-config-format.md) where it can, and reports anything it can't convert.
- No JS plugin bridge.

Why: the gap identified in [competitors](../research/competitors.md) requires an easy migration path. Runtime compatibility would bring back the JS-only issues listed in [porting gaps](../PORTING-GAPS.md).
