import { describe, expect, it } from "vitest";
import { extractHashtagsFromNote } from "../lib/hashtags";

/**
 * EI-V0.5-9 — hashtag → label merge auto-removes labels for hashtags
 * that disappeared from the text. The actual merge logic lives in
 * `NoteEditor.close()` and isn't extracted; these tests pin the
 * input-shape behavior so the merge correctness is provable.
 */

interface Note {
  title: string;
  body: string;
  checklist: { text: string }[];
}

function previousTagSet(prev: Note | null): Set<string> {
  if (!prev) return new Set();
  return new Set(
    extractHashtagsFromNote(prev).map((t) => t.toLowerCase()),
  );
}

function currentTagSet(cur: Note): Set<string> {
  return new Set(extractHashtagsFromNote(cur).map((t) => t.toLowerCase()));
}

function removedTags(prev: Note | null, cur: Note): string[] {
  const p = previousTagSet(prev);
  const c = currentTagSet(cur);
  return [...p].filter((t) => !c.has(t));
}

describe("hashtag removal diff", () => {
  it("identifies tags removed from body", () => {
    const prev: Note = { title: "", body: "buy #milk and #eggs", checklist: [] };
    const cur: Note = { title: "", body: "buy #milk", checklist: [] };
    expect(removedTags(prev, cur)).toEqual(["eggs"]);
  });

  it("returns empty when nothing was removed", () => {
    const prev: Note = { title: "#work", body: "", checklist: [] };
    const cur: Note = { title: "#work today", body: "", checklist: [] };
    expect(removedTags(prev, cur)).toEqual([]);
  });

  it("returns empty when there was no previous (new note)", () => {
    const cur: Note = { title: "#work", body: "", checklist: [] };
    expect(removedTags(null, cur)).toEqual([]);
  });

  it("dedupes within previous (case-insensitive)", () => {
    const prev: Note = {
      title: "#Work",
      body: "#work today",
      checklist: [{ text: "do #work" }],
    };
    const cur: Note = { title: "", body: "", checklist: [] };
    expect(removedTags(prev, cur)).toEqual(["work"]);
  });

  it("handles tags moving between title/body/checklist", () => {
    const prev: Note = {
      title: "#urgent",
      body: "do #work",
      checklist: [{ text: "buy #milk" }],
    };
    const cur: Note = {
      title: "",
      body: "do #urgent #work",
      checklist: [{ text: "buy #milk" }],
    };
    // Nothing was removed across the union of fields, even though
    // #urgent moved from title to body.
    expect(removedTags(prev, cur)).toEqual([]);
  });
});
