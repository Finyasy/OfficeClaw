import type { ActivityEnvelope, RawTeamsActivity } from "../types.js";

function parseAction(rawValue: unknown): string | undefined {
  if (!rawValue || typeof rawValue !== "object") {
    return undefined;
  }

  const maybeAction = (rawValue as Record<string, unknown>).action;
  if (typeof maybeAction === "string" && maybeAction.length > 0) {
    return maybeAction;
  }

  return undefined;
}

function parseRecipients(rawValue: unknown): string[] {
  if (!rawValue || typeof rawValue !== "object") {
    return [];
  }

  const recipients = (rawValue as Record<string, unknown>).recipients;
  if (!Array.isArray(recipients)) {
    return [];
  }

  return recipients.filter((value): value is string => typeof value === "string" && value.length > 0);
}

function parseHour(rawValue: unknown): number {
  if (!rawValue || typeof rawValue !== "object") {
    return 10;
  }

  const value = (rawValue as Record<string, unknown>).requestHourLocal;
  if (typeof value !== "number" || !Number.isInteger(value) || value < 0 || value > 23) {
    return 10;
  }

  return value;
}

function parseAttendeeKnown(rawValue: unknown): boolean {
  if (!rawValue || typeof rawValue !== "object") {
    return true;
  }

  const value = (rawValue as Record<string, unknown>).attendeeKnown;
  if (typeof value !== "boolean") {
    return true;
  }

  return value;
}

export function normalizeActivity(activity: RawTeamsActivity): ActivityEnvelope {
  const tenantId = activity.channelData?.tenant?.id;
  const userId = activity.from?.aadObjectId;
  const conversationId = activity.conversation?.id;

  if (!tenantId) {
    throw new Error("Missing tenant id in activity payload");
  }

  if (!userId) {
    throw new Error("Missing user id in activity payload");
  }

  if (!conversationId) {
    throw new Error("Missing conversation id in activity payload");
  }

  const attachments = (activity.attachments ?? []).map((attachment) => ({
    kind: attachment.contentType ?? "unknown",
    id: attachment.id ?? "",
    data_json: JSON.stringify(attachment.content ?? {}),
  }));

  const action = parseAction(activity.value);
  const containsSensitive = Boolean((activity.value as Record<string, unknown> | undefined)?.containsSensitive);

  return {
    actor: {
      tenant_id: tenantId,
      user_id: userId,
      user_display_name: activity.from?.name,
    },
    conversation: {
      channel: activity.channelId ?? "teams",
      conversation_id: conversationId,
      message_id: activity.id,
    },
    text: activity.text ?? "",
    attachments,
    action: action as ActivityEnvelope["action"],
    action_payload_json: activity.value ? JSON.stringify(activity.value) : undefined,
    recipients: parseRecipients(activity.value),
    contains_sensitive: containsSensitive,
    request_hour_local: parseHour(activity.value),
    attendee_known: parseAttendeeKnown(activity.value),
  };
}
