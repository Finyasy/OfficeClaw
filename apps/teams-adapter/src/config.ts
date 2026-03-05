export interface AppConfig {
  port: number;
  botAppId: string;
  botAppPassword: string;
  agentGrpcEndpoint: string;
}

function required(name: string): string {
  const value = process.env[name];
  if (!value || !value.trim()) {
    throw new Error(`Missing required env var: ${name}`);
  }
  return value;
}

export function loadConfig(): AppConfig {
  const portRaw = process.env.PORT ?? "3978";
  const port = Number(portRaw);
  if (!Number.isInteger(port) || port <= 0 || port > 65535) {
    throw new Error("PORT must be an integer between 1 and 65535");
  }

  return {
    port,
    botAppId: required("BOT_APP_ID"),
    botAppPassword: required("BOT_APP_PASSWORD"),
    agentGrpcEndpoint: required("AGENT_GRPC_ENDPOINT"),
  };
}
