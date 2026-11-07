import Link from "next/link";
import { DownloadButton } from "@/components/download-button";
import { type FeatureStatus, StatusLabel } from "@/components/status-label";
import { getLatestRelease } from "@/lib/github";
import {
  AGENT_ISSUE_URL,
  latestDownloadUrl,
  PLATFORM_ASSETS,
  REPO_URL,
} from "@/lib/site";
import { AgentStage } from "./_home/agent-stage";
import { HeroScene } from "./_home/hero-scene";
import { PhoneDemo } from "./_home/phone-demo";
import "./_home/home.css";

type Feature = {
  title: string;
  body: string;
  status: FeatureStatus;
};

const FEATURES: Feature[] = [
  {
    title: "Tabs that keep your place",
    body: "Open many PDFs side by side. Each tab remembers its page, zoom and scroll position.",
    status: "now",
  },
  {
    title: "Select and copy",
    body: "Drag across real text, including rotated pages. Copies keep Unicode and line breaks intact.",
    status: "now",
  },
  {
    title: "Keyboard first",
    body: "Command palette, go to page, zoom and tab switching without leaving the keyboard.",
    status: "now",
  },
  {
    title: "Search the whole document",
    body: "Matches across line breaks and accents, with every result listed by page and highlighted in place.",
    status: "next",
  },
  {
    title: "Open from Finder",
    body: "Open With and double-click open straight into a tab, even when the app is already running.",
    status: "next",
  },
  {
    title: "Pick up where you left off",
    body: "Recent files reopen on the page and zoom you left them at.",
    status: "next",
  },
];

const BENEFITS = [
  [
    "Private by default",
    "Your PDF stays on your computer. Nothing leaves unless the agent you chose sends it.",
  ],
  [
    "No lock-in",
    "Switch between agents as they improve. The reader never ties you to one model or vendor.",
  ],
  [
    "Answers you can check",
    "Page citations jump to the exact passage, so you verify instead of trusting.",
  ],
  [
    "You approve every edit",
    "Reading is automatic. Writing to your document always waits for your yes.",
  ],
];

const USES = [
  [
    "Summarize this contract and list every deadline.",
    "A short summary with each date cited to its clause and page.",
  ],
  [
    "Where does it say we can cancel?",
    "The exact passages, highlighted on the page after you approve.",
  ],
  [
    "Compare section 4 with the appendix.",
    "Differences explained side by side, each linked to its page.",
  ],
  [
    "Which pages could you not read?",
    "Scanned pages are named, never skipped silently.",
  ],
];

const FAQ = [
  [
    "Does Agentic PDF send my documents anywhere?",
    "No. The reader works entirely on your computer. When the agent panel arrives, your document is only shared with the agent you choose, through that agent's own connection.",
  ],
  [
    "Which agents will work?",
    "Agents that speak the Agent Client Protocol, including Claude Code, Codex and Gemini CLI through their ACP adapters. You use your existing subscription.",
  ],
  [
    "Is it free?",
    "Yes. Agentic PDF is free and open source under the MIT license.",
  ],
  [
    "Can it read scanned PDFs?",
    "Not yet. Pages without a text layer are reported as unreadable. On-device OCR is being considered for the agent release.",
  ],
];

