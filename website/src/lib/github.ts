import { cacheLife, cacheTag } from "next/cache";
import { z } from "zod";
import { type Change, parseMergeCommit, parseReleaseNotes } from "./changes";
import { REPO_SLUG } from "./site";

const API_ROOT = `https://api.github.com/repos/${REPO_SLUG}`;
const REQUEST_TIMEOUT_MS = 10_000;
const RELEASES_PER_PAGE = 100;
const MAX_RELEASE_PAGES = 10;

const assetSchema = z.object({
  name: z.string(),
  size: z.number().int().nonnegative(),
  browser_download_url: z.url(),
});

const releaseSchema = z.object({
  tag_name: z.string(),
  name: z.string().nullable(),
  html_url: z.url(),
  published_at: z.string().nullable(),
  prerelease: z.boolean(),
  draft: z.boolean(),
  body: z.string().nullable(),
  assets: z.array(assetSchema),
});

const compareSchema = z.object({
  html_url: z.url(),
  commits: z.array(z.object({ commit: z.object({ message: z.string() }) })),
});

export type ReleaseAsset = {
  name: string;
  sizeBytes: number;
  url: string;
};

export type Release = {
  tag: string;
  url: string;
  publishedAt: string | null;
  prerelease: boolean;
  assets: ReleaseAsset[];
  changes: Change[];
};

export type Unreleased = {
  compareUrl: string;
  changes: Change[];
};

async function requestJson(path: string): Promise<unknown> {
  const headers: Record<string, string> = {
    Accept: "application/vnd.github+json",
    "X-GitHub-Api-Version": "2022-11-28",
    "User-Agent": "agentic-pdf-website",
  };
  const token = process.env.GITHUB_TOKEN;
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }
  const response = await fetch(`${API_ROOT}${path}`, {
    headers,
    signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
  });
  if (!response.ok) {
    throw new Error(
      `GitHub request ${path} failed with status ${response.status}`,
    );
  }
  return response.json();
}

export async function getReleases(): Promise<Release[]> {
  "use cache";
  cacheLife("hours");
  cacheTag("releases");
  const releases: z.infer<typeof releaseSchema>[] = [];
  for (let page = 1; page <= MAX_RELEASE_PAGES; page += 1) {
    const batch = z
      .array(releaseSchema)
      .parse(
        await requestJson(
          `/releases?per_page=${RELEASES_PER_PAGE}&page=${page}`,
        ),
      );
    releases.push(...batch);
    if (batch.length < RELEASES_PER_PAGE) {
      break;
    }
  }
  return releases
    .filter((release) => !release.draft)
    .map((release) => ({
      tag: release.tag_name,
      url: release.html_url,
      publishedAt: release.published_at,
      prerelease: release.prerelease,
      assets: release.assets.map((asset) => ({
        name: asset.name,
        sizeBytes: asset.size,
        url: asset.browser_download_url,
      })),
      changes: parseReleaseNotes(release.body ?? ""),
    }));
}

export async function getLatestRelease(): Promise<Release> {
  const releases = await getReleases();
  const latest = releases.find((release) => !release.prerelease);
  if (!latest) {
    throw new Error("GitHub returned no published stable release");
  }
  return latest;
}

export async function getUnreleased(sinceTag: string): Promise<Unreleased> {
  "use cache";
  cacheLife("hours");
  cacheTag("releases");
  const comparison = compareSchema.parse(
    await requestJson(`/compare/${encodeURIComponent(sinceTag)}...main`),
  );
  return {
    compareUrl: comparison.html_url,
    changes: comparison.commits
      .map((entry) => parseMergeCommit(entry.commit.message))
      .filter((change): change is Change => change !== null)
      .reverse(),
  };
}
