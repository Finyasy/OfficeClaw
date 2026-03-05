import { describe, expect, it } from "vitest";
import { InMemoryConversationRefStore } from "../src/storage/conversation_refs.js";

describe("InMemoryConversationRefStore", () => {
  it("upserts and reads conversation refs", () => {
    const store = new InMemoryConversationRefStore();

    store.upsert({
      tenantId: "tenant-1",
      userId: "user-1",
      channel: "teams",
      conversationId: "conv-1",
      refJson: { serviceUrl: "https://service" },
    });

    const found = store.get("tenant-1", "user-1", "teams", "conv-1");

    expect(found).toBeDefined();
    expect(found?.refJson).toEqual({ serviceUrl: "https://service" });
  });

  it("overwrites on duplicate key", () => {
    const store = new InMemoryConversationRefStore();

    store.upsert({
      tenantId: "tenant-1",
      userId: "user-1",
      channel: "teams",
      conversationId: "conv-1",
      refJson: { serviceUrl: "https://a" },
    });

    store.upsert({
      tenantId: "tenant-1",
      userId: "user-1",
      channel: "teams",
      conversationId: "conv-1",
      refJson: { serviceUrl: "https://b" },
    });

    const found = store.get("tenant-1", "user-1", "teams", "conv-1");
    expect(found?.refJson).toEqual({ serviceUrl: "https://b" });
    expect(store.size()).toBe(1);
  });

  it("returns undefined for missing key", () => {
    const store = new InMemoryConversationRefStore();
    expect(store.get("x", "y", "teams", "z")).toBeUndefined();
  });
});
