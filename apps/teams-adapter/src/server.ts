import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http";

import { normalizeOAuthTokenPayload } from "./bot/auth.js";
import { normalizeActivity } from "./bot/bot.js";
import { NoopProactiveMessageSender, type ProactiveMessageSender } from "./bot/proactive.js";
import type { ActivityEnvelope, AgentResponse, AuthEnvelope, ProactiveDeliveryRequest } from "./types.js";
import { InMemoryProactiveDeliveryStore } from "./storage/proactive_deliveries.js";

export interface AgentGatewayTransport {
  handleActivity(activity: ActivityEnvelope): Promise<AgentResponse>;
  oauthCallback(auth: AuthEnvelope): Promise<{ ok: boolean; message: string }>;
  close(): void;
}

export interface AppServerOptions {
  proactiveDeliveryStore?: InMemoryProactiveDeliveryStore;
  proactiveSender?: ProactiveMessageSender;
}

async function readJson(req: IncomingMessage): Promise<unknown> {
  let body = "";

  await new Promise<void>((resolve, reject) => {
    req.on("data", (chunk) => {
      body += chunk;
    });
    req.on("end", () => resolve());
    req.on("error", reject);
  });

  if (!body) {
    return {};
  }

  return JSON.parse(body);
}

function json(res: ServerResponse, statusCode: number, payload: unknown): void {
  res.writeHead(statusCode, { "content-type": "application/json" });
  res.end(JSON.stringify(payload));
}

export function createAppServer(client: AgentGatewayTransport): Server {
  return createAppServerWithOptions(client, {});
}

export function createAppServerWithOptions(client: AgentGatewayTransport, options: AppServerOptions): Server {
  const proactiveSender = options.proactiveSender ?? new NoopProactiveMessageSender();
  return createServer(async (req, res) => {
    if (req.method === "GET" && req.url === "/healthz") {
      json(res, 200, { ok: true });
      return;
    }

    if (req.method === "POST" && req.url === "/api/messages") {
      try {
        const payload = await readJson(req);
        const envelope = normalizeActivity(payload as Record<string, unknown>);
        const response = await client.handleActivity(envelope);
        json(res, 200, response);
      } catch (error) {
        json(res, 400, {
          ok: false,
          error: error instanceof Error ? error.message : "Invalid request",
        });
      }
      return;
    }

    if (req.method === "POST" && req.url === "/oauth/callback") {
      try {
        const payload = await readJson(req);
        const authEnvelope = normalizeOAuthTokenPayload(payload);
        const response = await client.oauthCallback(authEnvelope);
        json(res, 200, response);
      } catch (error) {
        json(res, 400, {
          ok: false,
          error: error instanceof Error ? error.message : "Invalid OAuth callback request",
        });
      }
      return;
    }

    if (req.method === "POST" && req.url === "/api/proactive") {
      let payload: ProactiveDeliveryRequest;
      try {
        payload = (await readJson(req)) as ProactiveDeliveryRequest;
        if (!payload.text || !payload.conversation_ref_json) {
          throw new Error("Proactive delivery request requires text and conversation_ref_json");
        }
      } catch (error) {
        json(res, 400, {
          ok: false,
          error: error instanceof Error ? error.message : "Invalid proactive delivery request",
        });
        return;
      }

      try {
        options.proactiveDeliveryStore?.add(payload);
        await proactiveSender.send(payload);
        json(res, 202, { ok: true, message: "proactive delivery accepted" });
      } catch (error) {
        json(res, 502, {
          ok: false,
          error: error instanceof Error ? error.message : "Proactive delivery failed",
        });
      }
      return;
    }

    json(res, 404, { ok: false, error: "Not found" });
  });
}
