import re
import subprocess
import sys
import tomllib
from pathlib import Path


def main():
    if len(sys.argv) != 2 or sys.argv[1] not in {"beta", "rc", "stable"}:
        raise SystemExit("release channel must be beta, rc, or stable")

    channel = sys.argv[1]
    package = tomllib.loads(Path("Cargo.toml").read_text())["package"]
    version = package["version"]

    if re.fullmatch(r"\d+\.\d+\.\d+", version) is None:
        raise SystemExit("Cargo.toml package version must be a stable semantic version")

    tags = subprocess.run(
        ["git", "tag", "--list", f"v{version}*"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.splitlines()

    if channel == "stable":
        tag = f"v{version}"
        if tag in tags:
            raise SystemExit(f"release tag {tag} already exists; update Cargo.toml first")
        print(tag)
        return

    prefix = f"v{version}-{channel}."
    sequence = [
        int(match.group(1))
        for tag in tags
        if (match := re.fullmatch(re.escape(prefix) + r"(\d+)", tag)) is not None
    ]
    print(f"{prefix}{max(sequence, default=0) + 1}")


if __name__ == "__main__":
    main()
