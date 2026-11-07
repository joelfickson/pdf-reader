"use client";

import { useSyncExternalStore } from "react";
import type { Platform } from "@/lib/site";

function detectPlatform(): Platform | null {
  const agent = navigator.userAgent;
  if (/Windows/i.test(agent)) {
    return "windows-x64";
  }
  if (/Mac/i.test(agent)) {
    return "macos-arm64";
  }
  return null;
}

function subscribe(): () => void {
  return () => {};
}

function serverPlatform(): Platform | null {
  return null;
}

export function useDetectedPlatform(): Platform | null {
  return useSyncExternalStore(subscribe, detectPlatform, serverPlatform);
}
