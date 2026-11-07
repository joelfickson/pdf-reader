import type { Metadata } from "next";
import Link from "next/link";
import type { ReactNode } from "react";
import { AGENT_ISSUE_URL, REPO_URL } from "@/lib/site";
import "./agent.css";

export const metadata: Metadata = {
  title: "Agent",
  description:
    "How Agentic PDF will connect to the AI agent you already use to find, summarize and cite pages, and ask before it changes anything. In development.",
};

type Message = {
  direction: "r" | "l";
  method: string;
  side: string;
  ask?: boolean;
};

const MESSAGES: Message[] = [
  { direction: "r", method: "initialize", side: "Agree on version 1" },
  {
    direction: "r",
    method: "session/new",
    side: "Open a session with the PDF tools",
  },
  {
    direction: "r",
    method: "session/prompt",
    side: '"Summarize the exit terms"',
  },
  { direction: "l", method: "tool_call: search", side: "Finds the clauses" },
  {
    direction: "l",
    method: "agent_message_chunk",
    side: "Answer with page links",
  },
  {
    direction: "l",
    method: "request_permission",
    side: '"May I highlight 2 passages?"',
    ask: true,
  },
  {
    direction: "r",
    method: "selected: allow_once",
    side: "You chose allow once",
  },
  { direction: "l", method: "end_turn", side: "Done" },
];

type Tool = {
  name: string;
  kind: "Read" | "Navigate" | "Asks first";
  description: ReactNode;
};

const TOOLS: Tool[] = [
  {
    name: "document_info",
    kind: "Read",
    description:
      "Page count, title, and which pages have no text layer, so nothing is skipped silently.",
  },
  {
    name: "get_outline",
    kind: "Read",
    description: "The table of contents, so the agent can plan what to read.",
  },
  {
    name: "get_text",
    kind: "Read",
    description: (
      <>
        Text for a range of pages, capped by <code>max_chars</code> with a{" "}
        <code>next_page</code> cursor to continue.
      </>
    ),
  },
  {
    name: "search",
    kind: "Read",
    description: "Every match with its page and position on the page.",
  },
  {
    name: "get_selection",
    kind: "Read",
    description:
      "Whatever you have selected, so you can ask about exactly that.",
  },
  {
    name: "goto_page",
    kind: "Navigate",
    description: "Takes you to the page being discussed.",
  },
  {
    name: "highlight",
    kind: "Asks first",
    description: "Marks passages on the page, after you approve.",
  },
  {
    name: "add_note",
    kind: "Asks first",
    description: "Adds a note to a page, after you approve.",
  },
];

const SUMMARY_STEPS: [string, string][] = [
  [
    "Outline first",
    "The agent reads the page count, the sections, and which pages have text.",
  ],
  [
    "Then the text, in ranges",
    "It reads a few pages at a time with a cursor, whatever the length of the document.",
  ],
  ["A summary with pages", "Key points, each linked to the page it came from."],
  [
    "Gaps named",
    "Scanned pages without a text layer are listed as not covered, never silently skipped.",
  ],
];

const TRUST: [string, string][] = [
  [
    "Stays on your computer",
    "The reader never uploads your PDF. Text leaves your machine only if the agent you chose sends it to its own service, under that agent's terms.",
  ],
  [
    "No keys, no lock-in",
    "You use the agent and subscription you already have, and can switch agents whenever you like.",
  ],
  [
    "Reading is automatic, writing is not",
    "Highlights and notes always wait for your approval: allow once, always allow, or reject.",
  ],
  [
    "Answers you can check",
    "Every point links to its page, so you can verify it in a click instead of trusting it.",
  ],
];

const OPEN_QUESTIONS: [string, string][] = [
  [
    "Scanned pages",
    "Pages without a text layer cannot be read yet. On-device text recognition, Vision on macOS and the built-in recognizer on Windows, is being considered. Until then those pages are reported as not covered.",
  ],
  [
    "Very long documents",
    "The tools keep every read small, but how well an agent summarizes hundreds of pages depends on that agent. We will measure it with real agents before making claims.",
  ],
  [
    "Which agents, exactly",
    "Claude Code, Codex and Gemini CLI are the first targets. Setup steps differ by agent, and the reader will say clearly when none is installed.",
  ],
];

