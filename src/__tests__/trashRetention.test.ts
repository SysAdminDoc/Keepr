import { describe, expect, it } from "vitest";
import type { Note } from "../types";
import { daysLeftInTrash, findExpiredTrashed } from "../lib/trashRetention";

function makeNote(over: Partial<Note> = {}): Note {
  return {
    id: over.id ?? "n",
    kind: "text",
    title: "",
    body: "",
    color: "default",
    pinned: false,
    archived: false,
    trashed: over.trashed ?? false,
    position: 0,
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
    trashed_at: over.trashed_at ?? null,
    checklist: [],
    labels: [],
    attachments: [],
  };
}

const NOW = Date.UTC(2026, 4, 25, 12, 0, 0); // 2026-05-25T12:00:00Z

describe("daysLeftInTrash", () => {
  it("returns null when retention is disabled", () => {
    const n = makeNote({ trashed: true, trashed_at: "2026-05-20T12:00:00Z" });
    expect(daysLeftInTrash(n, 0, NOW)).toBeNull();
  });

  it("returns null when the note is not in trash", () => {
    const n = makeNote({ trashed: false });
    expect(daysLeftInTrash(n, 7, NOW)).toBeNull();
  });

  it("returns the integer number of days until purge", () => {
    // Trashed 2 days ago, retention 7 → 5 days left
    const n = makeNote({ trashed: true, trashed_at: "2026-05-23T12:00:00Z" });
    expect(daysLeftInTrash(n, 7, NOW)).toBe(5);
  });

  it("returns 0 when the window has elapsed", () => {
    const n = makeNote({ trashed: true, trashed_at: "2026-05-01T00:00:00Z" });
    expect(daysLeftInTrash(n, 7, NOW)).toBe(0);
  });

  it("returns null when trashed_at is missing", () => {
    const n = makeNote({ trashed: true, trashed_at: null });
    expect(daysLeftInTrash(n, 7, NOW)).toBeNull();
  });
});

describe("findExpiredTrashed", () => {
  it("returns trashed notes past their retention window", () => {
    const fresh = makeNote({ id: "fresh", trashed: true, trashed_at: "2026-05-24T12:00:00Z" });
    const stale = makeNote({ id: "stale", trashed: true, trashed_at: "2026-05-01T00:00:00Z" });
    const active = makeNote({ id: "active", trashed: false });
    const expired = findExpiredTrashed([fresh, stale, active], 7, NOW);
    expect(expired.map((n) => n.id)).toEqual(["stale"]);
  });

  it("is empty when retention is disabled", () => {
    const old = makeNote({ id: "x", trashed: true, trashed_at: "2020-01-01T00:00:00Z" });
    expect(findExpiredTrashed([old], 0, NOW)).toEqual([]);
  });
});
