"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { REPO_URL } from "@/lib/site";

const LINKS = [
  { href: "/agent", label: "Agent" },
  { href: "/changelog", label: "Changelog" },
  { href: "/privacy", label: "Privacy" },
];

export function NavLinks() {
  const pathname = usePathname();
  return (
    <nav aria-label="Main">
      {LINKS.map((link) => (
        <Link
          key={link.href}
          href={link.href}
          aria-current={pathname === link.href ? "page" : undefined}
        >
          {link.label}
        </Link>
      ))}
      <a href={REPO_URL}>GitHub</a>
    </nav>
  );
}
