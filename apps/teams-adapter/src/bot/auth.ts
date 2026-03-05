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
