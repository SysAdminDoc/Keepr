import { describe, expect, it } from "vitest";
import type { Note, Section } from "../types";

// `filterNotes` is currently a private helper in App.tsx. Re-export it so
// tests can pin behavior. (When App.tsx is refactored to call a `src/lib/`
// helper, change this import to point there.)
import { filterNotes } from "../lib/filterNotes";

function makeNote(over: Partial<Note> = {}): Note {
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

describe("filterNotes", () => {
  const active = makeNote({ id: "active", title: "Active" });
  const archived = makeNote({ id: "arch", title: "Archived", archived: true });
  const trashed = makeNote({ id: "tra", title: "Trashed", trashed: true });
  const labeled = makeNote({ id: "lbl", title: "With label", labels: ["L1"] });

  const all = [active, archived, trashed, labeled];

  const section = (s: Section) => s;

  it("Notes section excludes archived and trashed", () => {
    const out = filterNotes(all, section({ kind: "notes" }), "");
    const ids = out.map((n) => n.id);
    expect(ids).toContain("active");
    expect(ids).toContain("lbl");
    expect(ids).not.toContain("arch");
    expect(ids).not.toContain("tra");
  });

  it("Archive section shows only archived non-trashed notes", () => {
    const out = filterNotes(all, section({ kind: "archive" }), "");
    expect(out.map((n) => n.id)).toEqual(["arch"]);
  });

  it("Trash section shows only trashed notes", () => {
    const out = filterNotes(all, section({ kind: "trash" }), "");
    expect(out.map((n) => n.id)).toEqual(["tra"]);
  });

  it("Label section filters to labeled notes that are not trashed", () => {
    const trashedLabeled = makeNote({
      id: "tlbl",
      labels: ["L1"],
      trashed: true,
    });
    const out = filterNotes(
      [...all, trashedLabeled],
      section({ kind: "label", labelId: "L1" }),
      "",
    );
    expect(out.map((n) => n.id)).toEqual(["lbl"]);
  });

  it("Label section with a non-existent label returns empty", () => {
    const out = filterNotes(all, section({ kind: "label", labelId: "ghost" }), "");
    expect(out).toEqual([]);
  });

  it("Search matches title, body, and checklist text case-insensitively", () => {
    const titleHit = makeNote({ id: "t", title: "Milk in TITLE" });
    const bodyHit = makeNote({ id: "b", body: "Has milk in body" });
    const itemHit = makeNote({
      id: "i",
      kind: "list",
      checklist: [
        { id: "1", text: "Buy MILK", checked: false, position: 0 },
      ],
    });
    const miss = makeNote({ id: "m", title: "nothing here" });
    const out = filterNotes(
      [titleHit, bodyHit, itemHit, miss],
      section({ kind: "notes" }),
      "milk",
    );
    expect(out.map((n) => n.id).sort()).toEqual(["b", "i", "t"]);
  });

  it("Empty search returns the unfiltered section", () => {
    const out = filterNotes(all, section({ kind: "notes" }), "   ");
    expect(out).toHaveLength(2); // active + labeled
  });

  it("EI-18 — when searchMatchIds is provided, narrows to those IDs (ignores substring scan)", () => {
    // The substring "ctive" would normally match `active`, but the
    // FTS5-supplied Set is the source of truth when present.
    const out = filterNotes(
      all,
      section({ kind: "notes" }),
      "ctive",
      undefined,
      undefined,
      new Set(["lbl"]),
    );
    expect(out.map((n) => n.id)).toEqual(["lbl"]);
  });

  it("EI-18 — searchMatchIds intersects with section + filters (cross-facet AND)", () => {
    // FTS5 might return matches in any section; the section filter
    // (Trash here) still narrows the pool first.
    const out = filterNotes(
      all,
      section({ kind: "trash" }),
      "foo",
      undefined,
      undefined,
      new Set(["tra", "active"]),
    );
    expect(out.map((n) => n.id)).toEqual(["tra"]);
  });
});
