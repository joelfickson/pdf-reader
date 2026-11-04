# PDF Reader application icon

The open-book icon is defined by two SVG masters. The checked-in `.icns` and `.ico` files are derived from them.

## Files

- `pdf-reader.svg`: standard master, used for 48 px and larger.
- `pdf-reader-small.svg`: optically adjusted master, used for 16, 24 and 32 px. It has a wider gutter and heavier pages so the book remains open at small sizes.
- `pdf-reader.icns`: macOS icon container, with the rounded-square presentation.
- `pdf-reader.ico`: Windows icon container, with a rounded-corner presentation.

Edit the SVG masters only. Regenerate the derived files after any master change.

## Colour

- Tile: `#F4F1E8`, fully opaque.
- Mark: `#1C2330`.
- The same artwork is used on macOS and Windows. There are no theme-dependent variants.

## Native containers

macOS (`pdf-reader.icns`): the standard master is scaled to 0.8047 and placed at offset 100 px on a 1024 px canvas. The tile is clipped to a 824 px rounded square with corner radius 185 px. Outside the rounded square is transparent.

Representations: 16, 32 (16 pt at 2x), 32, 64 (32 pt at 2x), 128, 256 (128 pt at 2x), 256, 512 (256 pt at 2x), 512 and 1024 px. The small master is used for 16 and 32 px. The standard master is used for all other sizes.

Windows (`pdf-reader.ico`): the standard master is clipped to a full-square 1024 px canvas with corner radius 112 px. Corners are transparent.

Sizes: 16, 24, 32, 48, 64 and 256 px, stored as 32-bit RGBA PNG entries. The small master is used for 16, 24 and 32 px.

## Generation

1. Render each size directly from the SVG master with a vector rasteriser (for example `cairosvg`). Do not downscale the 1024 px raster.
2. Apply the macOS or Windows presentation described above before rasterising.
3. Write the `.icns` as a standard container: the `icns` header with total length, then one chunk per representation (`icp4`, `icp5`, `icp6`, `ic07`, `ic08`, `ic09`, `ic10`, `ic11`, `ic12`, `ic13`, `ic14`), each with a 4-byte type, a big-endian length including its 8-byte header, and PNG data.
4. Write the `.ico` as a standard header with one 16-byte directory entry per size, followed by PNG payloads. Width and height are 0 for 256 px.
5. Validate the `.icns` with `iconutil -c iconset` on macOS. Validate the `.ico` by checking its directory entries and alpha channel.

The build embeds `pdf-reader.ico` into the Windows executable as resource ID 1 through `build.rs` and `winresource`. This is the resource ID that GPUI 0.2.2 loads for its window icon. The macOS packager copies `pdf-reader.icns` into `Contents/Resources` and sets `CFBundleIconFile`.
