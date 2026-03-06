import * as grpc from "@grpc/grpc-js";
import * as protoLoader from "@grpc/proto-loader";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";

import type { ActivityEnvelope, AgentResponse, AuthEnvelope } from "../types.js";

const protoPathCandidate = [
  fileURLToPath(new URL("../../../../proto/agent.proto", import.meta.url)),
  fileURLToPath(new URL("../../../../../proto/agent.proto", import.meta.url)),
].find((candidate) => existsSync(candidate));

if (!protoPathCandidate) {
  throw new Error("Could not locate proto/agent.proto for gRPC transport");
}

const PROTO_PATH = protoPathCandidate;

interface AgentGatewayClient {
  handleActivity(activity: ActivityEnvelope): Promise<AgentResponse>;
  oauthCallback(auth: AuthEnvelope): Promise<{ ok: boolean; message: string }>;
  close(): void;
}

type GrpcUnaryCallback<T> = (error: grpc.ServiceError | null, response: T) => void;

interface GrpcAgentGatewayClient extends grpc.Client {
  handleActivity(request: Record<string, unknown>, callback: GrpcUnaryCallback<Record<string, unknown>>): grpc.ClientUnaryCall;
  oAuthCallback(request: Record<string, unknown>, callback: GrpcUnaryCallback<Record<string, unknown>>): grpc.ClientUnaryCall;
  sendProactive(request: Record<string, unknown>, callback: GrpcUnaryCallback<Record<string, unknown>>): grpc.ClientUnaryCall;
}

interface LoadedProto {
  teamsagent: {
    v1: {
      AgentGateway: grpc.ServiceClientConstructor;
    };
  };
}

function shouldRetry(error: grpc.ServiceError): boolean {
  return error.code === grpc.status.UNAVAILABLE || error.code === grpc.status.DEADLINE_EXCEEDED;
}

function isServiceError(error: unknown): error is grpc.ServiceError {
  return typeof error === "object" && error !== null && "code" in error;
}

async function withRetry<T>(operation: () => Promise<T>): Promise<T> {
  let lastError: Error | undefined;

  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      return await operation();
    } catch (error) {
      lastError = error instanceof Error ? error : new Error("Unknown gRPC failure");
      if (!isServiceError(error) || !shouldRetry(error) || attempt === 2) {
        throw lastError;
      }

      await new Promise((resolve) => setTimeout(resolve, 100 * (attempt + 1)));
    }
  }

  throw lastError ?? new Error("Unknown gRPC failure");
}

function loadClientCtor(): grpc.ServiceClientConstructor {
  const packageDefinition = protoLoader.loadSync(PROTO_PATH, {
    keepCase: false,
    longs: String,
    enums: String,
    defaults: true,
    oneofs: true,
  });

  const loaded = grpc.loadPackageDefinition(packageDefinition) as unknown as LoadedProto;
  return loaded.teamsagent.v1.AgentGateway;
}

function toGrpcRequest(activity: ActivityEnvelope): Record<string, unknown> {
  return {
    actor: {
      tenantId: activity.actor.tenant_id,
      userId: activity.actor.user_id,
      userDisplayName: activity.actor.user_display_name ?? "",
    },
    conversation: {
      channel: activity.conversation.channel,
      conversationId: activity.conversation.conversation_id,
      threadId: activity.conversation.thread_id ?? "",
      messageId: activity.conversation.message_id ?? "",
    },
    text: activity.text,
    attachments: activity.attachments.map((attachment) => ({
      kind: attachment.kind,
      id: attachment.id,
      dataJson: attachment.data_json,
    })),
    action: activity.action ?? "",
    actionPayloadJson: activity.action_payload_json ?? "",
    recipients: activity.recipients,
    containsSensitive: activity.contains_sensitive,
    requestHourLocal: activity.request_hour_local,
    attendeeKnown: activity.attendee_known,
    conversationRefJson: activity.conversation_ref_json ?? "",
    attendeeEmail: activity.attendee_email ?? "",
  };
}

function toGrpcAuthRequest(auth: AuthEnvelope): Record<string, unknown> {
  return {
    actor: {
      tenantId: auth.actor.tenant_id,
      userId: auth.actor.user_id,
      userDisplayName: auth.actor.user_display_name ?? "",
    },
    provider: auth.provider,
    accessToken: auth.access_token,
    refreshToken: auth.refresh_token ?? "",
    expiresAtUtc: auth.expires_at_utc ?? "",
    scope: auth.scope ?? "",
  };
}

function fromGrpcResponse(response: Record<string, unknown>): AgentResponse {
  const actionsRaw = Array.isArray(response.actions) ? response.actions : [];

  return {
    text: typeof response.text === "string" ? response.text : "",
    adaptive_card_json: typeof response.adaptiveCardJson === "string" ? response.adaptiveCardJson : undefined,
    actions: actionsRaw
      .filter((item): item is Record<string, unknown> => typeof item === "object" && item !== null)
      .map((action) => ({
        id: typeof action.id === "string" ? action.id : "",
        label: typeof action.label === "string" ? action.label : "",
        payload_json: typeof action.payloadJson === "string" ? action.payloadJson : "",
        style: typeof action.style === "string" ? action.style : undefined,
      })),
    correlation_id: typeof response.correlationId === "string" ? response.correlationId : "",
  };
}

export class GrpcAgentGatewayTransport implements AgentGatewayClient {
  private readonly client: GrpcAgentGatewayClient;

  constructor(endpoint: string) {
    const ClientCtor = loadClientCtor();
    this.client = new ClientCtor(endpoint, grpc.credentials.createInsecure()) as unknown as GrpcAgentGatewayClient;
  }

  handleActivity(activity: ActivityEnvelope): Promise<AgentResponse> {
    const request = toGrpcRequest(activity);

    return withRetry(
      () =>
        new Promise<AgentResponse>((resolve, reject) => {
          this.client.handleActivity(request, (error, response) => {
            if (error) {
              reject(error);
              return;
            }

            resolve(fromGrpcResponse(response));
          });
        }),
    ).catch((error) => {
      throw new Error(
        `gRPC handleActivity failed: ${error instanceof Error ? error.message : "unknown error"}`,
      );
    });
  }

  oauthCallback(auth: AuthEnvelope): Promise<{ ok: boolean; message: string }> {
    const request = toGrpcAuthRequest(auth);

    return withRetry(
      () =>
        new Promise<{ ok: boolean; message: string }>((resolve, reject) => {
          this.client.oAuthCallback(request, (error, response) => {
            if (error) {
              reject(error);
              return;
            }

            resolve({
              ok: response.ok === true,
              message: typeof response.message === "string" ? response.message : "",
            });
          });
        }),
    ).catch((error) => {
      throw new Error(
        `gRPC oauthCallback failed: ${error instanceof Error ? error.message : "unknown error"}`,
      );
    });
  }

  close(): void {
    this.client.close();
  }
}

export { toGrpcAuthRequest, toGrpcRequest, fromGrpcResponse };
