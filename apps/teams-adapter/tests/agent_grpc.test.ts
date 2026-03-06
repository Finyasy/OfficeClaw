import { describe, expect, it } from "vitest";
import { fromGrpcResponse, toGrpcAuthRequest, toGrpcRequest } from "../src/transport/agent_grpc.js";
import type { ActivityEnvelope, AuthEnvelope } from "../src/types.js";

function sampleActivity(): ActivityEnvelope {
  return {
    actor: {
      tenant_id: "tenant-1",
      user_id: "user-1",
      user_display_name: "Bryan",
    },
    conversation: {
      channel: "teams",
      conversation_id: "conv-1",
      message_id: "msg-1",
    },
    text: "summarize unread",
    attachments: [{ kind: "email", id: "mail-1", data_json: "{}" }],
    action: "APPROVE_SEND",
    action_payload_json: "{}",
    recipients: ["james@contoso.com"],
    contains_sensitive: false,
    request_hour_local: 11,
    attendee_known: true,
    attendee_email: "james@contoso.com",
  };
}

describe("toGrpcRequest", () => {
  it("maps adapter activity to grpc payload", () => {
    const req = toGrpcRequest(sampleActivity());

    expect(req).toMatchObject({
      text: "summarize unread",
      action: "APPROVE_SEND",
      recipients: ["james@contoso.com"],
      containsSensitive: false,
      requestHourLocal: 11,
      attendeeKnown: true,
      attendeeEmail: "james@contoso.com",
    });
  });

  it("uses empty defaults for optional thread and payload fields", () => {
    const activity = sampleActivity();
    delete activity.conversation.thread_id;
    delete activity.action_payload_json;

    const req = toGrpcRequest(activity);

    expect(req).toMatchObject({
      conversation: {
        threadId: "",
      },
      actionPayloadJson: "",
    });
  });
});

describe("fromGrpcResponse", () => {
  it("maps grpc response into adapter response", () => {
    const response = fromGrpcResponse({
      text: "Draft ready",
      adaptiveCardJson: "{}",
      correlationId: "corr-1",
      actions: [{ id: "APPROVE_SEND", label: "Approve", payloadJson: "{}", style: "primary" }],
    });

    expect(response.text).toBe("Draft ready");
    expect(response.correlation_id).toBe("corr-1");
    expect(response.actions?.[0]?.id).toBe("APPROVE_SEND");
  });

  it("returns safe defaults for malformed grpc response", () => {
    const response = fromGrpcResponse({
      text: 123,
      actions: [null, "bad"],
    });

    expect(response.text).toBe("");
    expect(response.correlation_id).toBe("");
    expect(response.actions).toEqual([]);
  });
});

describe("toGrpcAuthRequest", () => {
  it("maps adapter auth payload to grpc request", () => {
    const auth: AuthEnvelope = {
      actor: {
        tenant_id: "tenant-1",
        user_id: "user-1",
        user_display_name: "Bryan",
      },
      provider: "graph",
      access_token: "access",
      refresh_token: "refresh",
      expires_at_utc: "2026-03-06T12:00:00Z",
      scope: "Mail.Read Calendars.Read",
    };

    const request = toGrpcAuthRequest(auth);

    expect(request).toMatchObject({
      provider: "graph",
      accessToken: "access",
      refreshToken: "refresh",
      scope: "Mail.Read Calendars.Read",
    });
  });
});
