import { timingSafeEqual } from "node:crypto";

export function isAuthorized(header: string | null, secret: string): boolean {
  if (!header?.startsWith("Bearer ")) {
    return false;
  }
  const given = Buffer.from(header.slice("Bearer ".length));
  const expected = Buffer.from(secret);
  return given.length === expected.length && timingSafeEqual(given, expected);
}
