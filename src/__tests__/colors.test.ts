import { describe, expect, it } from "vitest";
import { COLOR_KEYS, LIGHT_HEX, DARK_HEX, bgFor, borderFor } from "../colors";

describe("color palette", () => {
  it("covers all 12 Keep colors in both modes", () => {
    expect(COLOR_KEYS).toHaveLength(12);
    for (const k of COLOR_KEYS) {
      expect(LIGHT_HEX[k]).toMatch(/^#[0-9A-F]{6}$/);
      expect(DARK_HEX[k]).toMatch(/^#[0-9A-F]{6}$/);
    }
  });

  it("bgFor returns dark hex in dark mode and light hex otherwise", () => {
    expect(bgFor("yellow", false)).toBe(LIGHT_HEX.yellow);
    expect(bgFor("yellow", true)).toBe(DARK_HEX.yellow);
  });

  it("borderFor returns transparent for non-default colors", () => {
    expect(borderFor("yellow", false)).toBe("transparent");
    expect(borderFor("red", true)).toBe("transparent");
  });

  it("borderFor returns a visible border for the default color in both modes", () => {
    expect(borderFor("default", false)).not.toBe("transparent");
    expect(borderFor("default", true)).not.toBe("transparent");
  });
});
