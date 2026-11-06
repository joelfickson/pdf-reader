# PDF Reader application icon

The icon shows a page with a reading line on it and a dot beside the page. The page is the document, and the dot is the agent working next to it, where the agent panel sits in the app. The icon is defined by two SVG masters, and the checked-in `.icns` and `.ico` files are derived from them.

## Files

- `pdf-reader.svg`: standard master, used for 48 px and larger. The reading line fades in from the left.
- `pdf-reader-small.svg`: optically adjusted master, used for 16, 24 and 32 px. It uses a solid, heavier line, a larger dot and a wider page so the mark stays legible at small sizes.
- `pdf-reader.icns`: macOS icon container, with the rounded-square presentation.
- `pdf-reader.ico`: Windows icon container, with a rounded-corner presentation.

Edit the SVG masters only. Regenerate the derived files after any master change.

## Colour

- Tile: `#111418`, fully opaque.
- Page: `#F4F1E8`.
- Reading line: `#1B1E22`.
- Dot: `#F2C230`, the app's first highlight colour.
- The same artwork is used on macOS and Windows. There are no theme-dependent variants.

## Native containers

macOS (`pdf-reader.icns`): the master is scaled to 0.8047 and placed at offset 100 px on a 1024 px canvas. The tile is clipped to an 824 px rounded square with corner radius 185 px. Outside the rounded square is transparent.

The container is built by `iconutil` from a standard iconset: `icon_16x16`, `icon_32x32`, `icon_128x128`, `icon_256x256` and `icon_512x512`, each at 1x and 2x. That gives pixel sizes of 16, 32, 64, 128, 256, 512 and 1024. The small master is used for 16 and 32 px, and the standard master for everything larger.

Windows (`pdf-reader.ico`): the master is clipped to a full-square 1024 px canvas with corner radius 112 px. Corners are transparent.

Sizes: 16, 24, 32, 48, 64 and 256 px, stored as 32-bit RGBA PNG entries. The small master is used for 16, 24 and 32 px.

## Generation

Run on macOS with `rsvg-convert` installed (`brew install librsvg`):

```
uv run scripts/build_icons.py
```

The script renders every size directly from the vector master, applies the macOS or Windows presentation, builds the `.icns` with `iconutil`, and writes the `.ico` as a standard header with one 16-byte directory entry per size, followed by PNG payloads.

Validate the `.icns` with `iconutil -c iconset` and inspect the extracted images at 16 and 32 px. Validate the `.ico` by checking its directory entries and alpha channel.

The build embeds `pdf-reader.ico` into the Windows executable as resource ID 1 through `build.rs` and `winresource`. This is the resource ID that GPUI 0.2.2 loads for its window icon. The macOS packager copies `pdf-reader.icns` into `Contents/Resources` and sets `CFBundleIconFile`.
