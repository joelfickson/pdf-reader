"use client";

import { latestDownloadUrl, PLATFORM_ASSETS } from "@/lib/site";
import { useDetectedPlatform } from "./use-detected-platform";

export function PlatformCta() {
  const windows = useDetectedPlatform() === "windows-x64";
  const primaryAsset = windows
    ? PLATFORM_ASSETS["windows-x64"]
    : PLATFORM_ASSETS["macos-arm64"];
  return (
    <div className="ctas">
      <a className="btn primary" href={latestDownloadUrl(primaryAsset)}>
        <svg
          viewBox="0 0 16 16"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.6"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <path d="M8 2v8M5 7l3 3 3-3M3 13h10" />
        </svg>
        <span>
          {windows ? "Download for Windows" : "Download for Mac, Apple silicon"}
        </span>
      </a>
      {windows ? (
        <a className="alt" href="#builds">
          Mac downloads
        </a>
      ) : (
        <a
          className="alt"
          href={latestDownloadUrl(PLATFORM_ASSETS["macos-x64"])}
        >
          Intel Mac instead
        </a>
      )}
    </div>
  );
}
