# PDF Reader specification

## Purpose

Build an open-source desktop PDF reader for macOS and Windows using Rust, GPUI, and PDFium. Prioritize fast document opening, responsive navigation, and bounded memory. Measure performance rather than assuming it.

## Implemented scope

- Open multiple local PDFs through the native file picker, external file drops, the command line, or Finder (Open With, and double-click when PDF Reader is the default viewer). On macOS the bundle declares PDF as a viewable document type, and files sent while the app is running open as tabs in the existing window.
- Serve read-only PDF tools to AI agents with `pdf-reader mcp`, a Model Context Protocol server over stdio built on the official rmcp SDK: `document_info`, `get_outline`, `get_text` (bounded by `max_chars` with a `next_page` cursor), and `search`. It accepts any `.pdf` path, never writes, reports pages without a text layer instead of skipping them, runs PDFium on one dedicated thread, and keeps at most four documents open. Attaching to the running reader and the in-app agent panel are planned in issue 18.
- Remember up to 20 recently opened files with their last page and zoom, restore both on reopen (clamping to the last page if the file has shrunk), and list them in File > Open Recent and on the empty screen. Files that no longer exist are removed when chosen. The list is stored locally in `recent.tsv` under Application Support (macOS) or `%APPDATA%` (Windows), separately for debug and release builds, and is written atomically on open, tab switch, tab close, and quit.
- Show documents in tabs and focus an existing tab when its canonical path is reopened.
- Preserve each tab's page, zoom, canvas scroll, and sidebar scroll position.
- Display one page at a time with previous/next navigation, zoom, and fit-to-width.
- Provide a collapsible virtualized thumbnail sidebar.
- Select text on the current page by dragging; copy with the toolbar or platform shortcut.
- Go to a page with Cmd G (Ctrl G on Windows) or the page field, and to the first or last page with Home and End or Cmd Home and Cmd End.
- Support select-all on the current page and Escape to clear selection.
- Search the whole document from the find bar (Cmd F, Ctrl F on Windows): case-insensitive by default, with Match case and Whole word options. Matches span line breaks and fold Unicode case. Enter and Shift+Enter, the arrow keys, or the bar's buttons step between matches and move to their page. Matches on the visible page are highlighted, the bar shows the position ("3 of 41"), and the Results panel lists matches by page with surrounding text.
- Transform text geometry to match rotated pages and preserve Unicode and line breaks when copying.
- Report file, rendering, and text extraction errors without terminating the reader.

OCR, annotations, password entry, continuous scrolling, document editing, signed installers, and Windows distribution packaging remain out of scope for this version.

## Architecture

The binary entry point handles arguments. app.rs owns window startup, native menus, and key bindings. reader/ owns window state, per-document tabs, navigation, selection, caching, scheduling, and view composition. pdf/ owns PDFium discovery, rasterization, text extraction, and the background worker.

PDFium runs on one dedicated worker thread. A single pending request replaces obsolete work; generation identifiers reject stale results. Main-page rendering precedes text extraction and thumbnail rendering. In-progress native calls are not forcibly interrupted.

Search has its own slot and generation on the same worker thread, so render requests never cancel it. The worker scans pages in slices of at most 30 ms and yields to any pending render job or newer search between pages. Matching runs on page characters only and returns glyph ranges that index the same glyphs as text extraction, so highlights reuse the page text cache. A search stops at 5,000 matches; pages over the 100,000-character text cap are skipped and reported. The Results panel lists the first 200 matches.

The worker retains up to four document handles and reopens evicted handles lazily. Shared caches retain up to eight previous page images within 64 MiB, 64 thumbnails within 16 MiB, and text for 32 pages within 16 MiB. The active page and other engine/GPU allocations consume additional memory. Text extraction is capped at 100,000 characters per page. Closing a tab releases its cached content and document handle.

## Interface

Use a warm neutral canvas, compact toolbar and tab strip, blue interaction accent, collapsible sidebar, and ample reading space. Group controls with whitespace, typography, alignment, and hairline rules. Approved design mocks are kept locally in an ignored design/ folder and are not committed.

## Validation

Formatting, strict Clippy, and 41 tests (2 of them PDFium-backed and ignored by default) pass locally on macOS. Tests cover bounded caches, request cancellation, document-handle reuse and eviction, malformed-file recovery, Unicode text, rotated geometry overlapping rendered ink, image-only pages, and search matching (line breaks, Unicode case, whole word, match case, limits) against both synthetic text and the PDFium fixture, including a page render submitted mid-search. GPUI interaction tests cover the external-drop event route, path filtering and deduplication, selection and clipboard actions, stale-event rejection, cached and uncached tab scroll restoration, tab closing, the find flow (debounced search, stale-result rejection, stepping and wrapping between matches, option changes, and clearing on close), and recent files (position restore, clamping, recording on close, pruning missing files, clearing). Unit tests cover the recent-file store (ordering, cap, round trip, malformed lines, unreadable store), file URL decoding, and the Open Recent menu.

