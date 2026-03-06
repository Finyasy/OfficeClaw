import { describe, expect, it } from "vitest";
import { loadConfig } from "../src/config.js";

describe("loadConfig", () => {
  it("loads valid env config", () => {
    process.env.PORT = "3978";
    process.env.BOT_APP_ID = "bot-id";
    process.env.BOT_APP_PASSWORD = "bot-secret";
    process.env.BOT_TENANT_ID = "tenant-id";
    process.env.BOT_TOKEN_ENDPOINT = "https://login.microsoftonline.com/tenant-id/oauth2/v2.0/token";
    process.env.BOT_TOKEN_SCOPE = "https://api.botframework.com/.default";
    process.env.AGENT_GRPC_ENDPOINT = "agent-core:50051";

    const config = loadConfig();

    expect(config.port).toBe(3978);
    expect(config.botAppId).toBe("bot-id");
    expect(config.botTenantId).toBe("tenant-id");
  });

  it("throws when required env var is missing", () => {
    process.env.PORT = "3978";
    delete process.env.BOT_APP_ID;
    process.env.BOT_APP_PASSWORD = "bot-secret";
    process.env.AGENT_GRPC_ENDPOINT = "agent-core:50051";

    expect(() => loadConfig()).toThrow(/BOT_APP_ID/);
  });

  it("throws for invalid port", () => {
    process.env.PORT = "70000";
    process.env.BOT_APP_ID = "bot-id";
    process.env.BOT_APP_PASSWORD = "bot-secret";
    process.env.AGENT_GRPC_ENDPOINT = "agent-core:50051";

    expect(() => loadConfig()).toThrow(/PORT/);
  });

  it("defaults optional proactive auth values", () => {
    process.env.PORT = "3978";
    process.env.BOT_APP_ID = "bot-id";
    process.env.BOT_APP_PASSWORD = "bot-secret";
    delete process.env.BOT_TENANT_ID;
    delete process.env.BOT_TOKEN_ENDPOINT;
    delete process.env.BOT_TOKEN_SCOPE;
    process.env.AGENT_GRPC_ENDPOINT = "agent-core:50051";

    const config = loadConfig();

    expect(config.botTenantId).toBe("botframework.com");
    expect(config.botTokenEndpoint).toBeUndefined();
    expect(config.botTokenScope).toBe("https://api.botframework.com/.default");
  });
});
