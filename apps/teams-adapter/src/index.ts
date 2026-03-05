import { createServer } from "node:http";
import { loadConfig } from "./config.js";
import { normalizeActivity } from "./bot/bot.js";
import { GrpcAgentGatewayTransport } from "./transport/agent_grpc.js";

const config = loadConfig();
const client = new GrpcAgentGatewayTransport(config.agentGrpcEndpoint);

const server = createServer(async (req, res) => {
  if (req.method === "GET" && req.url === "/healthz") {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify({ ok: true }));
    return;
  }

  if (req.method === "POST" && req.url === "/api/messages") {
    let body = "";

    req.on("data", (chunk) => {
      body += chunk;
    });

    req.on("end", async () => {
      try {
        const payload = JSON.parse(body);
        const envelope = normalizeActivity(payload);
        const response = await client.handleActivity(envelope);
        res.writeHead(200, { "content-type": "application/json" });
        res.end(JSON.stringify(response));
      } catch (error) {
        res.writeHead(400, { "content-type": "application/json" });
        res.end(
          JSON.stringify({
            ok: false,
            error: error instanceof Error ? error.message : "Invalid request",
          }),
        );
      }
    });
    return;
  }

  res.writeHead(404, { "content-type": "application/json" });
  res.end(JSON.stringify({ ok: false, error: "Not found" }));
});

server.listen(config.port, () => {
  // eslint-disable-next-line no-console
  console.log(`teams-adapter listening on ${config.port}`);
});

process.on("SIGINT", () => {
  client.close();
  process.exit(0);
});
