import {
  BotFrameworkClientCredentialsTokenProvider,
  BotFrameworkProactiveMessageSender,
} from "./bot/proactive.js";
import { loadConfig } from "./config.js";
import { createAppServerWithOptions } from "./server.js";
import { GrpcAgentGatewayTransport } from "./transport/agent_grpc.js";

const config = loadConfig();
const client = new GrpcAgentGatewayTransport(config.agentGrpcEndpoint);
const proactiveSender = new BotFrameworkProactiveMessageSender(
  new BotFrameworkClientCredentialsTokenProvider({
    appId: config.botAppId,
    appPassword: config.botAppPassword,
    tenantId: config.botTenantId,
    tokenEndpoint: config.botTokenEndpoint,
    scope: config.botTokenScope,
  }),
);
const server = createAppServerWithOptions(client, { proactiveSender });

server.listen(config.port, () => {
  console.log(`teams-adapter listening on ${config.port}`);
});

process.on("SIGINT", () => {
  client.close();
  server.close(() => process.exit(0));
});
