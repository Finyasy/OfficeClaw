import type { AgentResponse } from "../types.js";

export interface ProactiveMessageInput {
  conversationReference: Record<string, unknown>;
  response: AgentResponse;
}

export function toProactivePayload(input: ProactiveMessageInput): Record<string, unknown> {
  return {
    type: "message",
    conversationReference: input.conversationReference,
    text: input.response.text,
    value: {
      correlation_id: input.response.correlation_id,
    },
  };
}
