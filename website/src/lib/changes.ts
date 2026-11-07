export type ChangeKind = "new" | "fixed" | "internal";

export type Change = {
  kind: ChangeKind;
  text: string;
  pullRequest: number | null;
};

const PREFIX =
  /^(feat|fix|ci|chore|docs|build|refactor|perf|test|style)(\([^)]*\))?!?\s*[:/]\s*/i;

const KIND_BY_PREFIX: Record<string, ChangeKind> = {
  feat: "new",
  fix: "fixed",
  perf: "fixed",
};

function capitalize(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

export function describeChange(
  title: string,
  pullRequest: number | null,
): Change {
  const trimmed = title.trim();
  const match = PREFIX.exec(trimmed);
  const prefix = match?.[1]?.toLowerCase();
  const rest = match ? trimmed.slice(match[0].length) : trimmed;
  return {
    kind: (prefix && KIND_BY_PREFIX[prefix]) || "internal",
    text: capitalize(rest.trim()),
    pullRequest,
  };
}

const NOTE_LINE = /^\s*[*-]\s+(.+?)\s+by\s+@[\w-]+\s+in\s+\S*\/pull\/(\d+)\s*$/;

export function parseReleaseNotes(body: string): Change[] {
  return body
    .split(/\r?\n/)
    .map((line) => NOTE_LINE.exec(line))
    .filter((match): match is RegExpExecArray => match !== null)
    .map((match) => describeChange(match[1] ?? "", Number(match[2])));
}

const MERGE_SUBJECT = /^Merge pull request #(\d+) from \S+/;

export function parseMergeCommit(message: string): Change | null {
  const [subject = "", ...rest] = message.split(/\r?\n/);
  const match = MERGE_SUBJECT.exec(subject);
  if (!match) {
    return null;
  }
  const title = rest.find((line) => line.trim().length > 0);
  if (!title) {
    return null;
  }
  return describeChange(title, Number(match[1]));
}