export default async function HomePage() {
  const latest = await getLatestRelease();
  return (
    <div className="home">
      <div className="hero">
        <div className="wrap">
          <h1>
            The PDF reader that reads{" "}
            <span className="keep">
              <em>with you</em>
              <span className="dot" />
            </span>
          </h1>
          <p className="lede">
            A fast, native reader built in Rust.{" "}
            <b>Next, the AI agents you already use will read alongside you:</b>{" "}
            summarizing, finding and citing pages, and highlighting what
            matters, right on the page.
          </p>
          <div className="ctas">
            <DownloadButton />
            <Link className="btn glass" href="#agent">
              See how the agent works
            </Link>
          </div>
          <div className="meta">
            <span>
              <i />
              Free and open source, MIT
            </span>
            <span>{latest.tag}</span>
            <span>Apple silicon, Intel and Windows</span>
          </div>
          <PhoneDemo />
        </div>
        <HeroScene />
      </div>

      <section id="reader">
        <div className="wrap">
          <div className="sec-head reveal">
            <div className="eyebrow">The reader</div>
            <h2>
              Fast, calm, and <em>native</em> on your desktop.
            </h2>
            <p>
              No browser engine, no account, no upload. Agentic PDF is written
              in Rust on the GPU-accelerated GPUI framework, with Google's
              PDFium engine doing the rendering.
            </p>
          </div>
          <div className="list reveal">
            {FEATURES.map((feature) => (
              <div className="row" key={feature.title}>
                <h3>{feature.title}</h3>
                <p>{feature.body}</p>
                <StatusLabel status={feature.status} />
              </div>
            ))}
          </div>
        </div>
      </section>

      <section id="agent">
        <div className="wrap agent-wrap">
          <div className="sec-head reveal">
            <div className="eyebrow">The agent</div>
            <h2>
              Bring the agent you already use. It <em>reads, cites</em> and asks
              first.
            </h2>
            <p>
              Agentic PDF connects to coding and research agents installed on
              your computer through the open Agent Client Protocol. No API keys
              to paste and no new subscription. The reader gives the agent
              precise tools for your document, and keeps you in control of every
              change.
            </p>
            <p className="issue-link">
              <a className="status soon" href={AGENT_ISSUE_URL}>
                <i />
                In development, follow issue #18
              </a>
            </p>
          </div>
          <AgentStage />
          <div className="flow reveal">
            <div>
              <span className="n">01</span>
              <h3>Connect your agent</h3>
              <p>
                The reader finds the agents installed on your computer and
                starts one in a panel beside your document.
              </p>
              <div className="agents">
                <span>Claude Code</span>
                <span>Codex</span>
                <span>Gemini CLI</span>
              </div>
            </div>
            <div>
              <span className="n">02</span>
              <h3>It reads with real tools</h3>
              <p>
                Outline, search, and text with page positions, served by the
                reader over MCP in small, bounded pieces. Long documents stay
                fast and memory stays flat.
              </p>
            </div>
            <div>
              <span className="n">03</span>
              <h3>It answers with pages, and asks before acting</h3>
              <p>
                Every claim links to the page it came from. Highlights and notes
                are only added after you approve them.
              </p>
            </div>
          </div>
          <div className="benefits reveal">
            {BENEFITS.map(([title, body]) => (
              <div key={title}>
                <h4>{title}</h4>
                <p>{body}</p>
              </div>
            ))}
          </div>
          <div className="uses reveal">
            <div className="eyebrow uses-label">What you will ask</div>
            {USES.map(([question, answer]) => (
              <div className="use" key={question}>
                <q>{question}</q>
                <p>{answer}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      <section id="speed">
        <div className="wrap">
          <div className="sec-head reveal">
            <div className="eyebrow">Measured, not promised</div>
            <h2>
              Fast enough that you <em>stop noticing</em> it.
            </h2>
            <p>
              We publish what we measure, with the machine and method. Search
              runs in the background in short slices, so turning pages never
              waits for it.
            </p>
          </div>
          <div className="numbers reveal">
            <div className="num">
              <strong>
                0.5<small>s</small>
              </strong>
              <p>
                to search all 457 pages of a technical book, from 425 to 620 ms
                across three runs.
              </p>
            </div>
            <div className="num">
              <strong>
                40<small>ms</small>
              </strong>
              <p>
                until the first matches appear while the rest of the book is
                still being searched.
              </p>
            </div>
            <div className="num">
              <strong>
                18<small>ms</small>
              </strong>
              <p>
                at most to render a new page while a full search is running.
              </p>
            </div>
          </div>
          <ul className="numfacts">
            <li>
              <b>0.5 s</b> to search all 457 pages of a technical book, from 425
              to 620 ms across three runs.
            </li>
            <li>
              <b>40 ms</b> until the first matches appear while the rest of the
              book is still being searched.
            </li>
            <li>
              <b>18 ms</b> at most to render a new page while a full search is
              running.
            </li>
          </ul>
          <p className="footnote">
            Measured on an Apple M1 Max with 32 GB of memory, release build,
            searching Rust in Action (457 pages, 18 MB) through the reader's
            worker, three runs. Your results will vary with the document and
            machine.
          </p>
        </div>
      </section>

      <section id="open-source">
        <div className="wrap stack">
          <div className="reveal">
            <div className="eyebrow">Open source</div>
            <h2 className="stack-title">
              Read the code that <em>reads your files.</em>
            </h2>
            <p className="stack-lede">
              MIT licensed and built in the open. Every feature lands through a
              public pull request with tests.
            </p>
            <a className="btn glass" href={REPO_URL}>
              Star on GitHub
            </a>
          </div>
          <dl className="reveal">
            <dt>Language</dt>
            <dd>Rust</dd>
            <dt>Interface</dt>
            <dd>GPUI, GPU-accelerated and native</dd>
            <dt>Rendering</dt>
            <dd>PDFium</dd>
            <dt>Agents</dt>
            <dd>Agent Client Protocol and MCP</dd>
            <dt>Platforms</dt>
            <dd>macOS on Apple silicon and Intel, Windows x64</dd>
            <dt>License</dt>
            <dd>MIT</dd>
          </dl>
        </div>
      </section>

      <section className="download" id="download">
        <div className="wrap">
          <div className="eyebrow reveal">Download</div>
          <h2 className="reveal">
            Start reading. The agent is <em>on its way.</em>
          </h2>
          <div className="dl-rows reveal">
            <div className="dl">
              <span className="os">macOS</span>
              <span className="note">Apple silicon (M1 and later)</span>
              <a
                className="btn small primary"
                href={latestDownloadUrl(PLATFORM_ASSETS["macos-arm64"])}
              >
                Download .zip
              </a>
            </div>
            <div className="dl">
              <span className="os">macOS</span>
              <span className="note">Intel</span>
              <a
                className="btn small glass"
                href={latestDownloadUrl(PLATFORM_ASSETS["macos-x64"])}
              >
                Download .zip
              </a>
            </div>
            <div className="dl">
              <span className="os">Windows</span>
              <span className="note">x64</span>
              <a
                className="btn small glass"
                href={latestDownloadUrl(PLATFORM_ASSETS["windows-x64"])}
              >
                Download .zip
              </a>
            </div>
          </div>
          <p className="honest">
            Current builds are not yet signed for every platform. On first
            launch, macOS may ask you to allow the app in Privacy and Security,
            and Windows SmartScreen may ask you to confirm.{" "}
            <Link href="/download">Install steps and checksums</Link>.
          </p>
        </div>
      </section>

      <section className="faq">
        <div className="wrap">
          <div className="eyebrow faq-label">Questions</div>
          {FAQ.map(([question, answer]) => (
            <details key={question}>
              <summary>{question}</summary>
              <p>{answer}</p>
            </details>
          ))}
        </div>
      </section>
    </div>
  );
}
