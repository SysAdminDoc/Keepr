import { describe, expect, it } from "vitest";
import { lineDiff } from "../components/HistoryDrawer";

describe("lineDiff", () => {
  it("returns only 'same' rows for identical input", () => {
    const out = lineDiff("a\nb\nc", "a\nb\nc");
    expect(out.every((r) => r.kind === "same")).toBe(true);
    expect(out.map((r) => r.line)).toEqual(["a", "b", "c"]);
  });

  it("marks a removed line", () => {
    const out = lineDiff("a\nb\nc", "a\nc");
    expect(out).toEqual([
      { kind: "same", line: "a" },
      { kind: "removed", line: "b" },
      { kind: "same", line: "c" },
    ]);
  });

  it("marks an added line", () => {
    const out = lineDiff("a\nc", "a\nb\nc");
    expect(out).toEqual([
      { kind: "same", line: "a" },
      { kind: "added", line: "b" },
      { kind: "same", line: "c" },
    ]);
  });

  it("handles a full replacement", () => {
    const out = lineDiff("old", "new");
    expect(out).toEqual([
      { kind: "removed", line: "old" },
      { kind: "added", line: "new" },
    ]);
  });

  it("handles empty old (everything added)", () => {
    const out = lineDiff("", "a\nb");
    // Empty string splits to [""]; ditto for the new body.
    // The shared empty-line at the head is "same".
    expect(out.filter((r) => r.kind === "added").map((r) => r.line)).toEqual(["a", "b"]);
  });

  it("handles empty new (everything removed)", () => {
    const out = lineDiff("a\nb", "");
    expect(out.filter((r) => r.kind === "removed").map((r) => r.line)).toEqual(["a", "b"]);
  });

  it("handles a middle-edit on multi-line bodies", () => {
    const out = lineDiff("a\nb\nc\nd", "a\nB\nc\nd");
    const kinds = out.map((r) => r.kind).join(",");
    expect(kinds).toBe("same,removed,added,same,same");
  });
});
