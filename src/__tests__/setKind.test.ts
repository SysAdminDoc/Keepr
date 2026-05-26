import { describe, expect, it } from "vitest";

/**
 * The text <-> list kind conversion that NoteEditor performs is the riskiest
 * round-trip in the app (EI-22). We extract the parse/serialize halves into
 * pure helpers here for unit testing.
 *
 * These mirror the inline logic in src/components/NoteEditor.tsx. If the
 * inline logic changes, mirror it here.
 */

interface Item {
  text: string;
  checked: boolean;
}

function listToText(items: Item[]): string {
  return items.map((c) => `- [${c.checked ? "x" : " "}] ${c.text}`).join("\n");
}

function textToList(body: string): Item[] {
  const lines = body
    .split(/\r?\n/)
    .map((s) => s.replace(/^\s+|\s+$/g, ""))
    .filter(Boolean);
  if (lines.length === 0) return [{ text: "", checked: false }];
  return lines.map((line) => {
    const m = /^[-*]?\s*\[( |x|X)\]\s+(.*)$/.exec(line);
    if (m) return { text: m[2], checked: m[1].toLowerCase() === "x" };
    return { text: line, checked: false };
  });
}

describe("setKind round-trip", () => {
  it("preserves text and checked state across list -> text -> list", () => {
    const items: Item[] = [
      { text: "Milk", checked: false },
      { text: "Eggs", checked: true },
      { text: "Bread", checked: false },
    ];
    const body = listToText(items);
    expect(body).toBe("- [ ] Milk\n- [x] Eggs\n- [ ] Bread");
    const back = textToList(body);
    expect(back).toEqual(items);
  });

  it("plain lines (no marker) parse as unchecked items", () => {
    const items = textToList("alpha\nbeta\ngamma");
    expect(items).toEqual([
      { text: "alpha", checked: false },
      { text: "beta", checked: false },
      { text: "gamma", checked: false },
    ]);
  });

  it("empty body produces a single empty unchecked item", () => {
    expect(textToList("")).toEqual([{ text: "", checked: false }]);
  });

  it("uppercase X is treated as checked", () => {
    const items = textToList("- [X] CAPS");
    expect(items).toEqual([{ text: "CAPS", checked: true }]);
  });

  it("asterisk bullets are accepted", () => {
    const items = textToList("* [x] starred");
    expect(items).toEqual([{ text: "starred", checked: true }]);
  });
});
