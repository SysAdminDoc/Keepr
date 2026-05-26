import { describe, expect, it } from "vitest";
import {
  BACKGROUND_PATTERNS,
  BACKGROUND_PATTERN_ORDER,
  normalizePattern,
} from "../lib/backgroundPatterns";

describe("backgroundPatterns", () => {
  it("the order array covers exactly the keys in the pattern map", () => {
    const orderKeys = [...BACKGROUND_PATTERN_ORDER].sort();
    const mapKeys = Object.keys(BACKGROUND_PATTERNS).sort();
    expect(orderKeys).toEqual(mapKeys);
  });

  it("every non-empty pattern produces a data URL string", () => {
    for (const k of BACKGROUND_PATTERN_ORDER) {
      const v = BACKGROUND_PATTERNS[k];
      if (k === "") {
        expect(v).toBe("");
      } else {
        expect(v).toMatch(/^url\("data:image\/svg\+xml/);
      }
    }
  });

  it("normalizePattern accepts every valid key and rejects unknowns to ''", () => {
    for (const k of BACKGROUND_PATTERN_ORDER) {
      expect(normalizePattern(k)).toBe(k);
    }
    expect(normalizePattern(null)).toBe("");
    expect(normalizePattern(undefined)).toBe("");
    expect(normalizePattern("not-a-real-pattern")).toBe("");
    expect(normalizePattern("")).toBe("");
  });
});
