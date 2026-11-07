import type { Metadata } from "next";
import { StatusLabel } from "@/components/status-label";
import { AGENT_ISSUE_URL, ISSUES_URL, REPO_URL } from "@/lib/site";
import "./privacy.css";

export const metadata: Metadata = {
  title: "Privacy",
  description:
    "How Agentic PDF handles your data: no account, no tracking, no uploads, and a short local list of recent files you can clear.",
};

const PROMISES = [
  [
    "account.",
    "There is nothing to sign up for or sign in to. You download the app and open a file.",
  ],
  [
    "tracking.",
    "The reader's code contains no telemetry, no analytics and no network requests.",
  ],
  [
    "uploads.",
    "Documents are opened and rendered on your computer by the PDFium engine. They are never sent anywhere.",
  ],
];

const RECENT_FACTS = [
  [
    "Saved",
    "The file's location on your computer, the page you were on, and your zoom level",
  ],
  ["Limit", "The 20 most recent files. Older entries are dropped."],
  [
    "Not saved",
    "The document's contents, its text, your searches or your selections",
  ],
  ["When", "Written when you open or close a file and when you quit the app"],
];

export default function PrivacyPage() {
  return (
    <div className="privacy-page">
      <div className="wrap">
        <div className="intro">
          <div className="eyebrow">Privacy</div>
          <h1>
            Your documents stay <em>yours.</em>
          </h1>
          <p className="lede">
            Agentic PDF reads your files on your own computer. This page
            explains, in plain language, what the app does with your data, what
            it keeps, and how to remove it.
          </p>
          <div className="updated">Last updated 7 November 2026</div>
          <div className="promises">
            {PROMISES.map(([word, body]) => (
              <div key={word}>
                <strong>
                  No <em>{word}</em>
                </strong>
                <p>{body}</p>
              </div>
            ))}
          </div>
        </div>

        <section className="topic">
          <div>
            <div className="eyebrow">The reader</div>
            <h2>
              What happens when you <em>open a PDF</em>
            </h2>
          </div>
          <div className="body">
            <p>
              The app reads the file from your disk, renders its pages and
              extracts its text on your computer. Page images and text are kept
              in memory while you read and are released when you close the tab.
            </p>
            <p>
              When you copy text, the selected text is placed on your system
              clipboard, as with any app. Nothing else leaves the reader.
            </p>
            <p>Version 0.1.2 writes no files of its own to your computer.</p>
          </div>
        </section>

        <section className="topic">
          <div>
            <div className="eyebrow">Stored on your computer</div>
            <h2>
              A short list of <em>recent files</em>
            </h2>
            <StatusLabel status="next" />
          </div>
          <div className="body">
            <p>
              Starting with the next release, the app remembers the PDFs you
              opened so it can list them under Recent and reopen them where you
              left off. It keeps only this:
            </p>
            <ul>
              {RECENT_FACTS.map(([label, fact]) => (
                <li key={label}>
                  <span>{label}</span>
                  {fact}
                </li>
              ))}
              <li>
                <span>macOS</span>
                <code>
                  ~/Library/Application Support/org.pdfreader.desktop/recent.tsv
                </code>
              </li>
              <li>
                <span>Windows</span>
                <code>%APPDATA%\PDF Reader\recent.tsv</code>
              </li>
            </ul>
            <p>The list never leaves your computer.</p>
          </div>
        </section>

        <section className="topic">
          <div>
            <div className="eyebrow">Your control</div>
            <h2>
              How to <em>delete it</em>
            </h2>
          </div>
          <div className="body">
            <p>
              <b>Clear the list:</b> choose File, then Open Recent, then Clear
              Menu.
            </p>
            <p>
              <b>Remove everything:</b> quit the app and delete the{" "}
              <code>recent.tsv</code> file shown above. Deleting the app removes
              the program itself; the recent list is a separate file, so delete
              it too if you want no trace.
            </p>
            <p>
              If a file in the list was moved or deleted, choosing it removes it
              from the list.
            </p>
          </div>
        </section>

        <section className="topic">
          <div>
            <div className="eyebrow">The agent panel</div>
            <h2>
              You choose who <em>reads along</em>
            </h2>
            <StatusLabel status="soon" />
          </div>
          <div className="body">
            <p>
              The agent panel is being built in the open, tracked in{" "}
              <a href={AGENT_ISSUE_URL}>issue #18</a>. It is not in any release
              yet. This is how it is designed to work:
            </p>
            <p>
              <b>Only the agent you pick.</b> The panel connects to an agent
              already installed on your computer, such as Claude Code, Codex or
              Gemini CLI. Parts of your document are shared only with that
              agent, through that agent's own connection, under its provider's
              terms.
            </p>
            <p>
              <b>Reading is not writing.</b> The agent can read the outline,
              search and read text through the reader's tools. Adding highlights
              or notes to your document always waits for your approval.
            </p>
            <p>
              <b>Nothing by default.</b> If you never open the panel, no agent
              is started and nothing is shared.
            </p>
          </div>
        </section>

        <section className="topic">
          <div>
            <div className="eyebrow">This website</div>
            <h2>
              No <em>cookies</em> here
            </h2>
          </div>
          <div className="body">
            <p>This site uses no analytics or cookies.</p>
            <p>
              Its fonts are served from this site itself, so your browser does
              not contact a font service when a page loads. Downloads are hosted
              on GitHub Releases, so GitHub's privacy statement applies when you
              download.
            </p>
          </div>
        </section>

        <section className="topic">
          <div>
            <div className="eyebrow">Questions</div>
            <h2>
              Something <em>looks wrong?</em>
            </h2>
          </div>
          <div className="body">
            <p>
              Agentic PDF is open source, so every claim on this page can be
              checked in the <a href={REPO_URL}>source code</a>.
            </p>
            <p>
              To report a privacy concern, open an issue on{" "}
              <a href={ISSUES_URL}>GitHub</a>. Issues are public, so please
              leave out personal details and document contents.
            </p>
          </div>
        </section>

        <p className="plain">
          This is a plain-language summary of how the app handles data, not a
          legal policy. When the app's behavior changes, this page changes with
          it.
        </p>
      </div>
    </div>
  );
}
