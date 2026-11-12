# /// script
# requires-python = ">=3.12"
# dependencies = ["psutil>=5.9; sys_platform == 'win32'"]
# ///
import argparse
import os
import platform
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
from create_sample import PAGES, create_pdf  # noqa: E402

MARKER = "first_page_painted_ms="
TIMEOUT_SECONDS = 60


def binary():
    name = "agenticpdf.exe" if os.name == "nt" else "agenticpdf"
    return ROOT / "target" / "release" / name


def machine():
    if sys.platform == "darwin":
        def sysctl(key):
            return subprocess.run(["sysctl", "-n", key], capture_output=True, text=True).stdout.strip()

        memory = int(sysctl("hw.memsize") or 0) // 2**30
        return f"{sysctl('machdep.cpu.brand_string')}, {memory} GB, macOS {platform.mac_ver()[0]}"
    return f"{platform.processor() or platform.machine()}, {platform.system()} {platform.release()}"


def documents(directory, extra):
    small = ROOT / "tests/fixtures/reading.pdf"
    generated = []
    for count in (500, 2000):
        path = directory / f"generated-{count}.pdf"
        if not path.exists():
            create_pdf(path, [PAGES[index % len(PAGES)] for index in range(count)])
        generated.append(path)
    return [small, *generated, *extra]


def page_count(path):
    result = subprocess.run([binary(), "--check", path], capture_output=True, text=True, check=True)
    fields = dict(part.split("=", 1) for part in result.stdout.split())
    return int(fields["pages"])


def launch(path, home):
    environment = dict(os.environ, AGENTICPDF_BENCH="first-page", HOME=str(home), APPDATA=str(home))
    started = time.perf_counter()
    process = subprocess.Popen([binary(), path], stdout=subprocess.PIPE, text=True, env=environment)
    reported = None
    stages = {}
    peak = 0
    if os.name == "nt":
        import psutil

        watched = psutil.Process(process.pid)
    for line in process.stdout:
        name, _, value = line.strip().partition("_ms=")
        if value:
            stages.setdefault(name, float(value))
        if line.startswith(MARKER):
            reported = float(line[len(MARKER):])
            wall = (time.perf_counter() - started) * 1000
            if os.name == "nt":
                peak = watched.memory_info().peak_wset
    if os.name == "nt":
        status = process.wait(TIMEOUT_SECONDS)
    else:
        _, status, usage = os.wait4(process.pid, 0)
        process.returncode = status
        peak = usage.ru_maxrss if sys.platform == "darwin" else usage.ru_maxrss * 1024
    if reported is None or status != 0:
        raise RuntimeError(f"{path.name}: no first-page report (exit status {status})")
    return reported, wall, peak, stages


def summary(values, unit, scale=1.0):
    values = [value / scale for value in values]
    return f"{statistics.median(values):.0f} {unit} ({min(values):.0f} to {max(values):.0f})"


def main():
    parser = argparse.ArgumentParser(description="Measure launch to first painted page and peak memory.")
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--work-dir", type=Path, default=Path(tempfile.gettempdir()) / "agenticpdf-bench")
    parser.add_argument("pdf", nargs="*", type=Path, help="Extra PDFs to measure")
    args = parser.parse_args()
    if not binary().is_file():
        sys.exit("Build first: cargo build --release")
    args.work_dir.mkdir(parents=True, exist_ok=True)
    print(f"Machine: {machine()}")
    print(f"Binary: {binary()}, {args.runs} runs after one warm-up launch per document")
    print()
    print("| Document | Pages | Size | First page painted (in process) | Launch to report (wall clock) | Peak resident memory |")
    print("| --- | --- | --- | --- | --- | --- |")
    breakdown = []
    for path in documents(args.work_dir, args.pdf):
        with tempfile.TemporaryDirectory() as home:
            launch(path, Path(home))
            results = [launch(path, Path(home)) for _ in range(args.runs)]
        reported, wall, peak, stages = zip(*results)
        size = path.stat().st_size / 2**20
        print(
            f"| {path.name} | {page_count(path)} | {size:.1f} MB | {summary(reported, 'ms')} | "
            f"{summary(wall, 'ms')} | {summary(peak, 'MB', 2**20)} |",
            flush=True,
        )
        names = sorted(set().union(*stages), key=lambda name: statistics.median(run.get(name, 0) for run in stages))
        breakdown.append((path.name, {name: statistics.median(run[name] for run in stages if name in run) for name in names}))
    print()
    print("Median milliseconds after launch at which each stage first happened:")
    print()
    for name, medians in breakdown:
        print(f"- {name}: " + ", ".join(f"{stage} {value:.0f}" for stage, value in medians.items()))


if __name__ == "__main__":
    main()
