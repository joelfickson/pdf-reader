## Summary

What changed and why. Link the issue it closes, for example `Closes #12`.

## Test plan

- [ ] `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked`, `cargo test --locked -- --ignored` (for app changes)
- [ ] `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm build` in `website/` (for website changes)
- [ ] Checked the change in the running app or site, and restarted the app after rebuilding
- [ ] For layout or visual changes, the mock was approved in the linked issue before this PR

Platforms tested (macOS arm64, macOS x64, Windows x64), and anything you could not test:
