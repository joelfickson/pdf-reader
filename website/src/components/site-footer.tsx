import Link from "next/link";
import { REPO_URL } from "@/lib/site";

export function SiteFooter() {
  return (
    <footer>
      <div className="wrap">
        <span>Agentic PDF, MIT licensed</span>
        <nav aria-label="Footer">
          <Link href="/download">Download</Link>
          <Link href="/agent">Agent</Link>
          <Link href="/changelog">Changelog</Link>
          <Link href="/privacy">Privacy</Link>
          <a href={REPO_URL}>GitHub</a>
        </nav>
      </div>
    </footer>
  );
}
