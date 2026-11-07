"use client";

import { useEffect, useState } from "react";

export type VersionIndexItem = {
  id: string;
  label: string;
  date: string;
};

type VersionIndexProps = {
  items: VersionIndexItem[];
};

const ACTIVE_MARGIN = "-30% 0px -60% 0px";

export function VersionIndex({ items }: VersionIndexProps) {
  const [active, setActive] = useState(items[0]?.id ?? "");

  useEffect(() => {
    if (!("IntersectionObserver" in window)) {
      return;
    }
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            setActive(entry.target.id);
          }
        }
      },
      { rootMargin: ACTIVE_MARGIN },
    );
    for (const item of items) {
      const section = document.getElementById(item.id);
      if (section) {
        observer.observe(section);
      }
    }
    return () => observer.disconnect();
  }, [items]);

  return (
    <nav className="index" aria-label="Versions">
      <div className="eyebrow">Versions</div>
      {items.map((item) => (
        <a
          key={item.id}
          href={`#${item.id}`}
          className={item.id === active ? "active" : undefined}
          aria-current={item.id === active ? "location" : undefined}
        >
          <b>{item.label}</b>
          <span>{item.date}</span>
        </a>
      ))}
    </nav>
  );
}