export default function AgentPage() {
  return (
    <div className="agent-page">
      <div className="ahero">
        <div className="wrap">
          <div className="eyebrow">The agent</div>
          <h1>
            Bring the agent <em>you already use.</em>
          </h1>
          <p className="lede">
            Agentic PDF will connect to the AI agent installed on your computer
            and let it read your document with you.{" "}
            <b>
              It finds, summarizes and cites pages, and it asks before it
              changes anything.
            </b>{" "}
            No API keys, no new subscription, no upload to us.
          </p>
          <div className="statusline">
            <span className="status soon inline">
              <i />
              In development
            </span>
            <span>
              Not in today's download. Follow the work in{" "}
              <a href={AGENT_ISSUE_URL}>issue #18</a>.
            </span>
          </div>
          <div className="ctas">
            <a className="btn primary" href={AGENT_ISSUE_URL}>
              Follow issue #18
            </a>
            <Link className="btn glass" href="#how">
              How it works
            </Link>
          </div>

          <figure className="map reveal" aria-label="How the pieces connect">
            <div className="track">
              <div className="node">
                <b>Agentic PDF</b>
                <span>the reader</span>
              </div>
              <div className="link">
                <em>ACP</em>
                <i aria-hidden="true" />
              </div>
              <div className="node">
                <b>Your agent</b>
                <span>Claude Code, Codex, Gemini CLI</span>
              </div>
              <div className="link">
                <em>MCP</em>
                <i aria-hidden="true" />
              </div>
              <div className="node">
                <b>PDF tools</b>
                <span>served by the reader</span>
              </div>
              <div className="link">
                <em>Local socket</em>
                <i aria-hidden="true" />
              </div>
              <div className="node">
                <b>Your document</b>
                <span>the open tab</span>
              </div>
            </div>
            <div className="caption">
              <p>
                <b>The conversation</b>The reader talks to your agent over the
                open Agent Client Protocol, and shows its plan, steps and answer
                in a panel beside the page.
              </p>
              <p>
                <b>The document</b>The agent reaches your PDF only through tools
                the reader provides over MCP, in small and bounded pieces.
              </p>
              <p>
                <b>Your computer</b>The tools connect back to the open tab over
                a local socket. Nothing passes through a server of ours.
              </p>
            </div>
          </figure>
        </div>
      </div>

      <section id="how">
        <div className="wrap">
          <div className="sec-head reveal">
            <div className="eyebrow">How it works</div>
            <h2>
              From question to <em>cited answer</em>, step by step.
            </h2>
            <p>
              This is the design in issue #18. Details may change as it is built
              and tested against real agents.
            </p>
          </div>
          <div className="steps reveal">
            <div className="st">
              <h3>The reader starts your agent</h3>
              <p>
                It finds agents installed on your computer and starts the one
                you pick as a child process, speaking ACP over standard input
                and output. Claude Code and Codex connect through ACP adapters,
                and Gemini CLI supports ACP directly.
              </p>
            </div>
            <div className="st">
              <h3>It hands over the PDF tools</h3>
              <p>
                When the session opens, <code>session/new</code> gives the agent
                the reader's own MCP server. The agent starts it, and it
                attaches to the exact tab you are reading.
              </p>
            </div>
            <div className="st">
              <h3>You ask, it reads</h3>
              <p>
                Your question goes out with a link to the open file. The agent
                calls the tools it needs, such as the outline, a search, or the
                text of a few pages, and each call appears in the panel as it
                happens.
              </p>
            </div>
            <div className="st">
              <h3>The answer arrives with pages</h3>
              <p>
                The reply streams into the panel. Every page reference is a link
                that jumps to the page and outlines the passage it came from.
              </p>
            </div>
            <div className="st">
              <h3>Changes wait for you</h3>
              <p>
                Before the agent highlights text or adds a note, the reader asks
                you: allow once, always allow, or reject. Reading never changes
                your file.
              </p>
            </div>
          </div>

          <figure
            className="seq reveal"
            aria-label="Illustrative message sequence"
          >
            <div className="label">
              <div className="eyebrow">One session, in messages</div>
              <span>Illustrative, simplified</span>
            </div>
            <div className="ends" aria-hidden="true">
              <span>Reader</span>
              <span />
              <span>Agent</span>
            </div>
            {MESSAGES.map((message) => (
              <div
                className={`msg ${message.direction}${message.ask ? " ask" : ""}`}
                key={message.method}
              >
                <span className="side">
                  {message.direction === "r" ? message.side : ""}
                </span>
                <span className="wire">
                  <b>{message.method}</b>
                </span>
                <span className="side end">
                  {message.direction === "l" ? message.side : ""}
                </span>
              </div>
            ))}
          </figure>
        </div>
      </section>

      <section id="tools">
        <div className="wrap">
          <div className="sec-head reveal">
            <div className="eyebrow">The tools</div>
            <h2>
              Precise tools, <em>not a pasted file.</em>
            </h2>
            <p>
              The agent never receives your whole PDF in one go. It asks for
              exactly what it needs, and long documents are read in bounded
              pieces so the reader stays fast and its memory stays flat.
            </p>
          </div>
          <div className="tools reveal">
            {TOOLS.map((tool) => (
              <div className="t" key={tool.name}>
                <code>{tool.name}</code>
                <p>{tool.description}</p>
                <span
                  className={tool.kind === "Asks first" ? "kind write" : "kind"}
                >
                  {tool.kind}
                </span>
              </div>
            ))}
          </div>
          <p className="note">
            Tool names are proposed and may change. The same tools will also be
            available to agents you run outside the reader.
          </p>
        </div>
      </section>

      <section id="summarize">
        <div className="wrap">
          <div className="sec-head reveal">
            <div className="eyebrow">First feature</div>
            <h2>
              Summarize a document <em>in one click.</em>
            </h2>
            <p>
              The first thing the agent panel will do. One button sends a fixed
              request: summarize, cite pages for every point, and say which
              pages could not be read.
            </p>
          </div>
          <div className="sum">
            <ol className="reveal">
              {SUMMARY_STEPS.map(([title, body]) => (
                <li key={title}>
                  <div>
                    <b>{title}</b>
                    <span>{body}</span>
                  </div>
                </li>
              ))}
            </ol>
            <figure
              className="sumstage reveal"
              aria-label="Illustration of a summary in the agent panel"
            >
              <div className="page">
                <h4>Office Lease Agreement</h4>
                <div className="sub">
                  Harbor Point Holdings and Lumen Studio Ltd
                </div>
                <p>
                  12.1 Either party may terminate this Lease on the third
                  anniversary by giving not less than six months' written
                  notice.
                </p>
                <span className="gap">Page 8: scanned, no text layer</span>
                <p>
                  13. The Rent is reviewed each year in line with the consumer
                  price index, capped at four per cent.
                </p>
              </div>
              <div className="light">
                <div className="head">
                  <b>Your agent</b>
                  <span>
                    <i />
                    Ready
                  </span>
                </div>
                <h5>Summary</h5>
                <ul className="terms">
                  <li>
                    <span>Break clause</span>
                    <span>
                      Year three, six months' notice{" "}
                      <span className="cite">p. 4</span>
                    </span>
                  </li>
                  <li>
                    <span>Rent review</span>
                    <span>
                      Yearly, capped at 4% <span className="cite">p. 4</span>
                    </span>
                  </li>
                </ul>
                <p className="gapnote">
                  Pages 8 and 9 are scanned and were not covered.
                </p>
                <span className="tag">Illustration, in development</span>
              </div>
            </figure>
          </div>
        </div>
      </section>

      <section id="control">
        <div className="wrap">
          <div className="sec-head reveal">
            <div className="eyebrow">Privacy and control</div>
            <h2>
              Your document, <em>your agent, your call.</em>
            </h2>
          </div>
          <div className="trust reveal">
            {TRUST.map(([title, body]) => (
              <div key={title}>
                <h3>{title}</h3>
                <p>{body}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      <section id="open">
        <div className="wrap">
          <div className="sec-head reveal">
            <div className="eyebrow">Still open</div>
            <h2>
              What we have <em>not solved yet.</em>
            </h2>
            <p>Honest limits, tracked in issue #18.</p>
          </div>
          <div className="open reveal">
            {OPEN_QUESTIONS.map(([title, body]) => (
              <div className="o" key={title}>
                <h3>{title}</h3>
                <p>{body}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      <section className="cta" id="follow">
        <div className="wrap">
          <div className="eyebrow reveal">Get involved</div>
          <h2 className="reveal">
            Read today. <em>Bring your agent soon.</em>
          </h2>
          <div className="acts reveal">
            <div className="act">
              <b>Follow the agent work</b>
              <span>Progress, design and open questions in issue #18.</span>
              <a className="btn small primary" href={AGENT_ISSUE_URL}>
                Open issue #18
              </a>
            </div>
            <div className="act">
              <b>Star the project</b>
              <span>Free and open source under the MIT license.</span>
              <a className="btn small glass" href={REPO_URL}>
                Star on GitHub
              </a>
            </div>
            <div className="act">
              <b>Download the reader</b>
              <span>
                The fast, native reader is available now for macOS and Windows.
              </span>
              <Link className="btn small glass" href="/download">
                Download
              </Link>
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}
