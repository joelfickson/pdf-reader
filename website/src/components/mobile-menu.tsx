"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useId, useRef, useState } from "react";
import { latestDownloadUrl, PLATFORM_ASSETS, REPO_URL } from "@/lib/site";
import { BrandMark } from "./brand-mark";

type MenuLink = {
  href: string;
  label: string;
  note?: string;
};

const LINKS: MenuLink[] = [
  { href: "/", label: "Home" },
  { href: "/agent", label: "Agent", note: "In development" },
  { href: "/changelog", label: "Changelog" },
  { href: "/privacy", label: "Privacy" },
];

export function MobileMenu() {
  const [open, setOpen] = useState(false);
  const pathname = usePathname();
  const sheetId = useId();
  const toggle = useRef<HTMLButtonElement>(null);
  const firstLink = useRef<HTMLAnchorElement>(null);
  const [openedAt, setOpenedAt] = useState(pathname);

  if (open && openedAt !== pathname) {
    setOpen(false);
  }

  useEffect(() => {
    if (!open) {
      return;
    }
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    firstLink.current?.focus();
    const button = toggle.current;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      document.body.style.overflow = previousOverflow;
      window.removeEventListener("keydown", onKey);
      button?.focus();
    };
  }, [open]);

  return (
    <>
      <button
        ref={toggle}
        type="button"
        className="menu-toggle"
        aria-expanded={open}
        aria-controls={sheetId}
        aria-label="Open menu"
        onClick={() => {
          setOpenedAt(pathname);
          setOpen(true);
        }}
      >
        <svg viewBox="0 0 20 20" aria-hidden="true">
          <path d="M3 6h14M3 10h14M3 14h14" />
        </svg>
      </button>
      {open && (
        <div
          className="menu-sheet"
          id={sheetId}
          role="dialog"
          aria-modal="true"
          aria-label="Menu"
        >
          <div className="menu-bar">
            <Link className="brand" href="/" onClick={() => setOpen(false)}>
              <BrandMark />
              Agentic PDF
            </Link>
            <button
              type="button"
              className="menu-toggle"
              aria-label="Close menu"
              onClick={() => setOpen(false)}
            >
              <svg viewBox="0 0 20 20" aria-hidden="true">
                <path d="M5 5l10 10M15 5L5 15" />
              </svg>
            </button>
          </div>
          <nav aria-label="Menu">
            {LINKS.map((link, index) => (
              <Link
                key={link.href}
                ref={index === 0 ? firstLink : undefined}
                href={link.href}
                aria-current={pathname === link.href ? "page" : undefined}
                onClick={() => setOpen(false)}
              >
                {link.label}
                {link.note && <small>{link.note}</small>}
              </Link>
            ))}
            <a href={REPO_URL}>
              GitHub
              <small>Source</small>
            </a>
          </nav>
          <div className="menu-cta">
            <a
              className="btn primary"
              href={latestDownloadUrl(PLATFORM_ASSETS["macos-arm64"])}
            >
              Download for Mac
            </a>
            <Link
              className="btn glass"
              href="/download"
              onClick={() => setOpen(false)}
            >
              All downloads
            </Link>
          </div>
        </div>
      )}
    </>
  );
}
