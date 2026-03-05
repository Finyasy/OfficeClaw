import { describe, expect, it } from "vitest";
import { loadConfig } from "../src/config.js";

describe("loadConfig", () => {
  it("loads valid env config", () => {
    process.env.PORT = "3978";
    process.env.BOT_APP_ID = "bot-id";
    process.env.BOT_APP_PASSWORD = "bot-secret";
    process.env.AGENT_GRPC_ENDPOINT = "agent-core:50051";

    const config = loadConfig();

    expect(config.port).toBe(3978);
    expect(config.botAppId).toBe("bot-id");
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
});
