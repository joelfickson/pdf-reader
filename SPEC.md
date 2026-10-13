# PDF Reader specification

## Purpose

Build an open-source desktop PDF reader for macOS and Windows using Rust, GPUI, and PDFium. Prioritize fast document opening, responsive navigation, and bounded memory. Measure performance rather than assuming it.

## Implemented scope

- Open multiple local PDFs through the native file picker or external file drops.
- Show documents in tabs and focus an existing tab when its canonical path is reopened.
- Preserve each tab's page, zoom, canvas scroll, and sidebar scroll position.
- Display one page at a time with previous/next navigation, zoom, and fit-to-width.
- Provide a collapsible virtualized thumbnail sidebar.
- Select text on the current page by dragging; copy with the toolbar or platform shortcut.
- Support select-all on the current page and Escape to clear selection.
- Transform text geometry to match rotated pages and preserve Unicode and line breaks when copying.
- Report file, rendering, and text extraction errors without terminating the reader.

Search, OCR, annotations, password entry, continuous scrolling, document editing, signed installers, and Windows distribution packaging remain out of scope for this version.

## Architecture

The binary entry point handles arguments. app.rs owns window startup, native menus, and key bindings. reader/ owns window state, per-document tabs, navigation, selection, caching, scheduling, and view composition. pdf/ owns PDFium discovery, rasterization, text extraction, and the background worker.

PDFium runs on one dedicated worker thread. A single pending request replaces obsolete work; generation identifiers reject stale results. Main-page rendering precedes text extraction and thumbnail rendering. In-progress native calls are not forcibly interrupted.

The worker retains up to four document handles and reopens evicted handles lazily. Shared caches retain up to eight previous page images within 64 MiB, 64 thumbnails within 16 MiB, and text for 32 pages within 16 MiB. The active page and other engine/GPU allocations consume additional memory. Text extraction is capped at 100,000 characters per page. Closing a tab releases its cached content and document handle.

## Interface

Use a warm neutral canvas, compact toolbar and tab strip, blue interaction accent, collapsible sidebar, and ample reading space. Group controls with whitespace, typography, alignment, and hairline rules. Approved design sources are in design/.

## Validation

Formatting, strict Clippy, and 12 tests pass locally on macOS. Tests cover bounded caches, request cancellation, document-handle reuse and eviction, malformed-file recovery, Unicode text, rotated geometry overlapping rendered ink, and image-only pages. GPUI interaction tests cover the external-drop event route, path filtering and deduplication, selection and clipboard actions, stale-event rejection, cached and uncached tab scroll restoration, and tab closing.

The packaged macOS engine opens and renders the authored fixtures. Full native interaction verification for the tabs/selection build remains incomplete because desktop automation loses window access after the native picker closes. Windows runtime validation is pending. CI is configured for macOS and Windows; compilation and unit tests alone do not establish Windows runtime behavior.

Engine-only inspection reports initialization, parsing, rendering, and bitmap size. It does not measure time until a page is visible or total process memory. Performance reports must name the machine, document, measurement method, and memory metric.
