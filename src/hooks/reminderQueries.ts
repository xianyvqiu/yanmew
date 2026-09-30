import { reminderApi } from "../api";
import type { ReminderOccurrence } from "../types";

/** 名称展示可去重，但展示确认必须保留所有触发记录 ID。 */
export async function readMissedReminders(): Promise<ReminderOccurrence[]> {
  const items = new Map<string, ReminderOccurrence>();
  let after: string | null = null;
  do {
    const page = await reminderApi.missed(after);
    page.occurrences.forEach(item => items.set(item.id, item));
    after = page.nextAfterId;
  } while (after !== null);
  return [...items.values()];
}

export async function readQueuedReminders(): Promise<ReminderOccurrence[]> {
  const items = new Map<string, ReminderOccurrence>();
  let after: string | undefined;
  do {
    const page = await reminderApi.listOccurrences({ deliveryState: "queued", afterId: after, limit: 200 });
    page.items.forEach(item => { if (item.status === "pending") items.set(item.id, item); });
    after = page.nextAfterId ?? undefined;
  } while (after !== undefined);
  return [...items.values()];
}
