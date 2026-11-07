"use client";

import { useSyncExternalStore } from "react";
import { latestDownloadUrl, PLATFORM_ASSETS } from "@/lib/site";

type DownloadButtonProps = {
  className?: string;
};

function isWindows(): boolean {
  return /Windows/i.test(navigator.userAgent);
}

function subscribe(): () => void {
  return () => {};
}

export function DownloadButton({
  className = "btn primary",
}: DownloadButtonProps) {
  const windows = useSyncExternalStore(subscribe, isWindows, () => false);
  const asset = windows
    ? PLATFORM_ASSETS["windows-x64"]
    : PLATFORM_ASSETS["macos-arm64"];
  return (
    <a className={className} href={latestDownloadUrl(asset)}>
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
      <span>{windows ? "Download for Windows" : "Download for Mac"}</span>
    </a>
  );
}
