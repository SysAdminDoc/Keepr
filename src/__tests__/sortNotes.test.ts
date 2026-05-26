import { describe, expect, it } from "vitest";
import { sortNotes } from "../store";
import type { Note } from "../types";

function makeNote(over: Partial<Note> = {}): Note {
  return {
    id: over.id ?? "n",
    kind: over.kind ?? "text",
    title: over.title ?? "",
    body: "",
    color: "default",
    pinned: over.pinned ?? false,
    archived: false,
    trashed: false,
    position: over.position ?? 0,
    created_at: over.created_at ?? "2026-01-01T00:00:00Z",
    updated_at: over.updated_at ?? "2026-01-01T00:00:00Z",
    trashed_at: null,
    checklist: [],
    labels: [],
  };
}

describe("sortNotes", () => {
  it("pins win regardless of mode", () => {
    const pinned = makeNote({ id: "p", pinned: true, updated_at: "2026-01-01T00:00:00Z" });
    const recent = makeNote({ id: "r", updated_at: "2026-05-01T00:00:00Z" });
    const sorted = sortNotes([recent, pinned], "modified");
    expect(sorted[0].id).toBe("p");
  });

  it("modified mode orders by updated_at DESC", () => {
    const a = makeNote({ id: "a", updated_at: "2026-01-01T00:00:00Z" });
    const b = makeNote({ id: "b", updated_at: "2026-05-01T00:00:00Z" });
    const c = makeNote({ id: "c", updated_at: "2026-03-01T00:00:00Z" });
    expect(sortNotes([a, b, c], "modified").map((n) => n.id)).toEqual([
      "b",
      "c",
      "a",
    ]);
  });

  it("created mode orders by created_at DESC", () => {
    const a = makeNote({ id: "a", created_at: "2026-01-01T00:00:00Z" });
    const b = makeNote({ id: "b", created_at: "2026-05-01T00:00:00Z" });
    expect(sortNotes([a, b], "created").map((n) => n.id)).toEqual(["b", "a"]);
  });

  it("title mode is case-insensitive A->Z", () => {
    const banana = makeNote({ id: "b", title: "banana" });
    const Apple = makeNote({ id: "a", title: "Apple" });
    const cherry = makeNote({ id: "c", title: "Cherry" });
    expect(sortNotes([banana, Apple, cherry], "title").map((n) => n.id)).toEqual([
      "a",
      "b",
      "c",
    ]);
  });

  it("custom mode orders by position ASC", () => {
    const a = makeNote({ id: "a", position: 2 });
    const b = makeNote({ id: "b", position: 0 });
    const c = makeNote({ id: "c", position: 1 });
    expect(sortNotes([a, b, c], "custom").map((n) => n.id)).toEqual([
      "b",
      "c",
      "a",
    ]);
  });

  it("custom mode ties broken by updated_at DESC", () => {
    const a = makeNote({ id: "a", position: 0, updated_at: "2026-01-01T00:00:00Z" });
    const b = makeNote({ id: "b", position: 0, updated_at: "2026-05-01T00:00:00Z" });
    expect(sortNotes([a, b], "custom").map((n) => n.id)).toEqual(["b", "a"]);
  });
});
