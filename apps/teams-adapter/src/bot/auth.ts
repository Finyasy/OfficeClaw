import type { AuthEnvelope } from "../types.js";

export function buildSignInCard(signInUrl: string): Record<string, unknown> {
  return {
    type: "AdaptiveCard",
    version: "1.5",
    body: [
      {
        type: "TextBlock",
        text: "Sign in to continue",
        weight: "Bolder",
      },
    ],
    actions: [
      {
        type: "Action.OpenUrl",
        title: "Sign in",
        url: signInUrl,
      },
    ],
  };
}

export function parseOAuthCallback(query: URLSearchParams): { code: string; state: string } {
  const code = query.get("code");
  const state = query.get("state");

  if (!code || !state) {
    throw new Error("Invalid OAuth callback payload");
  }

  return { code, state };
}

export function normalizeOAuthTokenPayload(payload: unknown): AuthEnvelope {
  if (!payload || typeof payload !== "object") {
    throw new Error("OAuth callback payload must be an object");
  }

  const value = payload as Record<string, unknown>;
  const tenantId = value.tenant_id;
  const userId = value.user_id;
  const accessToken = value.access_token;

  if (typeof tenantId !== "string" || tenantId.length === 0) {
    throw new Error("OAuth callback payload requires tenant_id");
  }

  if (typeof userId !== "string" || userId.length === 0) {
    throw new Error("OAuth callback payload requires user_id");
  }

  if (typeof accessToken !== "string" || accessToken.length === 0) {
    throw new Error("OAuth callback payload requires access_token");
  }

  return {
    actor: {
      tenant_id: tenantId,
      user_id: userId,
      user_display_name: typeof value.user_display_name === "string" ? value.user_display_name : undefined,
    },
    provider: "graph",
    access_token: accessToken,
    refresh_token: typeof value.refresh_token === "string" ? value.refresh_token : undefined,
    expires_at_utc: typeof value.expires_at_utc === "string" ? value.expires_at_utc : undefined,
    scope: typeof value.scope === "string" ? value.scope : undefined,
  };
}
