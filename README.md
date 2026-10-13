# PDF Reader

A local desktop PDF reader for macOS and Windows, built with Rust, GPUI, and PDFium.

This is an early development build. It supports dropping or opening multiple PDFs in tabs, viewing one page at a time, page navigation, zoom, fit to width, a collapsible thumbnail sidebar, and selecting/copying text. Search, OCR, annotations, password entry, and editing are not implemented.

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

Use this release app for normal reading and performance checks. `--debug` creates a separate `PDF Reader Debug.app` for development. The bundles are unsigned local builds.

Open a document immediately:

```sh
cargo run --release -- tests/fixtures/reading.pdf
```

The setup script selects the Mac or Windows archive for the current architecture, verifies its pinned SHA-256 digest, and extracts PDFium with its upstream license notices to `vendor/pdfium`. It requires Python 3.12 or newer; uv can provision Python if needed. PDFium is a native C++ dependency. The first Cargo build downloads and compiles GPUI and its dependencies.

The app looks for the PDFium library using `PDFIUM_LIBRARY_PATH`, next to its executable, in a macOS app's `Frameworks` directory, or in this checkout's `vendor/pdfium/lib` and `vendor/pdfium/bin` directories. `PDFIUM_LIBRARY_PATH` can point to the library file or its directory.

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
| Close active tab | Command+W | Ctrl+W |
| Next / previous tab | Ctrl+Tab / Ctrl+Shift+Tab | Ctrl+Tab / Ctrl+Shift+Tab |
| Copy selected text | Command+C | Ctrl+C |
| Select all text on current page | Command+A | Ctrl+A |
| Clear selection | Escape | Escape |
| Quit | Command+Q | Ctrl+Q |

Click a sidebar thumbnail to jump to that page. Use the scroll wheel or trackpad to move within a page. Drop one or several PDFs anywhere in the window to open them in tabs. Reopening the same path focuses its existing tab. Each tab retains its page, zoom, and scroll position. Closing the final tab returns to the empty reader; the close shortcut from an empty reader closes the window.

Drag across text to select it, then use the Copy text button or keyboard shortcut. Selection is limited to the current page and follows the PDF's text order. Scanned pages without a text layer require OCR, which is not included. The title bar uses the platform's standard window controls.

## Architecture and limits

Source is organized by responsibility:

```text
src/
  main.rs             Command-line entry point
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
    tests.rs          GPUI interaction regression tests
    view/             Tab strip, toolbar, sidebar, canvas, and text highlights
  pdf/
    mod.rs            Render requests, results, and rasterization
    library.rs        Native PDFium discovery and loading
    worker.rs         Background scheduling and bounded document handles
    text.rs           Character extraction, coordinates, and hit testing
```

Tests live beside the behavior they verify. `tests/fixtures` contains sample documents, `scripts` contains development setup utilities, and `design` contains the approved local mock. Build output and local verification files are ignored by Git.

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

The ignored test exercises the native PDF engine and requires the setup script. The check command reports engine initialization, document parsing, first-page rendering, and bitmap size for the supplied file. It does not measure time until the GUI presents the page or total process memory.

The sample PDF is original test content, reproducible with `uv run scripts/create_sample.py`. GitHub Actions is configured to build and test on Mac and Windows. Windows runtime behavior still requires testing on a Windows machine.

## Contributing

Issues and pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for development checks and bug-report guidance.

## License and distribution

Application source: MIT. GPUI: Apache-2.0. pdfium-render: MIT OR Apache-2.0. PDFium and its bundled third-party components have separate license notices supplied in the downloaded archive. Preserve those notices when packaging or redistributing the native library.

The macOS packaging script includes PDFium and its license notices in a local app bundle. Signed installers and Windows distribution packaging are not implemented. No remote repository, release, or hosted service is required to run it.