The installed macOS release bundle appears in Open With for PDFs. Opening a PDF through Launch Services (`open -a`, which sends the same open-documents event as Finder) opened it when the app was closed, and opened a second file as a new tab in the same process while it was running. Quitting wrote `recent.tsv`, a relaunch listed both files under Recent, and reopening a file with a stored position restored page 3 at 125 percent. Double-clicking was not tested, because it requires making PDF Reader the default PDF viewer.

The packaged macOS engine opens and renders the authored fixtures. Full native interaction verification for the tabs/selection build remains incomplete because desktop automation loses window access after the native picker closes. Windows runtime validation is pending. CI is configured for macOS and Windows; compilation and unit tests alone do not establish Windows runtime behavior.

The MCP server was measured on an Apple M1 Max (32 GB) with Rust in Action (457 pages, 18 MB), release build, driving `pdf-reader mcp` over stdio from a Python client, three runs: initialize answered in about 8 ms once warm (309 to 540 ms on the first cold launch), `document_info` scanned all pages in 391 to 429 ms, `get_outline` returned 336 entries in 13 to 15 ms, `get_text` returned 17 pages (19,718 characters) in about 7 ms, and `search` found 125 matches in 355 to 365 ms. Adding the server grew the release binary from 9.39 MB to 11.83 MB. Claude Code 2.1.319 used the server through `--mcp-config` to read the outline fixture, find a phrase split across a line break, and report a page without text.

Search was measured on an Apple M1 Max (32 GB) with Rust in Action (457 pages, 18 MB), release build, using a temporary worker harness timing submission to completion over three runs: a full scan took 425 to 620 ms, the first matches arrived after about 40 ms, a 5,000-match query stopped at the cap after about 300 ms, and a page render submitted mid-search completed in 11 to 18 ms including render time.

Time to first page was measured on an Apple M1 Max (32 GB, macOS 27.0), release build, with `uv run scripts/bench_first_page.py`: one warm-up launch and five measured launches per document, each with an empty Recent list. The app reports, from the start of `main` until the first frame that paints the page, and the script reads the process's peak resident memory from `wait4`. Medians (range): the 3-page fixture 253 ms (247 to 261), 148 MB; generated 500 pages 262 ms (240 to 269), 148 MB; generated 2,000 pages 258 ms (248 to 275), 148 MB; C and GUI Programming (156 pages, 7.5 MB) 260 ms (256 to 273), 170 MB; Rust in Action (457 pages, 18 MB) 287 ms (283 to 295), 184 MB. Page count does not change the result. About 250 ms is spent before any page can be shown: GPUI is ready after about 75 ms, the window after about 150 ms, and the first frame (the loading view, including GPU setup) holds the main thread until about 250 ms, so a page rendered earlier waits for it. The document opens and its first page starts rendering before the window finishes opening. PDF work is on the critical path only when the first page takes longer than about 100 ms to render: Rust in Action's cover renders in about 130 ms at 2x. A test render at 1x took about 110 ms and moved the first paint from about 305 ms to about 270 ms in that build; a low-resolution first render was not adopted because it saves about 35 ms on such pages, nothing on others, and costs a second full render and a visible swap. Thumbnails already render after the page and text, so deferring them does not change the first paint.

Continuous scrolling was measured on the same Apple M1 Max (120 Hz display), release build, with `PDF_READER_BENCH=scroll` scrolling the whole document at 120 px per frame inside `/usr/bin/time -l`, one run each: generated 500 pages took 4,894 frames with a median interval of 8.33 ms, 95th percentile 8.66 ms, maximum 13.6 ms, no frame over 33 ms, 42 frames showing a page before its image was ready, and 344 MB peak resident memory; generated 2,000 pages: 19,587 frames, median 8.33 ms, 95th percentile 9.39 ms, maximum 21.5 ms, none over 33 ms, 581 frames with an unrendered page, 387 MB; Rust in Action (457 pages): 4,348 frames, median 8.33 ms, 95th percentile 9.97 ms, maximum 22.8 ms, none over 33 ms, 75 frames with an unrendered page, 492 MB. Memory does not grow with page count: four times as many pages added 43 MB. Rendered pages stay inside the existing caches (8 pages and 64 MB of page images, 32 pages and 16 MB of text).

Engine-only inspection reports initialization, parsing, rendering, and bitmap size. It does not measure time until a page is visible or total process memory. Performance reports must name the machine, document, measurement method, and memory metric.
