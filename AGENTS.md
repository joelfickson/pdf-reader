# Project instructions

- Build an open-source PDF reader for macOS and Windows using Rust and GPUI.
- Prioritize fast opening and bounded memory usage. Measure performance instead of inventing targets or results.
- Before implementing or changing UI layout, create a standalone local HTML mock and wait for explicit user approval.
- Keep previews and outputs local. Do not publish without explicit permission.
- Use whitespace, typography, alignment, and hairline rules to group content. No card framing or decorative rules attached to labels.
- Do not add code comments or docstrings. Preserve existing comments unless changing the associated code requires otherwise. Preserve tool directives and license headers.
- Do not print credentials.
- Prefer service CLIs when available. Use uv for Python.
- Announce command batches and their purpose before executing them.
- Preserve other sessions' changes. Stage only task-owned paths explicitly.
- Use no emojis or em dashes in prose.
- Write production-quality, idiomatic Rust. Keep the binary entry point small, organize source by responsibility, and keep files within cohesive feature modules. Do not accumulate the implementation in main.rs or scatter unrelated files at the project root.
- Handle external failures explicitly, bound background work and caches, and test meaningful behavior. Keep platform-dependent validation claims accurate.
- Verify actual native file picking and repeated page navigation before calling the reader usable. Reuse rendered pages within a byte-bounded cache; do not discard them on every page change.
- In GPUI 0.2.2, set a button's hover style once. Applying another hover style to a helper that already sets one causes a runtime panic.
- Give debug and release app bundles distinct names and identifiers, and verify the release bundle after rebuilding it.

- Restore saved tab scroll offsets after an evicted bitmap renders; a temporary loading view can otherwise clamp them to zero. Cover cached and uncached restoration in interaction tests.
- Verify PDF text geometry against rendered ink, including rotated pages. Preserve Unicode and line breaks when copying.
- When a newly built feature appears missing, compare the running process start time with the bundle modification time before changing code. Restart the outdated reader and preserve the current document and reading position where possible; rebuilding alone does not update an existing process.
- The product is Agentic PDF. Use "Agentic PDF" wherever people read the name and `agenticpdf` for the crate, executable, release files, environment variable prefix (`AGENTICPDF_`), and MCP server name. The bundle identifier is `app.agenticpdf.desktop` and must not change again, because the updater rejects a different identifier. The former name, PDF Reader, appears only where old settings and old release files are still recognised.
- The site's domain will be agenticpdf.app. Do not link to it until it is registered and attached; use the Vercel address until then.
