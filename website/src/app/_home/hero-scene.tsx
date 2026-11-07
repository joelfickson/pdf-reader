"use client";

import { type RefObject, useEffect, useRef, useState } from "react";

type Stage = 0 | 1 | 2 | 3 | 4 | 5 | 6;

const STAGE_TIMES_MS: [Stage, number][] = [
  [1, 600],
  [2, 1400],
  [3, 2700],
  [4, 3900],
  [5, 4700],
  [6, 5700],
];

const STATE_LABELS: Record<Stage, string> = {
  0: "Reading",
  1: "Reading",
  2: "Searching",
  3: "Searching",
  4: "Waiting for you",
  5: "Writing",
  6: "Ready",
};

const WIDE_QUERY = "(min-width: 981px)";
const REDUCED_QUERY = "(prefers-reduced-motion: reduce)";
const TRACE_START = { left: 52, top: 70 };

function clamp(value: number): number {
  return Math.min(1, Math.max(0, value));
}

function traceTarget(
  page: HTMLElement,
  mark: HTMLElement,
): { left: number; top: number } {
  const pageBox = page.getBoundingClientRect();
  const rects = mark.getClientRects();
  const last = rects[rects.length - 1] ?? mark.getBoundingClientRect();
  const scaleX = page.offsetWidth / pageBox.width || 1;
  const scaleY = page.offsetHeight / pageBox.height || 1;
  return {
    left: (last.right - pageBox.left) * scaleX + 8,
    top: (last.top - pageBox.top) * scaleY + (last.height * scaleY) / 2 - 7,
  };
}

function useScrollMotion(
  scene: RefObject<HTMLElement | null>,
  page: RefObject<HTMLDivElement | null>,
  light: RefObject<HTMLElement | null>,
) {
  useEffect(() => {
    const wide = window.matchMedia(WIDE_QUERY);
    const reduced = window.matchMedia(REDUCED_QUERY);
    let frame = 0;
    const update = () => {
      frame = 0;
      const sceneElement = scene.current;
      const pageElement = page.current;
      const lightElement = light.current;
      if (!sceneElement || !pageElement || !lightElement) {
        return;
      }
      if (reduced.matches || !wide.matches) {
        lightElement.style.removeProperty("--slide");
        pageElement.style.removeProperty("transform");
        return;
      }
      const viewport = window.innerHeight;
      const box = sceneElement.getBoundingClientRect();
      const progress = clamp((viewport - box.top) / (viewport + box.height));
      lightElement.style.setProperty(
        "--slide",
        `${((0.5 - progress) * 120).toFixed(1)}px`,
      );
      pageElement.style.transform = `rotateX(${(8 - progress * 8).toFixed(2)}deg) rotateY(${(-10 + progress * 6).toFixed(2)}deg) rotateZ(-1.5deg)`;
    };
    const schedule = () => {
      if (!frame) {
        frame = window.requestAnimationFrame(update);
      }
    };
    window.addEventListener("scroll", schedule, { passive: true });
    window.addEventListener("resize", schedule);
    update();
    return () => {
      window.removeEventListener("scroll", schedule);
      window.removeEventListener("resize", schedule);
      window.cancelAnimationFrame(frame);
    };
  }, [scene, page, light]);
}

export function HeroScene() {
  const scene = useRef<HTMLElement>(null);
  const page = useRef<HTMLDivElement>(null);
  const light = useRef<HTMLElement>(null);
  const firstMark = useRef<HTMLElement>(null);
  const secondMark = useRef<HTMLElement>(null);
  const [stage, setStage] = useState<Stage>(0);
  const [trace, setTrace] = useState(TRACE_START);

  useScrollMotion(scene, page, light);

  useEffect(() => {
    if (window.matchMedia(REDUCED_QUERY).matches) {
      setStage(6);
      return;
    }
    const timers = STAGE_TIMES_MS.map(([next, delay]) =>
      window.setTimeout(() => setStage(next), delay),
    );
    return () => {
      for (const timer of timers) {
        window.clearTimeout(timer);
      }
    };
  }, []);

  useEffect(() => {
    const pageElement = page.current;
    const target =
      stage >= 3 ? secondMark.current : stage >= 2 ? firstMark.current : null;
    if (pageElement && target) {
      setTrace(traceTarget(pageElement, target));
    }
  }, [stage]);

  const marked = stage >= 5 ? "on" : undefined;
  return (
    <figure
      className="scene"
      ref={scene}
      aria-label="Preview of the reader with the agent panel"
    >
      <div className="pane lens" aria-hidden="true">
        <span className="caught" />
      </div>
      <div className="page" ref={page}>
        <span
          className="trace"
          aria-hidden="true"
          style={{
            left: trace.left,
            top: trace.top,
            opacity: stage >= 6 ? 0 : 1,
          }}
        />
        <h4>Office Lease Agreement</h4>
        <div className="sub">Harbor Point Holdings and Lumen Studio Ltd</div>
        <p>
          <b>12. Term and Termination</b>
        </p>
        <p>
          12.1 Either party may terminate this Lease on the third anniversary of
          the Commencement Date by giving the other party{" "}
          <mark ref={firstMark} className={marked}>
            not less than six months' written notice
          </mark>
          .
        </p>
        <p>
          12.2 The Landlord may terminate by notice if the Rent is unpaid for{" "}
          <mark ref={secondMark} className={marked}>
            twenty-one days after becoming due
          </mark>
          , or if the Tenant commits a material breach that is not remedied
          within thirty days.
        </p>
        <p>
          <b>13. Rent Review</b>
        </p>
        <p>
          The Rent is reviewed on each anniversary in line with the consumer
          price index, subject to a cap of four per cent per year.
        </p>
      </div>
      <div className="pane float" aria-hidden="true" />
      <aside className="light" ref={light}>
        <div className="head">
          <b>Claude Code</b>
          <span>
            <i />
            {STATE_LABELS[stage]}
          </span>
        </div>
        <div className="q">
          Summarize the exit terms and highlight the notice periods.
        </div>
        <div className={stage >= 1 ? "step show" : "step"}>
          <b>read</b>Outline, 9 pages
        </div>
        <div className={stage >= 2 ? "step show" : "step"}>
          <b>find</b>"terminat" on page 4, 2 matches
        </div>
        <div className={stage >= 5 ? "step show" : "step"}>
          <b>mark</b>2 highlights, approved by you
        </div>
        <div className={stage >= 6 ? "ans show" : "ans"}>
          <p>
            Either side can leave at year three with six months' written notice.{" "}
            <span className="cite">p. 4, 12.1</span>
          </p>
          <p>
            The landlord can end it if rent is 21 days late.{" "}
            <span className="cite">p. 4, 12.2</span>
          </p>
        </div>
        <span className="tag">Agent panel preview, in development</span>
      </aside>
    </figure>
  );
}
