import { createServer, type Server as HttpServer } from "node:http";
import { once } from "node:events";
import net from "node:net";
import { spawn, type ChildProcess } from "node:child_process";
import { fileURLToPath } from "node:url";

import { afterAll, beforeAll, describe, expect, it } from "vitest";

import {
  BotFrameworkClientCredentialsTokenProvider,
  BotFrameworkProactiveMessageSender,
} from "../src/bot/proactive.js";
import { createAppServerWithOptions } from "../src/server.js";
import { InMemoryProactiveDeliveryStore } from "../src/storage/proactive_deliveries.js";
import { GrpcAgentGatewayTransport } from "../src/transport/agent_grpc.js";

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

async function waitForPort(port: number, timeoutMs: number): Promise<void> {
  const started = Date.now();
  while (Date.now() - started < timeoutMs) {
    const connected = await new Promise<boolean>((resolve) => {
      const socket = net.createConnection({ host: "127.0.0.1", port }, () => {
        socket.end();
        resolve(true);
      });
      socket.on("error", () => resolve(false));
    });

    if (connected) {
      return;
    }

    await new Promise((resolve) => setTimeout(resolve, 200));
  }

  throw new Error(`Port ${port} did not open within ${timeoutMs}ms`);
}

describe("local e2e", () => {
  let graphServer: HttpServer;
  let connectorServer: HttpServer;
  let adapterServer: HttpServer;
  let transport: GrpcAgentGatewayTransport;
  let rustProcess: ChildProcess;
  let proactiveDeliveryStore: InMemoryProactiveDeliveryStore;
  let adapterPort = 0;
  let grpcPort = 0;
  let graphPort = 0;
  let connectorPort = 0;
  let tokenRequests = 0;
  const proactiveActivities: Array<Record<string, unknown>> = [];

  beforeAll(async () => {
    graphPort = await getFreePort();
    grpcPort = await getFreePort();
    adapterPort = await getFreePort();
    connectorPort = await getFreePort();

    graphServer = createServer((req, res) => {
      if (req.method === "GET" && req.url?.startsWith("/v1.0/me/mailFolders/inbox/messages")) {
        res.writeHead(200, { "content-type": "application/json" });
        res.end(
          JSON.stringify({
            value: [
              {
                id: "mail-1",
                subject: "Budget review",
                receivedDateTime: "2026-03-06T08:00:00Z",
                from: {
                  emailAddress: {
                    name: "James",
                    address: "james@contoso.com",
                  },
                },
              },
            ],
          }),
        );
        return;
      }

      res.writeHead(404, { "content-type": "application/json" });
      res.end(JSON.stringify({ ok: false }));
    });
    graphServer.listen(graphPort, "127.0.0.1");
    await once(graphServer, "listening");

    connectorServer = createServer(async (req, res) => {
      if (req.method === "POST" && req.url === "/oauth2/v2.0/token") {
        tokenRequests += 1;
        res.writeHead(200, { "content-type": "application/json" });
        res.end(JSON.stringify({ access_token: "bot-token", expires_in: 3600 }));
        return;
      }

      if (req.method === "POST" && req.url === "/v3/conversations/conv-1/activities") {
        let body = "";
        req.on("data", (chunk) => {
          body += String(chunk);
        });
        req.on("end", () => {
          proactiveActivities.push(JSON.parse(body) as Record<string, unknown>);
          res.writeHead(200, { "content-type": "application/json" });
          res.end(JSON.stringify({ id: "activity-1" }));
        });
        return;
      }

      res.writeHead(404, { "content-type": "application/json" });
      res.end(JSON.stringify({ ok: false }));
    });
    connectorServer.listen(connectorPort, "127.0.0.1");
    await once(connectorServer, "listening");

    const agentCoreDir = fileURLToPath(new URL("../../agent-core", import.meta.url));
    rustProcess = spawn("cargo", ["run", "--quiet", "--bin", "agent-core"], {
      cwd: agentCoreDir,
      env: {
        ...process.env,
        GRPC_PORT: String(grpcPort),
        GRAPH_BASE_URL: `http://127.0.0.1:${graphPort}/v1.0`,
        TEAMS_ADAPTER_BASE_URL: `http://127.0.0.1:${adapterPort}`,
      },
      stdio: ["ignore", "pipe", "pipe"],
    });

    const ready = new Promise<void>((resolve, reject) => {
      rustProcess.stdout?.on("data", (chunk) => {
        if (String(chunk).includes("agent-core gRPC listening on")) {
          resolve();
        }
      });
      rustProcess.once("exit", (code) => {
        reject(new Error(`agent-core exited before becoming ready (code=${code ?? "null"})`));
      });
    });

    rustProcess.stderr?.on("data", () => {
      // Keep stderr drained so the child does not block during test runs.
    });

    await Promise.race([ready, waitForPort(grpcPort, 60_000)]);

    transport = new GrpcAgentGatewayTransport(`127.0.0.1:${grpcPort}`);
    proactiveDeliveryStore = new InMemoryProactiveDeliveryStore();
    adapterServer = createAppServerWithOptions(transport, {
      proactiveDeliveryStore,
      proactiveSender: new BotFrameworkProactiveMessageSender(
        new BotFrameworkClientCredentialsTokenProvider({
          appId: "bot-id",
          appPassword: "bot-secret",
          tokenEndpoint: `http://127.0.0.1:${connectorPort}/oauth2/v2.0/token`,
          tenantId: "tenant-1",
        }),
      ),
    });
    adapterServer.listen(adapterPort, "127.0.0.1");
    await once(adapterServer, "listening");
  }, 90_000);

  afterAll(async () => {
    transport?.close();

    if (adapterServer?.listening) {
      await new Promise<void>((resolve, reject) => {
        adapterServer.close((error) => {
          if (error) {
            reject(error);
            return;
          }
          resolve();
        });
      });
    }

    if (graphServer?.listening) {
      await new Promise<void>((resolve, reject) => {
        graphServer.close((error) => {
          if (error) {
            reject(error);
            return;
          }
          resolve();
        });
      });
    }

    if (connectorServer?.listening) {
      await new Promise<void>((resolve, reject) => {
        connectorServer.close((error) => {
          if (error) {
            reject(error);
            return;
          }
          resolve();
        });
      });
    }

    if (rustProcess && !rustProcess.killed) {
      rustProcess.kill("SIGINT");
      await once(rustProcess, "exit");
    }
  });

  it(
    "routes oauth callback and unread summary through adapter, gRPC, and rust core",
    async () => {
      const authResponse = await fetch(`http://127.0.0.1:${adapterPort}/oauth/callback`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          tenant_id: "tenant-1",
          user_id: "user-1",
          access_token: "access-token",
          refresh_token: "refresh-token",
          expires_at_utc: "2026-03-06T12:00:00Z",
          scope: "Mail.Read Calendars.Read",
        }),
      });

      expect(authResponse.status).toBe(200);

      const response = await fetch(`http://127.0.0.1:${adapterPort}/api/messages`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          channelId: "teams",
          id: "msg-1",
          serviceUrl: "https://smba.trafficmanager.net/teams/",
          text: "summarize unread emails from today",
          from: { aadObjectId: "user-1", name: "Bryan" },
          recipient: { id: "bot-1", name: "OfficeClaw" },
          conversation: { id: "conv-1" },
          channelData: { tenant: { id: "tenant-1" } },
        }),
      });

      expect(response.status).toBe(200);
      const payload = (await response.json()) as { text: string; actions: Array<{ id: string }> };

      expect(payload.text).toContain("1 unread emails");
      expect(payload.text).toContain("James: Budget review");
      expect(payload.actions.map((action) => action.id)).toContain("DRAFT_REPLIES");
    },
    90_000,
  );

  it(
    "stores conversation references in rust and delivers proactive notifications through the adapter",
    async () => {
      tokenRequests = 0;
      proactiveActivities.length = 0;
      const seedResponse = await fetch(`http://127.0.0.1:${adapterPort}/api/messages`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          channelId: "teams",
          id: "msg-2",
          serviceUrl: `http://127.0.0.1:${connectorPort}`,
          text: "hello",
          from: { aadObjectId: "user-1", name: "Bryan" },
          recipient: { id: "bot-1", name: "OfficeClaw" },
          conversation: { id: "conv-1" },
          channelData: { tenant: { id: "tenant-1" } },
        }),
      });

      expect(seedResponse.status).toBe(200);

      const webhookResponse = await fetch(`http://127.0.0.1:${adapterPort}/api/messages`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          channelId: "teams",
          id: "msg-3",
          serviceUrl: `http://127.0.0.1:${connectorPort}`,
          text: "",
          from: { aadObjectId: "user-1", name: "Bryan" },
          recipient: { id: "bot-1", name: "OfficeClaw" },
          conversation: { id: "conv-1" },
          channelData: { tenant: { id: "tenant-1" } },
          value: {
            action: "WEBHOOK_NOTIFICATION",
          },
        }),
      });

      expect(webhookResponse.status).toBe(200);

      const deliveries = proactiveDeliveryStore.list();
      expect(deliveries).toHaveLength(1);
      expect(deliveries[0]?.text).toContain("Webhook processed");
      expect(deliveries[0]?.conversation_ref_json).toContain("conv-1");
      expect(tokenRequests).toBe(1);
      expect(proactiveActivities).toHaveLength(1);
      expect(proactiveActivities[0]?.text).toContain("Webhook processed");
    },
    90_000,
  );
});
