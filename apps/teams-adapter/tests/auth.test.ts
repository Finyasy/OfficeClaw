import { describe, expect, it } from "vitest";

import { normalizeOAuthTokenPayload, parseOAuthCallback } from "../src/bot/auth.js";

describe("parseOAuthCallback", () => {
  it("parses valid oauth query params", () => {
    const result = parseOAuthCallback(new URLSearchParams("code=abc&state=xyz"));
    expect(result).toEqual({ code: "abc", state: "xyz" });
  });

  it("throws on invalid oauth callback payload", () => {
    expect(() => parseOAuthCallback(new URLSearchParams("code=abc"))).toThrow(/Invalid OAuth callback payload/);
  });
});

describe("normalizeOAuthTokenPayload", () => {
  it("normalizes a valid token capture payload", () => {
    const payload = normalizeOAuthTokenPayload({
      tenant_id: "tenant-1",
      user_id: "user-1",
      access_token: "access",
      refresh_token: "refresh",
      expires_at_utc: "2026-03-06T12:00:00Z",
      scope: "Mail.Read Calendars.Read",
    });

    expect(payload.provider).toBe("graph");
    expect(payload.actor.user_id).toBe("user-1");
    expect(payload.access_token).toBe("access");
  });

  it("rejects payloads without required identifiers", () => {
    expect(() => normalizeOAuthTokenPayload({ user_id: "user-1", access_token: "access" })).toThrow(/tenant_id/);
    expect(() => normalizeOAuthTokenPayload({ tenant_id: "tenant-1", access_token: "access" })).toThrow(/user_id/);
    expect(() => normalizeOAuthTokenPayload({ tenant_id: "tenant-1", user_id: "user-1" })).toThrow(/access_token/);
  });
});
