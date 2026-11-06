import re
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

SMALL_PIXELS = {16, 24, 32}
ICONSET = [
    ("icon_16x16.png", 16),
    ("icon_16x16@2x.png", 32),
    ("icon_32x32.png", 32),
    ("icon_32x32@2x.png", 64),
    ("icon_128x128.png", 128),
    ("icon_128x128@2x.png", 256),
    ("icon_256x256.png", 256),
    ("icon_256x256@2x.png", 512),
    ("icon_512x512.png", 512),
    ("icon_512x512@2x.png", 1024),
]
ICO_SIZES = [16, 24, 32, 48, 64, 256]


def artwork(master):
    text = master.read_text()
    match = re.search(r"<svg\b[^>]*>(.*)</svg>", text, re.DOTALL)
    if match is None:
        raise SystemExit(f"{master} is not an SVG document")
    return match.group(1)


def macos_presentation(inner):
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">'
        '<defs><clipPath id="plate"><rect x="100" y="100" width="824" height="824" rx="185"/></clipPath></defs>'
        f'<g clip-path="url(#plate)"><g transform="translate(100 100) scale(0.8047)">{inner}</g></g>'
        "</svg>"
    )


def windows_presentation(inner):
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">'
        '<defs><clipPath id="tile"><rect width="1024" height="1024" rx="112"/></clipPath></defs>'
        f'<g clip-path="url(#tile)">{inner}</g>'
        "</svg>"
    )


def rasterise(document, pixels):
    result = subprocess.run(
        ["rsvg-convert", "--width", str(pixels), "--height", str(pixels), "--format", "png"],
        input=document.encode(),
        capture_output=True,
        check=True,
    )
    if not result.stdout.startswith(b"\x89PNG\r\n\x1a\n"):
        raise SystemExit(f"rsvg-convert produced no PNG at {pixels} px")
    return result.stdout


def render(presentation, standard, small, pixels):
    inner = small if pixels in SMALL_PIXELS else standard
    return rasterise(presentation(inner), pixels)


def write_icns(path, standard, small):
    with tempfile.TemporaryDirectory() as temporary:
        iconset = Path(temporary) / "pdf-reader.iconset"
        iconset.mkdir()
        for name, pixels in ICONSET:
            (iconset / name).write_bytes(render(macos_presentation, standard, small, pixels))
        subprocess.run(["iconutil", "--convert", "icns", str(iconset), "--output", str(path)], check=True)


def write_ico(path, standard, small):
    images = [render(windows_presentation, standard, small, pixels) for pixels in ICO_SIZES]
    header = struct.pack("<HHH", 0, 1, len(images))
    offset = len(header) + 16 * len(images)
    directory = b""
    for pixels, png in zip(ICO_SIZES, images):
        edge = 0 if pixels >= 256 else pixels
        directory += struct.pack("<BBBBHHII", edge, edge, 0, 0, 1, 32, len(png), offset)
        offset += len(png)
    path.write_bytes(header + directory + b"".join(images))


def main():
    if sys.platform != "darwin":
        raise SystemExit("icon generation requires macOS for iconutil")
    if shutil.which("rsvg-convert") is None:
        raise SystemExit("rsvg-convert is required (brew install librsvg)")
    brand = Path(__file__).resolve().parent.parent / "assets/brand"
    standard = artwork(brand / "pdf-reader.svg")
    small = artwork(brand / "pdf-reader-small.svg")
    write_icns(brand / "pdf-reader.icns", standard, small)
    write_ico(brand / "pdf-reader.ico", standard, small)
    print(f"wrote {brand / 'pdf-reader.icns'} and {brand / 'pdf-reader.ico'}")


if __name__ == "__main__":
    main()
