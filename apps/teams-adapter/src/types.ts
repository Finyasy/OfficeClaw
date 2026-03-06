export type ActionName =
  | "APPROVE_SEND"
  | "SELECT_SLOT"
  | "APPROVE_INVITE"
  | "CANCEL"
  | "CONFIRM_EXTERNAL_SEND"
  | "WEBHOOK_NOTIFICATION";

export interface ActivityEnvelope {
  actor: {
    tenant_id: string;
    user_id: string;
    user_display_name?: string;
  };
  conversation: {
    channel: string;
    conversation_id: string;
    thread_id?: string;
    message_id?: string;
  };
  text: string;
  attachments: Array<{ kind: string; id: string; data_json: string }>;
  action?: ActionName;
  action_payload_json?: string;
  recipients: string[];
  contains_sensitive: boolean;
  request_hour_local: number;
  attendee_known: boolean;
  attendee_email?: string;
  conversation_ref_json?: string;
}

export interface AgentResponse {
  text: string;
  adaptive_card_json?: string;
  actions?: Array<{ id: string; label: string; payload_json: string; style?: string }>;
  correlation_id: string;
}

export interface AuthEnvelope {
  actor: {
    tenant_id: string;
    user_id: string;
    user_display_name?: string;
  };
  provider: "graph";
  access_token: string;
  refresh_token?: string;
  expires_at_utc?: string;
  scope?: string;
}

export interface RawTeamsActivity {
  channelId?: string;
  id?: string;
  serviceUrl?: string;
  text?: string;
  from?: { aadObjectId?: string; name?: string };
  recipient?: { id?: string; name?: string };
  conversation?: { id?: string };
  channelData?: { tenant?: { id?: string } };
  value?: Record<string, unknown>;
  attachments?: Array<{ contentType?: string; id?: string; content?: unknown }>;
}

export interface ProactiveDeliveryRequest {
  actor: {
    tenant_id: string;
    user_id: string;
  };
  conversation: {
    channel: string;
    conversation_id: string;
    message_id?: string;
  };
  conversation_ref_json: string;
  text: string;
  adaptive_card_json?: string;
  correlation_id: string;
}
