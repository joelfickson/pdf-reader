import { describe, expect, it } from "vitest";
import { isAuthorized } from "./revalidate";

describe("isAuthorized", () => {
  const secret = "s3cret-value";

  it("accepts the matching bearer token", () => {
    expect(isAuthorized(`Bearer ${secret}`, secret)).toBe(true);
  });

  it("rejects a missing, malformed or different token", () => {
    expect(isAuthorized(null, secret)).toBe(false);
    expect(isAuthorized(secret, secret)).toBe(false);
    expect(isAuthorized("Bearer s3cret-valuX", secret)).toBe(false);
    expect(isAuthorized("Bearer s3cret", secret)).toBe(false);
  });
});
