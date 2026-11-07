import type { Metadata } from "next";
import Link from "next/link";
import { formatDate, formatMegabytes } from "@/lib/format";
import { getLatestRelease, getReleases, type Release } from "@/lib/github";
import {
  CHECKSUMS_ASSET,
  latestDownloadUrl,
  PLATFORM_ASSETS,
  type Platform,
} from "@/lib/site";
import { type BuildRow, BuildRows } from "./_components/build-rows";
import { InstallSteps } from "./_components/install-steps";
import { PlatformCta } from "./_components/platform-cta";
import "./download.css";

export const metadata: Metadata = {
  title: "Download",
  description:
    "Download Agentic PDF for macOS on Apple silicon or Intel, or Windows x64. Install steps, checksums and earlier versions.",
};

const PLATFORM_LABELS: Record<Platform, { system: string; variant: string }> = {
  "macos-arm64": { system: "macOS", variant: "Apple silicon" },
  "macos-x64": { system: "macOS", variant: "Intel" },
  "windows-x64": { system: "Windows", variant: "x64" },
};

const PLATFORM_ORDER: Platform[] = ["macos-arm64", "macos-x64", "windows-x64"];

const BYTES_PER_MEGABYTE = 1_000_000;

function buildRows(release: Release): BuildRow[] {
  return PLATFORM_ORDER.map((platform) => {
    const file = PLATFORM_ASSETS[platform];
    const asset = release.assets.find((candidate) => candidate.name === file);
    if (!asset) {
      throw new Error(`Release ${release.tag} is missing ${file}`);
    }
    return {
      platform,
      ...PLATFORM_LABELS[platform],
      file,
      size: formatMegabytes(asset.sizeBytes),
      url: latestDownloadUrl(file),
    };
  });
}

function typicalMegabytes(release: Release): number {
  const sizes = PLATFORM_ORDER.flatMap((platform) =>
    release.assets
      .filter((asset) => asset.name === PLATFORM_ASSETS[platform])
      .map((asset) => asset.sizeBytes),
  );
  const average =
    sizes.reduce((total, size) => total + size, 0) / Math.max(sizes.length, 1);
  return Math.round(average / BYTES_PER_MEGABYTE);
}

function releaseKind(
  release: Release,
  latestTag: string,
): { label: string; className: string } {
  if (release.tag === latestTag) {
    return { label: "Latest", className: "kind latest" };
  }
  if (release.prerelease) {
    return { label: "Pre-release", className: "kind pre" };
  }
  return { label: "Release", className: "kind" };
}

function SourceText() {
  return (
    <>
      <p>
        Install Rust stable and uv. On macOS, install Xcode and select its
        command-line tools. If GPUI reports a missing Metal compiler, run{" "}
        <code>xcodebuild -downloadComponent MetalToolchain</code>.
      </p>
      <p>
        On Windows, use the MSVC Rust toolchain and install Visual Studio Build
        Tools with the Desktop development with C++ workload and the Windows
        SDK.
      </p>
      <p>
        The setup script downloads PDFium for your platform and verifies its
        pinned SHA-256 digest. It needs Python 3.12 or newer, which uv can
        provide.
      </p>
    </>
  );
}

function SourceCommands() {
  return (
    <div className="source-commands">
      <h3>Clone</h3>
      <pre>
        {
          "git clone https://github.com/joelfickson/pdf-reader.git\ncd pdf-reader"
        }
      </pre>
      <h3>Run</h3>
      <pre>{"uv run scripts/setup_pdfium.py\ncargo run --release"}</pre>
      <h3>Package a macOS app</h3>
      <pre>
        {
          'uv run scripts/package_macos.py\nopen "target/release/PDF Reader.app"'
        }
      </pre>
    </div>
  );
}

