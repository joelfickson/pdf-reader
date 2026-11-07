"use client";

import type { Platform } from "@/lib/site";
import { useDetectedPlatform } from "./use-detected-platform";

export type BuildRow = {
  platform: Platform;
  system: string;
  variant: string;
  file: string;
  size: string;
  url: string;
};

type BuildRowsProps = {
  builds: BuildRow[];
};

export function BuildRows({ builds }: BuildRowsProps) {
  const detected = useDetectedPlatform();
  return (
    <div className="builds">
      {builds.map((build) => (
        <div
          className={
            build.platform === detected ? "build recommended" : "build"
          }
          key={build.platform}
        >
          <span className="os">
            {build.system}
            <small>{build.variant}</small>
          </span>
          <span className="file">{build.file}</span>
          <span className="size">{build.size}</span>
          <a className="btn small glass" href={build.url}>
            Download
          </a>
        </div>
      ))}
    </div>
  );
}
