import { describe, expect, it } from "vitest";
import { extractHashtags, extractHashtagsFromNote } from "../lib/hashtags";

describe("extractHashtags", () => {
  it("picks up a single tag at start", () => {
    expect(extractHashtags("#groceries milk")).toEqual(["groceries"]);
  });

  it("picks up multiple tags in order", () => {
    expect(extractHashtags("milk #shopping eggs #weekend")).toEqual([
      "shopping",
      "weekend",
    ]);
  });

  it("dedupes case-insensitively, keeping first occurrence's case", () => {
    expect(extractHashtags("#Work and #work")).toEqual(["Work"]);
  });

  it("ignores URL fragments (no whitespace before #)", () => {
    expect(extractHashtags("see https://example.com/page#section now")).toEqual([]);
  });

  it("ignores pure-numeric tokens", () => {
    expect(extractHashtags("step #1 next #2 then #real")).toEqual(["real"]);
  });

  it("accepts hyphen and underscore inside tags", () => {
    expect(extractHashtags("#big-idea #snake_case")).toEqual([
      "big-idea",
      "snake_case",
    ]);
  });

  it("accepts Unicode letters", () => {
    expect(extractHashtags("#café #日本語")).toEqual(["café", "日本語"]);
  });

  it("handles empty / whitespace input", () => {
    expect(extractHashtags("")).toEqual([]);
    expect(extractHashtags("   ")).toEqual([]);
  });
});

describe("extractHashtagsFromNote", () => {
  it("merges across title, body, and checklist items, deduping", () => {
    const out = extractHashtagsFromNote({
      title: "#weekly review",
      body: "checked #weekly items and #planning todos",
      checklist: [{ text: "buy #groceries" }, { text: "drop off #books" }],
    });
    expect(out).toEqual(["weekly", "planning", "groceries", "books"]);
  });
});
