# Contributing

Issues and pull requests are welcome. This is an early development build; see README.md for supported features and current limitations, and SPEC.md for the project scope.

## Development

Follow the platform setup in README.md, then run:

```sh
uv run scripts/setup_pdfium.py
cargo run
```

Before submitting a change:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo test --locked -- --ignored
```

The ignored tests require the PDFium library installed by the setup script. Reproduce the authored PDF fixtures with `uv run scripts/create_sample.py`.

Keep changes focused and explain the behavior changed and how it was tested. Keep PDF work off the UI thread, bound allocations and caches, and reject stale results. Add regression tests for behavior changes. Follow the module layout in README.md and project instructions in AGENTS.md.

For UI layout changes, include a standalone local HTML mock for design review before implementing the native layout.

## Bug reports

Include the operating system, architecture, commit or build, reproduction steps, and expected versus observed behavior. For rendering or text-selection issues, attach a small PDF you have permission to share, ideally an authored reproduction without personal or confidential content. Verify that the running application was restarted after rebuilding.

## License

Contributions are provided under the project's MIT license. Preserve required license notices for dependencies and redistributed native libraries.
