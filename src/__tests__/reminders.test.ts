import { describe, expect, it } from "vitest";
import type { Note, Reminder, Section } from "../types";
import { EMPTY_FILTERS } from "../types";
import {
  compareByDue,
  effectiveFireAt,
  isActive,
  recurrenceLabel,
} from "../lib/reminders";
import { filterNotes } from "../lib/filterNotes";

function reminder(over: Partial<Reminder> = {}): Reminder {
  return {
    id: over.id ?? "r1",
    noteId: over.noteId ?? "n1",
    fireAt: over.fireAt ?? "2026-06-01T12:00:00Z",
    rrule: over.rrule ?? null,
    snoozeUntil: over.snoozeUntil ?? null,
    firedAt: over.firedAt ?? null,
    dismissedAt: over.dismissedAt ?? null,
    createdAt: over.createdAt ?? "2026-05-01T00:00:00Z",
  };
}

function note(over: Partial<Note> = {}): Note {
  return {
    id: over.id ?? "n1",
    kind: over.kind ?? "text",
    title: over.title ?? "",
    body: over.body ?? "",
    color: over.color ?? "default",
    pinned: over.pinned ?? false,
    archived: over.archived ?? false,
    trashed: over.trashed ?? false,
    position: over.position ?? 0,
    created_at: over.created_at ?? "2026-01-01T00:00:00Z",
    updated_at: over.updated_at ?? "2026-01-01T00:00:00Z",
    trashed_at: over.trashed_at ?? null,
    checklist: over.checklist ?? [],
    labels: over.labels ?? [],
    attachments: over.attachments ?? [],
  };
}

describe("reminders helpers", () => {
  it("effectiveFireAt prefers a snooze that pushes the fire later", () => {
    const r = reminder({
      fireAt: "2026-06-01T12:00:00Z",
      snoozeUntil: "2026-06-01T13:00:00Z",
    });
    expect(effectiveFireAt(r)).toBe("2026-06-01T13:00:00Z");
  });

  it("effectiveFireAt ignores a snooze that's already in the past", () => {
    const r = reminder({
      fireAt: "2026-06-01T12:00:00Z",
      snoozeUntil: "2026-05-30T00:00:00Z",
    });
    expect(effectiveFireAt(r)).toBe("2026-06-01T12:00:00Z");
  });

  it("isActive rejects fired and dismissed reminders", () => {
    expect(isActive(reminder())).toBe(true);
    expect(isActive(reminder({ firedAt: "2026-06-01T12:00:00Z" }))).toBe(false);
    expect(isActive(reminder({ dismissedAt: "2026-06-01T12:00:00Z" }))).toBe(false);
  });

  it("recurrenceLabel maps the four supported RRULE strings", () => {
    expect(recurrenceLabel("FREQ=DAILY")).toBe("daily");
    expect(recurrenceLabel("FREQ=WEEKLY")).toBe("weekly");
    expect(recurrenceLabel("FREQ=MONTHLY")).toBe("monthly");
    expect(recurrenceLabel("FREQ=YEARLY")).toBe("yearly");
    expect(recurrenceLabel(null)).toBe("");
    expect(recurrenceLabel("FREQ=HOURLY")).toBe("");
  });

  it("compareByDue sorts soonest first using effectiveFireAt", () => {
    const a = reminder({ id: "a", fireAt: "2026-06-01T12:00:00Z" });
    const b = reminder({
      id: "b",
      fireAt: "2026-06-01T10:00:00Z",
      snoozeUntil: "2026-06-01T15:00:00Z",
    });
    const c = reminder({ id: "c", fireAt: "2026-06-01T09:00:00Z" });
    const sorted = [a, b, c].slice().sort(compareByDue);
    expect(sorted.map((r) => r.id)).toEqual(["c", "a", "b"]);
  });
});

describe("filterNotes — reminders section", () => {
  const section = (s: Section) => s;

  it("returns notes ordered by next due, excluding trashed and notes without a reminder", () => {
    const n1 = note({ id: "n1", title: "first" });
    const n2 = note({ id: "n2", title: "second" });
    const nTrash = note({ id: "trash", trashed: true });
    const nNoReminder = note({ id: "lonely" });
    const r1 = reminder({ id: "r1", noteId: "n1", fireAt: "2026-06-02T00:00:00Z" });
    const r2 = reminder({ id: "r2", noteId: "n2", fireAt: "2026-06-01T00:00:00Z" });
    const rTrash = reminder({ id: "rt", noteId: "trash", fireAt: "2026-05-30T00:00:00Z" });
    const rFired = reminder({
      id: "rf",
      noteId: "lonely",
      firedAt: "2026-05-01T00:00:00Z",
    });
    const out = filterNotes(
      [n1, n2, nTrash, nNoReminder],
      section({ kind: "reminders" }),
      "",
      EMPTY_FILTERS,
      [r1, r2, rTrash, rFired],
    );
    expect(out.map((n) => n.id)).toEqual(["n2", "n1"]);
  });
});
