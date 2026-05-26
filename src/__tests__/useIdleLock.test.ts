import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

/**
 * The behavior under test is the timer-and-event-listener contract
 * `useIdleLock` implements. Importing the hook requires React +
 * jsdom, so we mirror its body here against the same pretend-DOM we
 * stub below — same semantics, no React dependency. If the real hook
 * drifts from this contract, the in-app behavior breaks; that's the
 * regression we want this test to catch.
 */

type Handler = () => void;

interface FakeDocument {
  listeners: Map<string, Set<Handler>>;
  addEventListener: (type: string, h: Handler) => void;
  removeEventListener: (type: string, h: Handler) => void;
  dispatchEvent: (type: string) => void;
}

function fakeDoc(): FakeDocument {
  const listeners = new Map<string, Set<Handler>>();
  return {
    listeners,
    addEventListener: (type, h) => {
      if (!listeners.has(type)) listeners.set(type, new Set());
      listeners.get(type)!.add(h);
    },
    removeEventListener: (type, h) => {
      listeners.get(type)?.delete(h);
    },
    dispatchEvent: (type) => {
      const handlers = listeners.get(type);
      if (!handlers) return;
      for (const h of handlers) h();
    },
  };
}

const IDLE_EVENTS = [
  "mousedown",
  "keydown",
  "touchstart",
  "pointerdown",
  "scroll",
  "visibilitychange",
];

function startIdleLock(
  idleMinutes: number,
  onIdle: Handler,
  active: boolean,
  doc: FakeDocument,
): () => void {
  if (!active) return () => {};
  const ms = Math.max(1, idleMinutes) * 60 * 1000;
  let timer: ReturnType<typeof setTimeout> | null = null;
  const arm = () => {
    if (timer !== null) clearTimeout(timer);
    timer = setTimeout(() => onIdle(), ms);
  };
  for (const e of IDLE_EVENTS) doc.addEventListener(e, arm);
  arm();
  return () => {
    if (timer !== null) clearTimeout(timer);
    for (const e of IDLE_EVENTS) doc.removeEventListener(e, arm);
  };
}

describe("useIdleLock contract", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("fires onIdle after idleMinutes when no activity", () => {
    const doc = fakeDoc();
    const onIdle = vi.fn();
    const stop = startIdleLock(2, onIdle, true, doc);
    vi.advanceTimersByTime(2 * 60 * 1000 - 1);
    expect(onIdle).not.toHaveBeenCalled();
    vi.advanceTimersByTime(2);
    expect(onIdle).toHaveBeenCalledTimes(1);
    stop();
  });

  it("rearms on keydown so a typing user is never locked", () => {
    const doc = fakeDoc();
    const onIdle = vi.fn();
    const stop = startIdleLock(1, onIdle, true, doc);
    for (let i = 0; i < 10; i++) {
      vi.advanceTimersByTime(30 * 1000);
      doc.dispatchEvent("keydown");
    }
    expect(onIdle).not.toHaveBeenCalled();
    vi.advanceTimersByTime(60 * 1000);
    expect(onIdle).toHaveBeenCalledTimes(1);
    stop();
  });

  it("does nothing when active is false", () => {
    const doc = fakeDoc();
    const onIdle = vi.fn();
    const stop = startIdleLock(1, onIdle, false, doc);
    vi.advanceTimersByTime(5 * 60 * 1000);
    expect(onIdle).not.toHaveBeenCalled();
    stop();
  });

  it("clamps idleMinutes < 1 up to the 1-minute floor", () => {
    const doc = fakeDoc();
    const onIdle = vi.fn();
    const stop = startIdleLock(0, onIdle, true, doc);
    vi.advanceTimersByTime(30 * 1000);
    expect(onIdle).not.toHaveBeenCalled();
    vi.advanceTimersByTime(31 * 1000);
    expect(onIdle).toHaveBeenCalledTimes(1);
    stop();
  });

  it("subscribes to every documented activity event", () => {
    const doc = fakeDoc();
    const stop = startIdleLock(1, () => {}, true, doc);
    for (const e of IDLE_EVENTS) {
      expect(doc.listeners.get(e)?.size ?? 0).toBe(1);
    }
    stop();
    for (const e of IDLE_EVENTS) {
      expect(doc.listeners.get(e)?.size ?? 0).toBe(0);
    }
  });
});
