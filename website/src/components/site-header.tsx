import Link from "next/link";
import { BrandMark } from "./brand-mark";
import { MobileMenu } from "./mobile-menu";
import { NavLinks } from "./nav-links";

export function SiteHeader() {
  return (
    <header className="nav">
      <div className="bar">
        <div className="inner">
          <Link className="brand" href="/">
            <BrandMark />
            Agentic PDF
          </Link>
          <NavLinks />
          <Link className="btn small primary" href="/download">
            Download
          </Link>
          <MobileMenu />
        </div>
      </div>
    </header>
  );
}
