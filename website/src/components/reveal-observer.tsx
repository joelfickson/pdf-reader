"use client";

import { useEffect } from "react";

const REVEAL_THRESHOLD = 0.12;
const PENDING_SELECTOR = ".reveal:not(.in)";

export function RevealObserver() {
  useEffect(() => {
    if (!("IntersectionObserver" in window)) {
      for (const element of document.querySelectorAll(PENDING_SELECTOR)) {
        element.classList.add("in");
      }
      return;
    }
    const visibility = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            entry.target.classList.add("in");
            visibility.unobserve(entry.target);
          }
        }
      },
      { threshold: REVEAL_THRESHOLD },
    );
    const observePending = () => {
      for (const element of document.querySelectorAll(PENDING_SELECTOR)) {
        visibility.observe(element);
      }
    };
    const additions = new MutationObserver(observePending);
    additions.observe(document.body, { childList: true, subtree: true });
    observePending();
    return () => {
      additions.disconnect();
      visibility.disconnect();
    };
  }, []);
  return null;
}
