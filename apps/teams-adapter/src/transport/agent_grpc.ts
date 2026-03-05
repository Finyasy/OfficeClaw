import * as grpc from "@grpc/grpc-js";
import * as protoLoader from "@grpc/proto-loader";
import { fileURLToPath } from "node:url";

import type { ActivityEnvelope, AgentResponse } from "../types.js";

const PROTO_PATH = fileURLToPath(new URL("../../../../proto/agent.proto", import.meta.url));

interface AgentGatewayClient {
  handleActivity(activity: ActivityEnvelope): Promise<AgentResponse>;
  close(): void;
}

type GrpcUnaryCallback<T> = (error: grpc.ServiceError | null, response: T) => void;

interface GrpcAgentGatewayClient extends grpc.Client {
  handleActivity(request: Record<string, unknown>, callback: GrpcUnaryCallback<Record<string, unknown>>): grpc.ClientUnaryCall;
  sendProactive(request: Record<string, unknown>, callback: GrpcUnaryCallback<Record<string, unknown>>): grpc.ClientUnaryCall;
}

interface LoadedProto {
  teamsagent: {
    v1: {
      AgentGateway: grpc.ServiceClientConstructor;
    };
  };
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
      tenant_id: activity.actor.tenant_id,
      user_id: activity.actor.user_id,
      user_display_name: activity.actor.user_display_name ?? "",
    },
    conversation: {
      channel: activity.conversation.channel,
      conversation_id: activity.conversation.conversation_id,
      thread_id: activity.conversation.thread_id ?? "",
      message_id: activity.conversation.message_id ?? "",
    },
    text: activity.text,
    attachments: activity.attachments,
    action: activity.action ?? "",
    action_payload_json: activity.action_payload_json ?? "",
    recipients: activity.recipients,
    contains_sensitive: activity.contains_sensitive,
    request_hour_local: activity.request_hour_local,
    attendee_known: activity.attendee_known,
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

    return new Promise<AgentResponse>((resolve, reject) => {
      this.client.handleActivity(request, (error, response) => {
        if (error) {
          reject(new Error(`gRPC handleActivity failed: ${error.message}`));
          return;
        }

        resolve(fromGrpcResponse(response));
      });
    });
  }

  close(): void {
    this.client.close();
  }
}

export { toGrpcRequest, fromGrpcResponse };
