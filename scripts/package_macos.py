import argparse
import os
import plistlib
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path


def copy_file(source, destination):
    with tempfile.TemporaryDirectory(dir=destination.parent) as temporary:
        staged = Path(temporary) / destination.name
        shutil.copy2(source, staged)
        staged.replace(destination)


def crate_version(root):
    with (root / "Cargo.toml").open("rb") as manifest:
        return tomllib.load(manifest)["package"]["version"]


def sign(paths, identity):
    command = ["codesign", "--force", "--sign", identity]
    if identity != "-":
        command += ["--options", "runtime", "--timestamp"]
    for path in paths:
        subprocess.run([*command, str(path)], check=True)


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
    icon = root / "assets/brand/pdf-reader.icns"
    if not icon.is_file():
        raise SystemExit(f"Missing application icon: {icon}")
    command = ["cargo", "build", "--locked"]
    if not args.debug:
        command.append("--release")
    subprocess.run(command, cwd=root, check=True)
    version = crate_version(root)
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
    copy_file(icon, resources / icon.name)
    shutil.copytree(root / "vendor/pdfium/licenses", resources / "licenses/pdfium", dirs_exist_ok=True)
    shutil.copy2(root / "vendor/pdfium/LICENSE", resources / "licenses/PDFium-LICENSE")
    shutil.copy2(root / "LICENSE", resources / "licenses/Application-LICENSE")
    info = {
        "CFBundleName": name,
        "CFBundleDisplayName": name,
        "CFBundleExecutable": executable_name,
        "CFBundleIdentifier": "org.pdfreader.desktop.debug" if args.debug else "org.pdfreader.desktop",
        "CFBundleIconFile": icon.name,
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": version,
        "CFBundleVersion": version,
        "CFBundleDocumentTypes": [
            {
                "CFBundleTypeName": "PDF Document",
                "CFBundleTypeRole": "Viewer",
                "LSHandlerRank": "Alternate",
                "LSItemContentTypes": ["com.adobe.pdf"],
            }
        ],
        "NSHighResolutionCapable": True,
        "NSPrincipalClass": "NSApplication",
    }
    with (contents / "Info.plist").open("wb") as output:
        plistlib.dump(info, output)
    identity = os.environ.get("MACOS_SIGNING_IDENTITY") or "-"
    sign([frameworks / library.name, bundle], identity)
    subprocess.run(["codesign", "--verify", "--deep", "--strict", str(bundle)], check=True)
    print(f"Local development bundle: {bundle}")


if __name__ == "__main__":
    main()
