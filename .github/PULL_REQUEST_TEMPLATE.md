<!--
PR title must follow Conventional Commits, e.g.
  feat(core): add cursor pagination links
  fix(axum): reject invalid Content-Type before extraction
-->

## Summary

<!-- What does this change and why? The diff shows what; explain the why. -->

## Related issue

<!-- Closes #123, or "none". -->

## Checklist

- [ ] PR title follows Conventional Commits (`<type>(<scope>): <description>`)
- [ ] Tests cover the new behaviour
- [ ] `cargo fmt --all` and `cargo clippy --workspace --all-targets --all-features -- -D warnings` pass
- [ ] `cargo test --workspace` passes
- [ ] Public API changes are reflected in the rustdoc and the guide under `docs/`
- [ ] `CHANGELOG.md` updated (user-facing changes)
- [ ] SemVer impact considered (breaking changes require a major-version bump on the `1.x` line)
