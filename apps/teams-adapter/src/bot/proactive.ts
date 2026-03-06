import type { ProactiveDeliveryRequest } from "../types.js";

export interface BotFrameworkTokenProvider {
  getToken(): Promise<string>;
}

export interface ProactiveMessageSender {
  send(request: ProactiveDeliveryRequest): Promise<void>;
}

export interface BotFrameworkAuthConfig {
  appId: string;
  appPassword: string;
  tenantId?: string;
  tokenEndpoint?: string;
  scope?: string;
}

interface ResolvedBotFrameworkAuthConfig {
  appId: string;
  appPassword: string;
  tenantId: string;
  tokenEndpoint: string;
  scope: string;
}

interface OAuthTokenResponse {
  access_token: string;
  expires_in: number;
}

interface BotConversationReference {
  serviceUrl: string;
  channelId?: string;
  tenantId?: string;
  conversation?: { id?: string };
  user?: { aadObjectId?: string; id?: string; name?: string };
  bot?: { id?: string; name?: string };
}

interface ConnectorActivity {
  type: "message";
  serviceUrl: string;
  channelId: string;
  conversation: { id: string };
  from: { id: string; name?: string };
  recipient: { id: string; name?: string };
  text: string;
  attachments?: Array<{
    contentType: "application/vnd.microsoft.card.adaptive";
    content: Record<string, unknown>;
  }>;
  channelData?: {
    tenant?: { id: string };
  };
}

export class BotFrameworkClientCredentialsTokenProvider implements BotFrameworkTokenProvider {
  private readonly authConfig: ResolvedBotFrameworkAuthConfig;
  private cachedToken?: { value: string; expiresAtMs: number };

  constructor(authConfig: BotFrameworkAuthConfig) {
    this.authConfig = {
      appId: authConfig.appId,
      appPassword: authConfig.appPassword,
      tenantId: authConfig.tenantId ?? "botframework.com",
      tokenEndpoint:
        authConfig.tokenEndpoint ??
        `https://login.microsoftonline.com/${authConfig.tenantId ?? "botframework.com"}/oauth2/v2.0/token`,
      scope: authConfig.scope ?? "https://api.botframework.com/.default",
    };
  }

  async getToken(): Promise<string> {
    if (this.cachedToken && Date.now() < this.cachedToken.expiresAtMs) {
      return this.cachedToken.value;
    }

    const response = await fetch(this.authConfig.tokenEndpoint, {
      method: "POST",
      headers: {
        "content-type": "application/x-www-form-urlencoded",
      },
      body: new URLSearchParams({
        grant_type: "client_credentials",
        client_id: this.authConfig.appId,
        client_secret: this.authConfig.appPassword,
        scope: this.authConfig.scope,
      }),
    });

    if (!response.ok) {
      throw new Error(`Bot Framework token request failed with status ${response.status}`);
    }

    const payload = (await response.json()) as Partial<OAuthTokenResponse>;
    if (typeof payload.access_token !== "string" || payload.access_token.length === 0) {
      throw new Error("Bot Framework token response missing access_token");
    }

    const expiresIn = typeof payload.expires_in === "number" ? payload.expires_in : 3600;
    this.cachedToken = {
      value: payload.access_token,
      expiresAtMs: Date.now() + Math.max(expiresIn - 60, 30) * 1000,
    };

    return payload.access_token;
  }
}

export class NoopProactiveMessageSender implements ProactiveMessageSender {
  async send(): Promise<void> {}
}

export class BotFrameworkProactiveMessageSender implements ProactiveMessageSender {
  constructor(private readonly tokenProvider: BotFrameworkTokenProvider) {}

  async send(request: ProactiveDeliveryRequest): Promise<void> {
    const reference = parseConversationReference(request.conversation_ref_json);
    const token = await this.tokenProvider.getToken();
    const serviceUrl = reference.serviceUrl.replace(/\/$/, "");
    const conversationId = reference.conversation?.id ?? request.conversation.conversation_id;
    if (!serviceUrl || !conversationId) {
      throw new Error("Conversation reference is missing serviceUrl or conversation id");
    }

    const activity = buildConnectorActivity(reference, request);
    const response = await fetch(`${serviceUrl}/v3/conversations/${encodeURIComponent(conversationId)}/activities`, {
      method: "POST",
      headers: {
        authorization: `Bearer ${token}`,
        "content-type": "application/json",
      },
      body: JSON.stringify(activity),
    });

    if (!response.ok) {
      throw new Error(`Bot Framework proactive send failed with status ${response.status}`);
    }
  }
}

function parseConversationReference(raw: string): BotConversationReference {
  const parsed = JSON.parse(raw) as Partial<BotConversationReference>;
  if (!parsed || typeof parsed !== "object") {
    throw new Error("Conversation reference must be a JSON object");
  }

  if (typeof parsed.serviceUrl !== "string" || parsed.serviceUrl.length === 0) {
    throw new Error("Conversation reference missing serviceUrl");
  }

  return parsed as BotConversationReference;
}

function buildConnectorActivity(
  reference: BotConversationReference,
  request: ProactiveDeliveryRequest,
): ConnectorActivity {
  const adaptiveCard = parseAdaptiveCard(request.adaptive_card_json);
  return {
    type: "message",
    serviceUrl: reference.serviceUrl,
    channelId: reference.channelId ?? request.conversation.channel ?? "teams",
    conversation: {
      id: reference.conversation?.id ?? request.conversation.conversation_id,
    },
    from: {
      id: reference.bot?.id ?? "",
      name: reference.bot?.name,
    },
    recipient: {
      id: reference.user?.id ?? reference.user?.aadObjectId ?? request.actor.user_id,
      name: reference.user?.name,
    },
    text: request.text,
    attachments: adaptiveCard
      ? [
          {
            contentType: "application/vnd.microsoft.card.adaptive",
            content: adaptiveCard,
          },
        ]
      : undefined,
    channelData: reference.tenantId
      ? {
          tenant: {
            id: reference.tenantId,
          },
        }
      : undefined,
  };
}

function parseAdaptiveCard(raw?: string): Record<string, unknown> | undefined {
  if (!raw || raw.trim().length === 0) {
    return undefined;
  }

  const parsed = JSON.parse(raw) as unknown;
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error("Adaptive card payload must be a JSON object");
  }

  return parsed as Record<string, unknown>;
}

export { buildConnectorActivity, parseAdaptiveCard, parseConversationReference };
