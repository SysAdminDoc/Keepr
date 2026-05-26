import { describe, expect, it, beforeEach, afterEach, vi } from "vitest";

/**
 * NF-02 / EI-V0.5-18 — ReminderPicker preset date math is computed
 * inline in the component. Lift it here as pure helpers so we can pin
 * the behavior without rendering React. If the inline logic in
 * `src/components/ReminderPicker.tsx` ever drifts from these, the
 * intent is that ReminderPicker imports from this module instead of
 * mirroring it (left as a v0.5.1 refactor — too risky to bundle into
 * the test PR).
 */

function laterToday(now: Date): Date {
  const d = new Date(now);
  d.setHours(18, 0, 0, 0);
  if (d.getTime() <= now.getTime()) {
    // Already past 6 PM today — push to tomorrow's 6 PM.
    d.setDate(d.getDate() + 1);
  }
  return d;
}

function tomorrowMorning(now: Date): Date {
  const d = new Date(now);
  d.setDate(d.getDate() + 1);
  d.setHours(8, 0, 0, 0);
  return d;
}

function nextMonday(now: Date): Date {
  const d = new Date(now);
  const days = ((1 - d.getDay() + 7) % 7) || 7; // 0 (Sunday) → 1 day, 1 (Monday) → 7 days
  d.setDate(d.getDate() + days);
  d.setHours(8, 0, 0, 0);
  return d;
}

describe("ReminderPicker presets", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  describe("Later today", () => {
    it("returns 6 PM today when called before 6 PM", () => {
      vi.setSystemTime(new Date("2026-05-26T10:00:00"));
      const d = laterToday(new Date());
      expect(d.getHours()).toBe(18);
      expect(d.getMinutes()).toBe(0);
      expect(d.getDate()).toBe(26);
    });

    it("rolls to tomorrow 6 PM when called after 6 PM", () => {
      vi.setSystemTime(new Date("2026-05-26T20:00:00"));
      const d = laterToday(new Date());
      expect(d.getHours()).toBe(18);
      expect(d.getDate()).toBe(27);
    });
  });

  describe("Tomorrow morning", () => {
    it("is always 8 AM the next calendar day", () => {
      vi.setSystemTime(new Date("2026-05-26T15:30:00"));
      const d = tomorrowMorning(new Date());
      expect(d.getDate()).toBe(27);
      expect(d.getHours()).toBe(8);
      expect(d.getMinutes()).toBe(0);
    });

    it("handles month boundaries", () => {
      vi.setSystemTime(new Date("2026-05-31T15:00:00"));
      const d = tomorrowMorning(new Date());
      expect(d.getMonth()).toBe(5); // June
      expect(d.getDate()).toBe(1);
    });
  });

  describe("Next Monday", () => {
    it("from Sunday → tomorrow Monday", () => {
      // 2026-05-24 is a Sunday (verified)
      vi.setSystemTime(new Date("2026-05-24T10:00:00"));
      const d = nextMonday(new Date());
      expect(d.getDay()).toBe(1); // Monday
      expect(d.getDate()).toBe(25);
    });

    it("from Monday → next Monday (7 days)", () => {
      // 2026-05-25 is a Monday; +7 days = 2026-06-01
      vi.setSystemTime(new Date("2026-05-25T10:00:00"));
      const d = nextMonday(new Date());
      expect(d.getDay()).toBe(1);
      expect(d.getMonth()).toBe(5); // June
      expect(d.getDate()).toBe(1);
    });

    it("from Tuesday → following Monday (6 days)", () => {
      // 2026-05-26 is a Tuesday; +6 days = 2026-06-01
      vi.setSystemTime(new Date("2026-05-26T10:00:00"));
      const d = nextMonday(new Date());
      expect(d.getDay()).toBe(1);
      expect(d.getMonth()).toBe(5);
      expect(d.getDate()).toBe(1);
    });
  });
});
