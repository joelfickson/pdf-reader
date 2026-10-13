import argparse
import plistlib
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


def copy_file(source, destination):
    with tempfile.TemporaryDirectory(dir=destination.parent) as temporary:
        staged = Path(temporary) / destination.name
        shutil.copy2(source, staged)
        staged.replace(destination)


def main():
    parser = argparse.ArgumentParser(description="Build a local macOS application bundle")
    parser.add_argument("--debug", action="store_true")
    args = parser.parse_args()
    if sys.platform != "darwin":
        raise SystemExit("macOS packaging requires a Mac")
    root = Path(__file__).resolve().parent.parent
    library = root / "vendor/pdfium/lib/libpdfium.dylib"
    if not library.is_file():
        raise SystemExit("Install the PDF engine first: uv run scripts/setup_pdfium.py")
    command = ["cargo", "build", "--locked"]
    if not args.debug:
        command.append("--release")
    subprocess.run(command, cwd=root, check=True)
    profile = "debug" if args.debug else "release"
    name = "PDF Reader Debug" if args.debug else "PDF Reader"
    executable_name = "pdf-reader-debug" if args.debug else "pdf-reader"
    bundle = root / "target" / profile / f"{name}.app"
    contents = bundle / "Contents"
    executable_dir = contents / "MacOS"
    frameworks = contents / "Frameworks"
    resources = contents / "Resources"
    for directory in [executable_dir, frameworks, resources]:
        directory.mkdir(parents=True, exist_ok=True)
    copy_file(root / "target" / profile / "pdf-reader", executable_dir / executable_name)
    copy_file(library, frameworks / library.name)
    shutil.copytree(root / "vendor/pdfium/licenses", resources / "licenses/pdfium", dirs_exist_ok=True)
    shutil.copy2(root / "vendor/pdfium/LICENSE", resources / "licenses/PDFium-LICENSE")
    shutil.copy2(root / "LICENSE", resources / "licenses/Application-LICENSE")
    info = {
        "CFBundleName": name,
        "CFBundleDisplayName": name,
        "CFBundleExecutable": executable_name,
        "CFBundleIdentifier": "org.pdfreader.desktop.debug" if args.debug else "org.pdfreader.desktop",
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": "0.1.0",
        "CFBundleVersion": "1",
        "NSHighResolutionCapable": True,
        "NSPrincipalClass": "NSApplication",
    }
    with (contents / "Info.plist").open("wb") as output:
        plistlib.dump(info, output)
    print(f"Local development bundle: {bundle}")


if __name__ == "__main__":
    main()
