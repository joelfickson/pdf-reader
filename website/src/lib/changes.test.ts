import { describe, expect, it } from "vitest";
import { describeChange, parseMergeCommit, parseReleaseNotes } from "./changes";

describe("describeChange", () => {
  it("maps conventional prefixes to kinds and strips them", () => {
    expect(describeChange("feat: search within a document", 20)).toEqual({
      kind: "new",
      text: "Search within a document",
      pullRequest: 20,
    });
    expect(
      describeChange("fix(release): validate stapled tickets", null).kind,
    ).toBe("fixed");
    expect(describeChange("ci: update actions to Node.js 24", 15).kind).toBe(
      "internal",
    );
  });

  it("handles breaking changes, performance and scoped internal work", () => {
    expect(describeChange("feat!: drop old option", 30)).toEqual({
      kind: "new",
      text: "Drop old option",
      pullRequest: 30,
    });
    expect(describeChange("perf: faster search", null)).toEqual({
      kind: "fixed",
      text: "Faster search",
      pullRequest: null,
    });
    expect(
      describeChange("docs(readme): document the command line", 26),
    ).toEqual({
      kind: "internal",
      text: "Document the command line",
      pullRequest: 26,
    });
  });

  it("accepts slash-style branch titles and plain titles", () => {
    expect(describeChange("Feat/UI overhaul", 13)).toEqual({
      kind: "new",
      text: "UI overhaul",
      pullRequest: 13,
    });
    expect(describeChange("Bump version", null)).toEqual({
      kind: "internal",
      text: "Bump version",
      pullRequest: null,
    });
  });
});

describe("parseReleaseNotes", () => {
  it("reads generated What's Changed lines and ignores everything else", () => {
    const body = [
      "## Install",
      "macOS: download the zip.",
      "## What's Changed",
      "* feat: add open-book application icon by @jelome265 in https://github.com/joelfickson/pdf-reader/pull/1",
      "* Feat/UI overhaul by @joelfickson in https://github.com/joelfickson/pdf-reader/pull/13",
      "* @jelome265 made their first contribution in https://github.com/joelfickson/pdf-reader/pull/1",
    ].join("\n");
    expect(parseReleaseNotes(body)).toEqual([
      { kind: "new", text: "Add open-book application icon", pullRequest: 1 },
      { kind: "new", text: "UI overhaul", pullRequest: 13 },
    ]);
  });

  it("tolerates extra spaces around the bullet, prefix and author", () => {
    expect(
      parseReleaseNotes(
        "  *   fix:   keep scroll position   by   @joelfickson   in   https://github.com/joelfickson/pdf-reader/pull/9   ",
      ),
    ).toEqual([
      { kind: "fixed", text: "Keep scroll position", pullRequest: 9 },
    ]);
  });
});

describe("parseMergeCommit", () => {
  it("takes the PR number from the subject and the title from the body", () => {
    expect(
      parseMergeCommit(
        "Merge pull request #20 from joelfickson/feat/document-search\n\nfeat: search within a document",
      ),
    ).toEqual({
      kind: "new",
      text: "Search within a document",
      pullRequest: 20,
    });
  });

  it("ignores ordinary commits and merges without a title", () => {
    expect(
      parseMergeCommit("fix: update macOS arm64 release runner"),
    ).toBeNull();
    expect(
      parseMergeCommit("Merge pull request #5 from someone/branch"),
    ).toBeNull();
  });
});
