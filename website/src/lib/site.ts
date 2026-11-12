export const REPO_SLUG = "joelfickson/agenticpdf";
export const REPO_URL = `https://github.com/${REPO_SLUG}`;
export const RELEASES_URL = `${REPO_URL}/releases`;
export const AGENT_ISSUE_URL = `${REPO_URL}/issues/18`;
export const ISSUES_URL = `${REPO_URL}/issues`;

export type Platform = "macos-arm64" | "macos-x64" | "windows-x64";

export const PLATFORM_ASSETS: Record<Platform, string> = {
  "macos-arm64": "agenticpdf-macos-arm64.zip",
  "macos-x64": "agenticpdf-macos-x64.zip",
  "windows-x64": "agenticpdf-windows-x64.zip",
};

export const FORMER_PLATFORM_ASSETS: Record<Platform, string> = {
  "macos-arm64": "pdf-reader-macos-arm64.zip",
  "macos-x64": "pdf-reader-macos-x64.zip",
  "windows-x64": "pdf-reader-windows-x64.zip",
};

export function isPlatformAsset(platform: Platform, name: string): boolean {
  return (
    name === PLATFORM_ASSETS[platform] ||
    name === FORMER_PLATFORM_ASSETS[platform]
  );
}

export const CHECKSUMS_ASSET = "SHA256SUMS.txt";

export function latestDownloadUrl(asset: string): string {
  return `${RELEASES_URL}/latest/download/${asset}`;
}
