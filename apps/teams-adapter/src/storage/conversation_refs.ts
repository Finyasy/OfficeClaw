export interface ConversationRef {
  tenantId: string;
  userId: string;
  channel: string;
  conversationId: string;
  refJson: Record<string, unknown>;
  updatedAt: string;
}

function keyOf(ref: Pick<ConversationRef, "tenantId" | "userId" | "channel" | "conversationId">): string {
  return `${ref.tenantId}:${ref.userId}:${ref.channel}:${ref.conversationId}`;
}

export class InMemoryConversationRefStore {
  private readonly refs = new Map<string, ConversationRef>();

  upsert(input: Omit<ConversationRef, "updatedAt">): ConversationRef {
    const next: ConversationRef = {
      ...input,
      updatedAt: new Date().toISOString(),
    };

    this.refs.set(keyOf(next), next);
    return next;
  }

  get(tenantId: string, userId: string, channel: string, conversationId: string): ConversationRef | undefined {
    return this.refs.get(`${tenantId}:${userId}:${channel}:${conversationId}`);
  }

  size(): number {
    return this.refs.size;
  }
}
