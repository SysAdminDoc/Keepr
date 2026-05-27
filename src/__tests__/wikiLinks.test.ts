import { describe, expect, it } from "vitest";
import { extractWikiLinks, findBacklinks, resolveTitle } from "../lib/wikiLinks";
import type { Note } from "../types";

function note(over: Partial<Note> = {}): Note {
  return {
    id: over.id ?? "n",
    kind: "text",
    title: over.title ?? "",
    body: over.body ?? "",
    color: "default",
    pinned: false,
    archived: false,
    trashed: over.trashed ?? false,
    position: 0,
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
    trashed_at: null,
    checklist: [],
    labels: [],
    attachments: [],
  };
}

describe("extractWikiLinks", () => {
  it("returns empty for empty body", () => {
    expect(extractWikiLinks("")).toEqual([]);
  });

  it("extracts a single link", () => {
    expect(extractWikiLinks("see [[Foo]] for context")).toEqual(["Foo"]);
  });

  it("extracts multiple links in document order", () => {
    expect(extractWikiLinks("[[A]] then [[B]] then [[C]]")).toEqual(["A", "B", "C"]);
  });

  it("trims whitespace inside brackets", () => {
    expect(extractWikiLinks("[[  Project Plan  ]]")).toEqual(["Project Plan"]);
  });

  it("dedupes case-insensitively", () => {
    expect(extractWikiLinks("[[Foo]] and again [[FOO]] and [[foo]]")).toEqual(["Foo"]);
  });

  it("skips empty brackets", () => {
    expect(extractWikiLinks("[[]] keep [[real]]")).toEqual(["real"]);
  });

  it("doesn't span newlines", () => {
    expect(extractWikiLinks("[[Foo\nBar]]")).toEqual([]);
  });
});

describe("resolveTitle", () => {
  const notes = [note({ id: "1", title: "Project Plan" }), note({ id: "2", title: "Other" })];
  it("matches case-insensitively", () => {
    expect(resolveTitle(notes, "project plan")?.id).toBe("1");
    expect(resolveTitle(notes, "PROJECT PLAN")?.id).toBe("1");
  });
  it("skips trashed notes", () => {
    const withTrash = [...notes, note({ id: "3", title: "Trashed", trashed: true })];
    expect(resolveTitle(withTrash, "Trashed")).toBeUndefined();
  });
});

describe("findBacklinks", () => {
  it("finds all notes that mention the target", () => {
    const notes = [
      note({ id: "a", body: "see [[Target]]" }),
      note({ id: "b", body: "no mention" }),
      note({ id: "c", body: "mentions [[target]] in lowercase" }),
    ];
    const back = findBacklinks(notes, "Target", "self");
    expect(back.map((n) => n.id).sort()).toEqual(["a", "c"]);
  });

  it("excludes the note itself", () => {
    const notes = [note({ id: "self", title: "Self", body: "[[Self]]" })];
    expect(findBacklinks(notes, "Self", "self")).toEqual([]);
  });

  it("skips trashed sources", () => {
    const notes = [note({ id: "live", body: "[[Foo]]" }), note({ id: "dead", body: "[[Foo]]", trashed: true })];
    expect(findBacklinks(notes, "Foo", "x").map((n) => n.id)).toEqual(["live"]);
  });
});
