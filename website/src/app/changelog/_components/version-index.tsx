"use client";

import { useEffect, useRef, useState } from "react";

export type VersionIndexItem = {
  id: string;
  label: string;
  date: string;
};

type VersionIndexProps = {
  items: VersionIndexItem[];
};

const ACTIVE_MARGIN = "-30% 0px -60% 0px";
const REDUCED_QUERY = "(prefers-reduced-motion: reduce)";

export function VersionIndex({ items }: VersionIndexProps) {
  const [active, setActive] = useState(items[0]?.id ?? "");
  const nav = useRef<HTMLElement>(null);

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

  useEffect(() => {
    const container = nav.current;
    if (!container || container.scrollWidth <= container.clientWidth) {
      return;
    }
    const pill = container.querySelector<HTMLElement>(
      `a[data-version="${CSS.escape(active)}"]`,
    );
    if (!pill) {
      return;
    }
    const reduced = window.matchMedia(REDUCED_QUERY).matches;
    container.scrollTo({
      left: pill.offsetLeft - (container.clientWidth - pill.offsetWidth) / 2,
      behavior: reduced ? "auto" : "smooth",
    });
  }, [active]);

  return (
    <nav className="index" aria-label="Versions" ref={nav}>
      <div className="eyebrow">Versions</div>
      {items.map((item) => (
        <a
          key={item.id}
          href={`#${item.id}`}
          data-version={item.id}
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