export default async function DownloadPage() {
  const [latest, releases] = await Promise.all([
    getLatestRelease(),
    getReleases(),
  ]);
  const builds = buildRows(latest);
  return (
    <div className="download-page">
      <div className="hero">
        <div className="wrap">
          <div className="eyebrow">Download</div>
          <h1>
            Get Agentic PDF <em>{latest.tag}</em>
          </h1>
          <p className="lede">
            Free and open source. One zip per platform, about{" "}
            {typicalMegabytes(latest)} MB, with the PDF engine included. No
            account, no installer.
          </p>
          <PlatformCta />
          <div className="meta">
            <span>
              <i />
              Latest release, {formatDate(latest.publishedAt)}
            </span>
            <span>MIT license</span>
          </div>
        </div>
      </div>

      <section id="builds" className="builds-section">
        <div className="wrap">
          <BuildRows builds={builds} />
        </div>
      </section>

      <section id="install">
        <div className="wrap">
          <div className="sec-head">
            <div className="eyebrow">Install</div>
            <h2>
              Two minutes, <em>no installer.</em>
            </h2>
            <p>
              Current builds are not yet signed and notarized, so your system
              asks you to confirm the first launch. After that, it opens like
              any other app.
            </p>
          </div>
          <InstallSteps
            macos={
              <>
                <h3>macOS</h3>
                <ol>
                  <li>
                    <span>
                      Download the zip for your Mac and open it to unzip.
                    </span>
                  </li>
                  <li>
                    <span>
                      Move <b>PDF Reader</b> to your{" "}
                      <span className="path">Applications</span> folder.
                    </span>
                  </li>
                  <li>
                    <span>
                      Open it. This build is not notarized, so macOS blocks the
                      first launch.
                    </span>
                  </li>
                  <li>
                    <span>
                      Open <b>System Settings</b>, go to{" "}
                      <b>Privacy and Security</b>, and click <b>Open Anyway</b>{" "}
                      next to the PDF Reader message.
                    </span>
                  </li>
                </ol>
              </>
            }
            windows={
              <>
                <h3>Windows</h3>
                <ol>
                  <li>
                    <span>Download the zip and extract it to a folder.</span>
                  </li>
                  <li>
                    <span>
                      Keep <span className="path">pdfium.dll</span> in the same
                      folder as <span className="path">pdf-reader.exe</span>.
                    </span>
                  </li>
                  <li>
                    <span>
                      Run <span className="path">pdf-reader.exe</span>. This
                      build is not code signed, so SmartScreen may warn on first
                      launch.
                    </span>
                  </li>
                  <li>
                    <span>
                      Click <b>More info</b>, then <b>Run anyway</b>.
                    </span>
                  </li>
                </ol>
              </>
            }
          />
        </div>
      </section>

      <section id="verify">
        <div className="wrap">
          <div className="sec-head">
            <div className="eyebrow">Verify</div>
            <h2>
              Check what <em>you downloaded.</em>
            </h2>
            <p>
              Every release publishes SHA-256 checksums. Compare the output with
              the matching line in{" "}
              <a
                className="inline-link"
                href={latestDownloadUrl(CHECKSUMS_ASSET)}
              >
                {CHECKSUMS_ASSET}
              </a>
              .
            </p>
          </div>
          <div className="verify">
            <div>
              <h3>macOS, in Terminal</h3>
              <pre>{`cd ~/Downloads\nshasum -a 256 ${PLATFORM_ASSETS["macos-arm64"]}`}</pre>
            </div>
            <div>
              <h3>Windows, in PowerShell</h3>
              <pre>{`cd $HOME\\Downloads\nGet-FileHash ${PLATFORM_ASSETS["windows-x64"]} -Algorithm SHA256`}</pre>
            </div>
          </div>
        </div>
      </section>

      <section id="older">
        <div className="wrap">
          <div className="sec-head">
            <div className="eyebrow">All releases</div>
            <h2>
              Earlier <em>versions.</em>
            </h2>
            <p>
              Every build stays available on GitHub with its notes and
              checksums. See the{" "}
              <Link className="inline-link" href="/changelog">
                changelog
              </Link>{" "}
              for what changed.
            </p>
          </div>
          <div className="releases">
            {releases.map((release) => {
              const kind = releaseKind(release, latest.tag);
              return (
                <a className="release" href={release.url} key={release.tag}>
                  <span className="tag">{release.tag}</span>
                  <span className="date">
                    {formatDate(release.publishedAt)}
                  </span>
                  <span className={kind.className}>{kind.label}</span>
                </a>
              );
            })}
          </div>
        </div>
      </section>

      <section id="source">
        <div className="wrap source source-wide">
          <div>
            <div className="eyebrow">Build from source</div>
            <h2 className="source-title">
              Prefer to <em>build it yourself?</em>
            </h2>
            <SourceText />
          </div>
          <div>
            <SourceCommands />
          </div>
        </div>
        <div className="wrap source-fold">
          <details>
            <summary>
              <h2 className="source-title">
                Prefer to <em>build it yourself?</em>
              </h2>
            </summary>
            <SourceText />
            <SourceCommands />
          </details>
        </div>
      </section>
    </div>
  );
}
