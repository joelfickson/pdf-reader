# Contributing

Issues and pull requests are welcome. Please read the [code of conduct](CODE_OF_CONDUCT.md). Report security problems privately as described in [SECURITY.md](SECURITY.md), never in a public issue.

Good places to start are issues labelled [good first issue](https://github.com/joelfickson/pdf-reader/labels/good%20first%20issue) and [help wanted](https://github.com/joelfickson/pdf-reader/labels/help%20wanted). Comment on an issue before starting larger work so it is not done twice.

The history was rewritten on 7 November 2026 to remove a folder of design files. If you cloned or forked before that date, make a fresh clone instead of pulling. This is an early development build; see README.md for supported features and current limitations, and SPEC.md for the project scope.

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

For layout or visual changes, start with a standalone HTML mock and get it approved before writing the real UI. Mocks are not committed: attach the HTML file or screenshots to the issue (issues that need one carry the `needs mock` label), and link the approved mock from your pull request. Locally, keep mocks in the repository's `design/` folder, which Git ignores.

Code conventions used throughout the project:

- Write self-explanatory code instead of comments. Comments are only for tool directives and license headers.
- Use clear names, small modules organized by responsibility, and keep `main.rs` small.
- Handle external failures explicitly, bound background work and caches, and add tests for behavior changes.
- Measure performance claims and state the machine, document, and method.
- Write user-facing text without emojis or em dashes.

## Website

The site in `website/` is a Next.js app. Install Node.js 20.9 or newer and pnpm, then:

```sh
cd website
pnpm install
pnpm dev
```

Before submitting website changes, run `pnpm lint`, `pnpm typecheck`, `pnpm test`, and `pnpm build`. Release data comes from the GitHub API; set `GITHUB_TOKEN` if you hit rate limits. Every claim on the site must be true for the current release, and site-specific rules are in `website/AGENTS.md`.

## Bug reports

Include the operating system, architecture, commit or build, reproduction steps, and expected versus observed behavior. For rendering or text-selection issues, attach a small PDF you have permission to share, ideally an authored reproduction without personal or confidential content. Verify that the running application was restarted after rebuilding.

## License

Contributions are provided under the project's MIT license. Preserve required license notices for dependencies and redistributed native libraries.
