"use client";

import { useEffect, useRef } from "react";

const WIDE_QUERY = "(min-width: 981px)";
const REDUCED_QUERY = "(prefers-reduced-motion: reduce)";
const SLIDE_DISTANCE_PX = 260;

export function AgentStage() {
  const stage = useRef<HTMLDivElement>(null);
  const light = useRef<HTMLElement>(null);

  useEffect(() => {
    const wide = window.matchMedia(WIDE_QUERY);
    const reduced = window.matchMedia(REDUCED_QUERY);
    let frame = 0;
    const update = () => {
      frame = 0;
      const stageElement = stage.current;
      const lightElement = light.current;
      if (!stageElement || !lightElement) {
        return;
      }
      if (reduced.matches || !wide.matches) {
        lightElement.style.removeProperty("--slide");
        return;
      }
      const viewport = window.innerHeight;
      const progress = Math.min(
        1,
        Math.max(
          0,
          (viewport - stageElement.getBoundingClientRect().top) /
            (viewport * 0.9),
        ),
      );
      lightElement.style.setProperty(
        "--slide",
        `${((1 - progress) * SLIDE_DISTANCE_PX).toFixed(1)}px`,
      );
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
  }, []);

  return (
    <div className="agent-stage" ref={stage} aria-hidden="true">
      <div className="page">
        <h4>Field Notes, Volume 2</h4>
        <div className="sub">Chapter 4, Coastal Birds</div>
        <p>
          The tern colonies along the northern spit returned in late April, two
          weeks earlier than the previous season.
        </p>
        <p>
          Counts on the eastern dunes fell by a third after the storm surge,
          while the sheltered western flats held steady.
        </p>
        <p>
          Observers should record nest density at dawn, when the colony is least
          disturbed.
        </p>
      </div>
      <aside className="light" ref={light}>
        <div className="head">
          <b>Codex</b>
          <span>
            <i />
            Ready
          </span>
        </div>
        <div className="q">What changed on the eastern dunes?</div>
        <div className="ans show">
          <p>
            Counts there fell by about a third after the storm surge.{" "}
            <span className="cite">p. 4</span>
          </p>
          <p>
            The western flats held steady over the same period.{" "}
            <span className="cite">p. 4</span>
          </p>
        </div>
      </aside>
    </div>
  );
}
