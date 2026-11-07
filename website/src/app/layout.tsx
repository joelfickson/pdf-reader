import type { Metadata } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import { LightField } from "@/components/light-field";
import { RevealObserver } from "@/components/reveal-observer";
import { SiteFooter } from "@/components/site-footer";
import { SiteHeader } from "@/components/site-header";
import "./globals.css";

const geist = Geist({ subsets: ["latin"], variable: "--font-geist" });
const geistMono = Geist_Mono({
  subsets: ["latin"],
  variable: "--font-geist-mono",
});

export const metadata: Metadata = {
  title: {
    default: "Agentic PDF, the PDF reader that reads with you",
    template: "%s | Agentic PDF",
  },
  description:
    "A fast, native PDF reader for macOS and Windows built in Rust. Next, the AI agents you already use will read alongside you.",
};

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    <html lang="en" className={`${geist.variable} ${geistMono.variable}`}>
      <body>
        <a className="skip" href="#main">
          Skip to content
        </a>
        <LightField />
        <div className="content">
          <SiteHeader />
          <main id="main">{children}</main>
          <SiteFooter />
        </div>
        <RevealObserver />
      </body>
    </html>
  );
}
