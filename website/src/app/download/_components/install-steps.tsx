"use client";

import {
  type KeyboardEvent,
  type ReactNode,
  useId,
  useRef,
  useState,
} from "react";
import { useDetectedPlatform } from "./use-detected-platform";

type InstallSystem = "macos" | "windows";

const SYSTEMS: InstallSystem[] = ["macos", "windows"];

const LABELS: Record<InstallSystem, string> = {
  macos: "macOS",
  windows: "Windows",
};

type InstallStepsProps = {
  macos: ReactNode;
  windows: ReactNode;
};

export function InstallSteps({ macos, windows }: InstallStepsProps) {
  const detected = useDetectedPlatform();
  const [chosen, setChosen] = useState<InstallSystem | null>(null);
  const selected: InstallSystem =
    chosen ?? (detected === "windows-x64" ? "windows" : "macos");
  const baseId = useId();
  const tabs = useRef<Record<InstallSystem, HTMLButtonElement | null>>({
    macos: null,
    windows: null,
  });
  const panels: Record<InstallSystem, ReactNode> = { macos, windows };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (
      event.key !== "ArrowLeft" &&
      event.key !== "ArrowRight" &&
      event.key !== "Home" &&
      event.key !== "End"
    ) {
      return;
    }
    event.preventDefault();
    const index = SYSTEMS.indexOf(selected);
    const next =
      event.key === "Home"
        ? SYSTEMS[0]
        : event.key === "End"
          ? SYSTEMS[SYSTEMS.length - 1]
          : SYSTEMS[
              (index + (event.key === "ArrowRight" ? 1 : SYSTEMS.length - 1)) %
                SYSTEMS.length
            ];
    if (next) {
      setChosen(next);
      tabs.current[next]?.focus();
    }
  };

  return (
    <>
      <div
        className="install-tabs"
        role="tablist"
        aria-label="Operating system"
        onKeyDown={onKeyDown}
      >
        {SYSTEMS.map((system) => (
          <button
            key={system}
            ref={(element) => {
              tabs.current[system] = element;
            }}
            type="button"
            role="tab"
            id={`${baseId}-${system}-tab`}
            aria-selected={selected === system}
            aria-controls={`${baseId}-${system}-panel`}
            tabIndex={selected === system ? 0 : -1}
            onClick={() => setChosen(system)}
          >
            {LABELS[system]}
          </button>
        ))}
      </div>
      <div className="steps">
        {SYSTEMS.map((system) => (
          <div
            key={system}
            role="tabpanel"
            id={`${baseId}-${system}-panel`}
            aria-labelledby={`${baseId}-${system}-tab`}
            className={selected === system ? "panel active" : "panel"}
          >
            {panels[system]}
          </div>
        ))}
      </div>
    </>
  );
}
