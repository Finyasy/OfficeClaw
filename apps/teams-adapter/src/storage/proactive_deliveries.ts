import type { ProactiveDeliveryRequest } from "../types.js";

export class InMemoryProactiveDeliveryStore {
  private readonly deliveries: ProactiveDeliveryRequest[] = [];

  add(delivery: ProactiveDeliveryRequest): void {
    this.deliveries.push(delivery);
  }

  list(): ProactiveDeliveryRequest[] {
    return [...this.deliveries];
  }
}
