import { describe, expect, it } from "vitest";
import { normalizeActivity } from "../src/bot/bot.js";

describe("normalizeActivity", () => {
  it("normalizes valid Teams activity", () => {
    const envelope = normalizeActivity({
      channelId: "teams",
      id: "msg-1",
      serviceUrl: "https://smba.trafficmanager.net/teams/",
      text: "reply to this email",
      from: { aadObjectId: "user-1", name: "Bryan" },
      recipient: { id: "bot-1", name: "OfficeClaw" },
      conversation: { id: "conv-1" },
      channelData: { tenant: { id: "tenant-1" } },
      value: {
        action: "APPROVE_SEND",
        recipients: ["james@contoso.com"],
        requestHourLocal: 11,
        attendeeEmail: "james@contoso.com",
      },
    });

    expect(envelope.actor.user_id).toBe("user-1");
    expect(envelope.action).toBe("APPROVE_SEND");
    expect(envelope.recipients).toEqual(["james@contoso.com"]);
    expect(envelope.request_hour_local).toBe(11);
    expect(envelope.attendee_email).toBe("james@contoso.com");
    expect(envelope.conversation_ref_json).toContain("conv-1");
    expect(envelope.conversation_ref_json).toContain("OfficeClaw");
  });

  it("throws when tenant id is missing", () => {
    expect(() =>
      normalizeActivity({
        from: { aadObjectId: "user-1" },
        conversation: { id: "conv-1" },
      }),
    ).toThrow(/tenant id/);
  });

  it("throws when conversation id is missing", () => {
    expect(() =>
      normalizeActivity({
        from: { aadObjectId: "user-1" },
        channelData: { tenant: { id: "tenant-1" } },
      }),
    ).toThrow(/conversation id/);
  });

  it("falls back to default hour for invalid payload", () => {
    const envelope = normalizeActivity({
      from: { aadObjectId: "user-1" },
      conversation: { id: "conv-1" },
      channelData: { tenant: { id: "tenant-1" } },
      value: { requestHourLocal: 99 },
    });

    expect(envelope.request_hour_local).toBe(10);
  });

  it("defaults attendee_known to true when missing", () => {
    const envelope = normalizeActivity({
      from: { aadObjectId: "user-1" },
      conversation: { id: "conv-1" },
      channelData: { tenant: { id: "tenant-1" } },
    });

    expect(envelope.attendee_known).toBe(true);
  });

  it("maps attendeeKnown boolean from activity value", () => {
    const envelope = normalizeActivity({
      from: { aadObjectId: "user-1" },
      conversation: { id: "conv-1" },
      channelData: { tenant: { id: "tenant-1" } },
      value: { attendeeKnown: false },
    });

    expect(envelope.attendee_known).toBe(false);
  });

  it("ignores malformed recipient array entries", () => {
    const envelope = normalizeActivity({
      from: { aadObjectId: "user-1" },
      conversation: { id: "conv-1" },
      channelData: { tenant: { id: "tenant-1" } },
      value: { recipients: ["valid@contoso.com", 123, null] },
    });

    expect(envelope.recipients).toEqual(["valid@contoso.com"]);
  });
});
