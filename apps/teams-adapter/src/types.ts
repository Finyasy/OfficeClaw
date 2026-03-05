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
}

export interface AgentResponse {
  text: string;
  adaptive_card_json?: string;
  actions?: Array<{ id: string; label: string; payload_json: string; style?: string }>;
  correlation_id: string;
}

export interface RawTeamsActivity {
  channelId?: string;
  id?: string;
  text?: string;
  from?: { aadObjectId?: string; name?: string };
  conversation?: { id?: string };
  channelData?: { tenant?: { id?: string } };
  value?: Record<string, unknown>;
  attachments?: Array<{ contentType?: string; id?: string; content?: unknown }>;
}
