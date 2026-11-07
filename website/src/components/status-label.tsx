import type { ReactNode } from "react";

export type FeatureStatus = "now" | "next" | "soon";

const LABELS: Record<FeatureStatus, string> = {
  now: "Available",
  next: "Next release",
  soon: "In development",
};

type StatusLabelProps = {
  status: FeatureStatus;
  children?: ReactNode;
};

export function StatusLabel({ status, children }: StatusLabelProps) {
  return (
    <span className={`status ${status}`}>
      <i />
      {children ?? LABELS[status]}
    </span>
  );
}
