import type { Reminder } from "../types";

/** When this reminder is next due to fire — `snoozeUntil` if it's set
 *  (and still in the future), otherwise the underlying `fireAt`. */
export function effectiveFireAt(r: Reminder): string {
  if (r.snoozeUntil && r.snoozeUntil > r.fireAt) return r.snoozeUntil;
  return r.fireAt;
}

/** True for reminders that are still on the schedule — not yet fired
 *  (or recurring, in which case `firedAt` is never set), not dismissed. */
export function isActive(r: Reminder): boolean {
  if (r.dismissedAt) return false;
  if (r.firedAt) return false;
  return true;
}

/** Order by next-due ascending (soonest first). */
export function compareByDue(a: Reminder, b: Reminder): number {
  const ea = effectiveFireAt(a);
  const eb = effectiveFireAt(b);
  return ea < eb ? -1 : ea > eb ? 1 : 0;
}

/** Human label for the supported RRULE strings. Used in toasts + badges. */
export function recurrenceLabel(rrule: string | null | undefined): string {
  switch (rrule) {
    case "FREQ=DAILY":
      return "daily";
    case "FREQ=WEEKLY":
      return "weekly";
    case "FREQ=MONTHLY":
      return "monthly";
    case "FREQ=YEARLY":
      return "yearly";
    default:
      return "";
  }
}
