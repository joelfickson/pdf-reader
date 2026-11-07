import type { Metadata } from "next";
import type { Change, ChangeKind } from "@/lib/changes";
import { formatDate } from "@/lib/format";
import {
  getLatestRelease,
  getReleases,
  getUnreleased,
  type Release,
} from "@/lib/github";
import { RELEASES_URL, REPO_URL } from "@/lib/site";
import {
  VersionIndex,
  type VersionIndexItem,
} from "./_components/version-index";
import "./changelog.css";

export const metadata: Metadata = {
  title: "Changelog",
  description:
    "Every Agentic PDF release and the changes merged since, generated from GitHub.",
};

const UNRELEASED_ID = "unreleased";

const GROUPS: { kind: ChangeKind; label: string }[] = [
  { kind: "new", label: "New" },
  { kind: "fixed", label: "Fixed" },
  { kind: "internal", label: "Under the hood" },
];

const SHORT_DATE = new Intl.DateTimeFormat("en-GB", {
  day: "numeric",
  month: "short",
  timeZone: "UTC",
});

function shortDate(iso: string | null): string {
  return iso ? SHORT_DATE.format(new Date(iso)) : "";
}

function compareUrl(from: string, to: string): string {
  return `${REPO_URL}/compare/${from}...${to}`;
}

type ChangeGroupsProps = {
  changes: Change[];
};

function ChangeGroups({ changes }: ChangeGroupsProps) {
  return (
    <>
      {GROUPS.map((group) => {
        const matching = changes.filter((change) => change.kind === group.kind);
        if (matching.length === 0) {
          return null;
        }
        return (
          <div className="group" key={group.kind}>
            <div className="eyebrow">{group.label}</div>
            {matching.map((change) =>
              change.pullRequest !== null ? (
                <a
                  className="change"
                  key={`${change.pullRequest}-${change.text}`}
                  href={`${REPO_URL}/pull/${change.pullRequest}`}
                >
                  <span className="text">{change.text}</span>
                  <span className="pr">#{change.pullRequest}</span>
                </a>
              ) : (
                <div className="change" key={`none-${change.text}`}>
                  <span className="text">{change.text}</span>
                </div>
              ),
            )}
          </div>
        );
      })}
    </>
  );
}

type ReleaseEntryProps = {
  release: Release;
  latestTag: string;
  previousTag: string | null;
};

function ReleaseEntry({ release, latestTag, previousTag }: ReleaseEntryProps) {
  return (
    <article className="entry reveal" id={release.tag}>
      <div className="entry-head">
        <h2>{release.tag}</h2>
        {release.tag === latestTag ? (
          <span className="tag latest">
            <i />
            Latest
          </span>
        ) : null}
        {release.prerelease ? (
          <span className="tag pre">
            <i />
            Pre-release
          </span>
        ) : null}
        {release.publishedAt ? (
          <time dateTime={release.publishedAt}>
            {formatDate(release.publishedAt)}
          </time>
        ) : null}
      </div>
      {release.changes.length > 0 ? (
        <ChangeGroups changes={release.changes} />
      ) : (
        <a className="release-link" href={release.url}>
          Read the {release.tag} release notes on GitHub
        </a>
      )}
      {previousTag ? (
        <a className="compare" href={compareUrl(previousTag, release.tag)}>
          Compare {previousTag} to {release.tag}
        </a>
      ) : null}
    </article>
  );
}

export default async function ChangelogPage() {
  const [releases, latest] = await Promise.all([
    getReleases(),
    getLatestRelease(),
  ]);
  const unreleased = await getUnreleased(latest.tag);
  const index: VersionIndexItem[] = [
    { id: UNRELEASED_ID, label: "Unreleased", date: "next" },
    ...releases.map((release) => ({
      id: release.tag,
      label: release.tag,
      date: shortDate(release.publishedAt),
    })),
  ];
  return (
    <div className="changelog-page">
      <div className="wrap head">
        <div className="eyebrow">Changelog</div>
        <h1>
          What is <em>new</em>
        </h1>
        <p>
          Every change lands through a public pull request. The full history
          lives on <a href={RELEASES_URL}>GitHub releases</a>.
        </p>
      </div>
      <div className="wrap log">
        <VersionIndex items={index} />
        <div className="entries">
          <article className="entry reveal" id={UNRELEASED_ID}>
            <div className="entry-head">
              <h2>Unreleased</h2>
              <span className="tag unreleased">
                <i />
                On main
              </span>
            </div>
            {unreleased.changes.length > 0 ? (
              <>
                <p className="summary">
                  Merged to main after {latest.tag}. These ship in the next
                  release.
                </p>
                <ChangeGroups changes={unreleased.changes} />
              </>
            ) : (
              <p className="empty">
                Main matches {latest.tag}. Nothing new has merged since the
                latest release.
              </p>
            )}
            <a className="compare" href={unreleased.compareUrl}>
              Compare {latest.tag} to main
            </a>
          </article>
          {releases.map((release, position) => (
            <ReleaseEntry
              key={release.tag}
              release={release}
              latestTag={latest.tag}
              previousTag={releases[position + 1]?.tag ?? null}
            />
          ))}
        </div>
      </div>
    </div>
  );
}
