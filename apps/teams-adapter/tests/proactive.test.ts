import { createServer, type IncomingMessage, type Server } from "node:http";
import { once } from "node:events";
import net from "node:net";

import { afterAll, beforeAll, describe, expect, it } from "vitest";

import {
  BotFrameworkClientCredentialsTokenProvider,
  BotFrameworkProactiveMessageSender,
} from "../src/bot/proactive.js";
import type { ProactiveDeliveryRequest } from "../src/types.js";

async function getFreePort(): Promise<number> {
  return await new Promise<number>((resolve, reject) => {
    const server = net.createServer();
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      if (!address || typeof address === "string") {
        reject(new Error("Could not allocate port"));
        return;
      }

      const port = address.port;
      server.close((error) => {
        if (error) {
          reject(error);
          return;
        }
        resolve(port);
      });
    });
    server.on("error", reject);
  });
}

async function readBody(req: IncomingMessage): Promise<string> {
  let body = "";
  await new Promise<void>((resolve, reject) => {
    req.on("data", (chunk: Buffer | string) => {
      body += String(chunk);
    });
    req.on("end", resolve);
    req.on("error", reject);
  });
  return body;
}

describe("BotFrameworkProactiveMessageSender", () => {
  let server: Server;
  let port = 0;
  let tokenRequests = 0;
  const sentActivities: Array<Record<string, unknown>> = [];

  beforeAll(async () => {
    port = await getFreePort();
    server = createServer(async (req, res) => {
      if (req.method === "POST" && req.url === "/oauth2/v2.0/token") {
        tokenRequests += 1;
        res.writeHead(200, { "content-type": "application/json" });
        res.end(JSON.stringify({ access_token: "bot-token", expires_in: 3600 }));
        return;
      }

      if (req.method === "POST" && req.url === "/v3/conversations/conv-1/activities") {
        const body = await readBody(req);
        sentActivities.push(JSON.parse(body) as Record<string, unknown>);
        expect(req.headers.authorization).toBe("Bearer bot-token");
        res.writeHead(200, { "content-type": "application/json" });
        res.end(JSON.stringify({ id: "activity-1" }));
        return;
      }

      res.writeHead(404, { "content-type": "application/json" });
      res.end(JSON.stringify({ ok: false }));
    });
    server.listen(port, "127.0.0.1");
    await once(server, "listening");
  });

  afterAll(async () => {
    if (server?.listening) {
      await new Promise<void>((resolve, reject) => {
        server.close((error) => {
          if (error) {
            reject(error);
            return;
          }
          resolve();
        });
      });
    }
  });

  function request(overrides?: Partial<ProactiveDeliveryRequest>): ProactiveDeliveryRequest {
    return {
      actor: {
        tenant_id: "tenant-1",
        user_id: "user-1",
      },
      conversation: {
        channel: "teams",
        conversation_id: "conv-1",
      },
      conversation_ref_json: JSON.stringify({
        serviceUrl: `http://127.0.0.1:${port}`,
        channelId: "teams",
        tenantId: "tenant-1",
        conversation: { id: "conv-1" },
        user: { aadObjectId: "user-1", name: "Bryan" },
        bot: { id: "bot-1", name: "OfficeClaw" },
      }),
      text: "Draft ready",
      adaptive_card_json: JSON.stringify({
        type: "AdaptiveCard",
        version: "1.5",
        body: [{ type: "TextBlock", text: "Draft ready" }],
      }),
      correlation_id: "corr-1",
      ...overrides,
    };
  }

  it("requests a token and sends a proactive activity through the connector service", async () => {
    tokenRequests = 0;
    sentActivities.length = 0;

    const sender = new BotFrameworkProactiveMessageSender(
      new BotFrameworkClientCredentialsTokenProvider({
        appId: "bot-id",
        appPassword: "bot-secret",
        tokenEndpoint: `http://127.0.0.1:${port}/oauth2/v2.0/token`,
        tenantId: "tenant-1",
      }),
    );

    await sender.send(request());

    expect(tokenRequests).toBe(1);
    expect(sentActivities).toHaveLength(1);
    expect(sentActivities[0]?.text).toBe("Draft ready");
    expect(sentActivities[0]?.channelData).toEqual({ tenant: { id: "tenant-1" } });
    expect(Array.isArray(sentActivities[0]?.attachments)).toBe(true);
  });

  it("reuses the cached token across repeated sends", async () => {
    tokenRequests = 0;
    sentActivities.length = 0;

    const provider = new BotFrameworkClientCredentialsTokenProvider({
      appId: "bot-id",
      appPassword: "bot-secret",
      tokenEndpoint: `http://127.0.0.1:${port}/oauth2/v2.0/token`,
      tenantId: "tenant-1",
    });
    const sender = new BotFrameworkProactiveMessageSender(provider);

    await sender.send(request());
    await sender.send(request({ correlation_id: "corr-2" }));

    expect(tokenRequests).toBe(1);
    expect(sentActivities).toHaveLength(2);
  });

  it("rejects malformed conversation references", async () => {
    const sender = new BotFrameworkProactiveMessageSender({
      getToken: async () => "bot-token",
    });

    await expect(
      sender.send(
        request({
          conversation_ref_json: JSON.stringify({ conversation: { id: "conv-1" } }),
        }),
      ),
    ).rejects.toThrow(/serviceUrl/);
  });
});
