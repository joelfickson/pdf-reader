import hashlib
import io
import platform
import tarfile
import urllib.request
from pathlib import Path

RELEASE = "7881"
ARCHIVES = {
    ("Darwin", "arm64"): ("mac-arm64", "52e94ca5aa8847934330daf3f8150c190682c5ca93831468794f8b90d4392e40"),
    ("Darwin", "x86_64"): ("mac-x64", "6dedf83990e0e3d6b7c93c9e7589c5a126b0ae14b7464d76120cff7a26afb18b"),
    ("Windows", "AMD64"): ("win-x64", "73cc0de638ac2095e7445bf56a38200a5b7c7ca0e9f4ba144598f2457377ac08"),
    ("Windows", "ARM64"): ("win-arm64", "d3035d4d2cacac6ecd1a2ece197a3d702a1b2a58466276b9f870b8cb278a9d84"),
}


def main():
    system = (platform.system(), platform.machine())
    if system not in ARCHIVES:
        raise SystemExit(f"Unsupported platform: {system}")
    archive, expected = ARCHIVES[system]
    destination = Path(__file__).resolve().parent.parent / "vendor" / "pdfium"
    url = f"https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/{RELEASE}/pdfium-{archive}.tgz"
    print(f"Downloading PDFium {RELEASE} for {archive}")
    with urllib.request.urlopen(url, timeout=60) as response:
        data = response.read()
    if hashlib.sha256(data).hexdigest() != expected:
        raise SystemExit("PDFium archive checksum mismatch")
    destination.mkdir(parents=True, exist_ok=True)
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as package:
        package.extractall(destination, filter="data")
    print(f"Verified PDFium and its license notices installed in {destination}")


if __name__ == "__main__":
    main()
