# Agentic PDF

[![Check](https://github.com/joelfickson/pdf-reader/actions/workflows/check.yml/badge.svg)](https://github.com/joelfickson/pdf-reader/actions/workflows/check.yml) [![Website](https://github.com/joelfickson/pdf-reader/actions/workflows/website.yml/badge.svg)](https://github.com/joelfickson/pdf-reader/actions/workflows/website.yml)

A fast, native PDF reader for macOS and Windows, built with Rust, GPUI, and PDFium. Next, the AI agents you already use will read alongside you through the Agent Client Protocol ([issue #18](https://github.com/joelfickson/pdf-reader/issues/18)).

Website and downloads: <https://agentic-pdf-eta.vercel.app>

This is an early development build. It supports dropping or opening multiple PDFs in tabs, viewing one page at a time, page navigation, zoom, fit to width, a collapsible thumbnail sidebar, selecting and copying text, searching the whole document, opening PDFs from Finder, and reopening recent files at the page and zoom you left them. OCR, annotations, password entry, editing, and the agent panel are not implemented yet. The app is still named PDF Reader in its bundle and menus.

## Run

Install Rust stable and uv. On macOS, install Xcode and select its command-line tools. If GPUI reports a missing Metal compiler, run `xcodebuild -downloadComponent MetalToolchain`. On Windows, use the MSVC Rust toolchain and install Visual Studio Build Tools with the Desktop development with C++ workload and Windows SDK.

Clone the repository:

```sh
git clone https://github.com/joelfickson/pdf-reader.git
cd pdf-reader
```

From this directory:

```sh
uv run scripts/setup_pdfium.py
cargo run --release
```

For a local macOS app with PDFium included:

```sh
uv run scripts/package_macos.py
open "target/release/PDF Reader.app"
```

Tagged commits matching `v*` are packaged by GitHub Actions for macOS arm64, macOS x64, and Windows x64. The workflow publishes release assets containing the application, PDFium, and dependency license notices.

Use this release app for normal reading and performance checks. `--debug` creates a separate `PDF Reader Debug.app` for development. The bundles are unsigned local builds.

The setup script selects the Mac or Windows archive for the current architecture, verifies its pinned SHA-256 digest, and extracts PDFium with its upstream license notices to `vendor/pdfium`. It requires Python 3.12 or newer; uv can provision Python if needed. PDFium is a native C++ dependency. The first Cargo build downloads and compiles GPUI and its dependencies.

The app looks for the PDFium library using `PDFIUM_LIBRARY_PATH`, next to its executable, in a macOS app's `Frameworks` directory, or in this checkout's `vendor/pdfium/lib` and `vendor/pdfium/bin` directories. `PDFIUM_LIBRARY_PATH` can point to the library file or its directory.

## Command line

| Command | What it does |
| --- | --- |
| `pdf-reader a.pdf b.pdf` | Opens each file in its own tab |
| `pdf-reader --check <file.pdf>` | Opens and renders the first page without a window, prints one line of timings, and exits |
| `pdf-reader --version` | Prints the name and version, such as `pdf-reader 0.1.2`, and exits |
| `pdf-reader mcp` | Runs the MCP server for AI agents (see below) |

From a source checkout:

```sh
cargo run --release -- tests/fixtures/reading.pdf tests/fixtures/outline.pdf
cargo run --release -- --check tests/fixtures/reading.pdf
cargo run --release -- --version
```

`--check` prints these fields:

| Field | Meaning |
| --- | --- |
| `pages` | Page count |
| `engine_ms` | Time to load the PDFium library |
| `open_ms` | Time to parse the document |
| `first_page_ms` | Time from start until the first page is rendered, including the two steps above |
| `bitmap_bytes` | Size of the first page's bitmap |
| `dimensions` | First page bitmap size in pixels |

## Use with AI agents (MCP)

`pdf-reader mcp` runs a read-only [Model Context Protocol](https://modelcontextprotocol.io) server over stdio, so coding agents such as Claude Code and Codex can read your PDFs. It needs no window and no account.

| Tool | What it returns |
| --- | --- |
| `document_info` | Page count, metadata, and pages without a text layer |
| `get_outline` | The table of contents with nesting levels and page numbers |
| `get_text` | Text of consecutive pages up to `max_chars` (default 20,000, at most 100,000), with `next_page` to continue |
| `search` | Matches with their page and surrounding text, ignoring case and accents by default |

Page numbers are 1-based. The server only opens `.pdf` files, never writes to them, and bounds every response.

Add it to Claude Code or Codex. With the macOS app installed in Applications:

```sh
claude mcp add agentic-pdf -- "/Applications/PDF Reader.app/Contents/MacOS/pdf-reader" mcp
codex mcp add agentic-pdf -- "/Applications/PDF Reader.app/Contents/MacOS/pdf-reader" mcp
```

From a source build, use `target/release/pdf-reader mcp`. On Windows, use the path to `pdf-reader.exe` with `pdfium.dll` beside it. The Windows server has not been tested yet.

The PDF content only leaves your computer if the agent you use sends it to its model provider.

## Controls

| Action | Mac | Windows |
| --- | --- | --- |
| Open PDFs | Command+O | Ctrl+O |
| Previous / next page | Left / Right | Left / Right |
| Previous / next page | Page Up / Page Down | Page Up / Page Down |
| Zoom in | Command+= | Ctrl+= |
| Zoom out | Command+- | Ctrl+- |
| Fit to width | Command+0 | Ctrl+0 |
| Toggle sidebar | Command+B | Ctrl+B |
| Find in document | Command+F | Ctrl+F |
| Next / previous match | Enter / Shift+Enter | Enter / Shift+Enter |
| Go to page | Command+G | Ctrl+G |
| First / last page | Home / End, Command+Home / Command+End | Home / End, Ctrl+Home / Ctrl+End |
| Back / forward | Command+[ / Command+] | Alt+Left / Alt+Right |
| Command palette | Command+K | Ctrl+K |
| Close active tab | Command+W | Ctrl+W |
| Next / previous tab | Ctrl+Tab / Ctrl+Shift+Tab | Ctrl+Tab / Ctrl+Shift+Tab |
| Copy selected text | Command+C | Ctrl+C |
| Select all text on current page | Command+A | Ctrl+A |
| Clear selection | Escape | Escape |
| Quit | Command+Q | Ctrl+Q |

Click a sidebar thumbnail to jump to that page. Links inside a PDF go to their target page and position; web and email links open in your browser or mail app, and other link types are not opened. Back and forward retrace jumps from links, go to page, first and last page, the command palette, and thumbnails. Each tab keeps its own history. Use the scroll wheel or trackpad to move within a page. Drop one or several PDFs anywhere in the window to open them in tabs. Reopening the same path focuses its existing tab. Each tab retains its page, zoom, and scroll position. Closing the final tab returns to the empty reader; the close shortcut from an empty reader closes the window.

Search runs on the background PDF thread and lists every match in the Results panel. It folds case and accents unless Match case is on, matches phrases across line breaks, and supports whole words. Matches on the visible page are highlighted.

Drag across text to select it, then use the Copy text button or keyboard shortcut. Selection is limited to the current page and follows the PDF's text order. Scanned pages without a text layer require OCR, which is not included. The title bar uses the platform's standard window controls.

## Architecture and limits

Source is organized by responsibility:

```text
src/
  main.rs             Entry point that dispatches the parsed command
  cli.rs              Command-line parsing (paths, --check, --version, and mcp)
  app.rs              Window startup, native menus, key bindings
  reader/
    mod.rs            Window state, worker event handling, shared caches
    actions.rs        Typed UI actions
    cache.rs          Byte-bounded image and text caches
    images.rs         Pixel validation and GPU image ownership
    tabs.rs           Document identity and per-tab reading state
    navigation.rs     File picking, tab lifecycle, page and zoom actions
    scheduling.rs     Cached view lookup and worker requests
    selection.rs      Selection ranges and clipboard actions
    search.rs         Find bar state, stepping between matches, revealing results
    tests.rs          GPUI interaction regression tests
    view/             Tab strip, toolbar, sidebar, canvas, and text highlights
  pdf/
    mod.rs            Render requests, results, and rasterization
    library.rs        Native PDFium discovery and loading
    worker.rs         Background scheduling and bounded document handles
    text.rs           Character extraction, coordinates, and hit testing
    search.rs         Case-folded, cross-line matching over page characters
    tools.rs          Transport-independent document info, outline, text ranges, and search
  mcp/
    engine.rs         Dedicated PDFium thread with a bounded document cache
    server.rs         MCP tool definitions over stdio
website/              Next.js site for downloads, the agent roadmap, and the changelog
```

Tests live beside the behavior they verify. `tests/fixtures` contains sample documents, `scripts` contains development setup utilities, and `website` contains the public site. Design mocks are kept locally and are not committed. Build output and local verification files are ignored by Git.

PDFium runs on one dedicated thread. The pending request is replaced when a newer view is requested; results from obsolete requests are discarded. An in-progress PDFium call is not forcibly interrupted. The visible page renders first, followed by its text and visible thumbnails. Up to four document handles are retained across tabs; evicted handles reopen lazily. Closing a tab releases its handle and cached content.

The sidebar uses GPUI's virtual list. Only requested thumbnails are rendered, with a shared limit of 64 images and 16 MiB of bitmap data. The page cache retains up to eight previously viewed images within 64 MiB, in addition to the current page. Returning to a cached page reuses its bitmap and GPU image. Cache keys include the document session, page, zoom or fitted width, and display scale. Evicted images are removed from GPUI's GPU atlas. While zooming, the previous bitmap stays visible until its replacement arrives. Thumbnail visibility changes do not restart a pending render of the same main page.

Text geometry is cached for up to 32 pages within 16 MiB, shared across tabs. Extraction is limited to 100,000 characters per page. Rotated page coordinates are transformed to match the rendered bitmap.

Raster dimensions are bounded to avoid unlimited zoom allocations. Bitmap budgets are not limits on total process memory: PDFium, GPU textures, active rendering, and the current page consume additional memory.

## Validation

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test -- --ignored
cargo run --release -- --check tests/fixtures/reading.pdf
```

For the website:

```sh
cd website
pnpm install
pnpm lint && pnpm typecheck && pnpm test && pnpm build
```

The ignored test exercises the native PDF engine and requires the setup script. The check command reports engine initialization, document parsing, first-page rendering, and bitmap size for the supplied file. It does not measure time until the GUI presents the page or total process memory.

The sample PDF is original test content, reproducible with `uv run scripts/create_sample.py`. GitHub Actions is configured to build and test on Mac and Windows. Windows runtime behavior still requires testing on a Windows machine.

## Contributing

Issues and pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for development checks and bug-report guidance.

## License and distribution

Application source: MIT. GPUI: Apache-2.0. pdfium-render: MIT OR Apache-2.0. PDFium and its bundled third-party components have separate license notices supplied in the downloaded archive. Preserve those notices when packaging or redistributing the native library.

The macOS packaging script includes PDFium and its license notices in a local app bundle. Signed installers and Windows distribution packaging are not implemented. No remote repository, release, or hosted service is required to run it.
