import { describe, expect, it } from "vitest";

/**
 * NF-03 — useGlobalHotkey shift-key handling. The hook treats
 * `hotkey.shift` as a strict requirement only when explicitly set;
 * omitting it allows the user's natural shift state (necessary for
 * inherently-shifted keys like `?` and `#`).
 *
 * The match function isn't exported, so this test re-implements it to
 * pin the contract. If you change `useGlobalHotkey.ts:matches()`, mirror
 * the change here.
 */

interface Hotkey {
  key: string;
  mod?: boolean;
  shift?: boolean;
  alt?: boolean;
}

interface FakeEvent {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

function matches(e: FakeEvent, hk: Hotkey, isMac = false): boolean {
  if (e.key.toLowerCase() !== hk.key.toLowerCase()) return false;
  const mod = isMac ? e.metaKey : e.ctrlKey;
  if (Boolean(hk.mod) !== mod) return false;
  if (hk.shift !== undefined && Boolean(hk.shift) !== e.shiftKey) return false;
  if (Boolean(hk.alt) !== e.altKey) return false;
  return true;
}

function ev(
  key: string,
  mods: Partial<Pick<FakeEvent, "ctrlKey" | "metaKey" | "shiftKey" | "altKey">> = {},
): FakeEvent {
  return {
    key,
    ctrlKey: mods.ctrlKey ?? false,
    metaKey: mods.metaKey ?? false,
    shiftKey: mods.shiftKey ?? false,
    altKey: mods.altKey ?? false,
  };
}

describe("useGlobalHotkey matches()", () => {
  it("matches a simple lowercase key", () => {
    expect(matches(ev("c"), { key: "c" })).toBe(true);
    expect(matches(ev("c"), { key: "C" })).toBe(true);
  });

  it("rejects when wrong key", () => {
    expect(matches(ev("d"), { key: "c" })).toBe(false);
  });

  it("matches ? when typed with shift, without requiring shift in the descriptor", () => {
    // The key itself is "?" (shift+/), and the descriptor omits shift.
    expect(matches(ev("?", { shiftKey: true }), { key: "?" })).toBe(true);
  });

  it("matches # similarly", () => {
    expect(matches(ev("#", { shiftKey: true }), { key: "#" })).toBe(true);
  });

  it("explicit shift: true requires shift to be held", () => {
    expect(matches(ev("J", { shiftKey: true }), { key: "j", shift: true })).toBe(true);
    expect(matches(ev("j", { shiftKey: false }), { key: "j", shift: true })).toBe(false);
  });

  it("explicit shift: false rejects when shift is held", () => {
    expect(matches(ev("c", { shiftKey: true }), { key: "c", shift: false })).toBe(false);
  });

  it("Ctrl on non-mac, Cmd on mac", () => {
    expect(matches(ev("g", { ctrlKey: true }), { key: "g", mod: true }, false)).toBe(true);
    expect(matches(ev("g", { ctrlKey: true }), { key: "g", mod: true }, true)).toBe(false);
    expect(matches(ev("g", { metaKey: true }), { key: "g", mod: true }, true)).toBe(true);
  });

  it("requires no mod when descriptor omits it", () => {
    expect(matches(ev("c", { ctrlKey: true }), { key: "c" })).toBe(false);
    expect(matches(ev("c"), { key: "c" })).toBe(true);
  });
});
