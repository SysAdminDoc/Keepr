import { useLayoutEffect, useRef } from "react";

/**
 * NF-20 polish — FLIP (First-Last-Invert-Play) animator. On every render
 * where `key` changes, captures the post-layout bounding rect of each
 * tracked element, compares to the previously-captured rect, and if the
 * element moved applies a one-frame `transform: translate(-dx, -dy)`
 * then clears it inside `requestAnimationFrame` with a 200 ms transition.
 *
 * The container forwards a ref-callback factory that consumers call per
 * row with a stable key — usually the checklist item's sort id. The
 * caller doesn't need to worry about React's reconciler: this hook
 * doesn't unmount/remount anything, it just animates the visual delta
 * after a sort change.
 *
 * No-op for users with `prefers-reduced-motion: reduce`.
 */
export function useFlip<K extends string>(orderKey: string): {
  register: (key: K) => (el: HTMLElement | null) => void;
} {
  const elsRef = useRef<Map<K, HTMLElement>>(new Map());
  const prevRectsRef = useRef<Map<K, DOMRect>>(new Map());

  // Capture rects synchronously after layout, then animate the delta.
  useLayoutEffect(() => {
    const reduce =
      typeof window !== "undefined" &&
      window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
    if (reduce) {
      prevRectsRef.current = captureRects(elsRef.current);
      return;
    }
    const prev = prevRectsRef.current;
    const next = captureRects(elsRef.current);
    for (const [key, el] of elsRef.current) {
      const before = prev.get(key);
      const after = next.get(key);
      if (!before || !after) continue;
      const dx = before.left - after.left;
      const dy = before.top - after.top;
      if (Math.abs(dx) < 0.5 && Math.abs(dy) < 0.5) continue;
      // Invert: jump the element back to its old position with no
      // transition, then on the next frame clear the transform with a
      // transition so it slides to the new position.
      el.style.transition = "none";
      el.style.transform = `translate(${dx}px, ${dy}px)`;
      // Force a layout flush so the no-transition style sticks.
      el.getBoundingClientRect();
      requestAnimationFrame(() => {
        el.style.transition = "transform 200ms ease";
        el.style.transform = "";
        const clear = () => {
          el.style.transition = "";
          el.removeEventListener("transitionend", clear);
        };
        el.addEventListener("transitionend", clear);
      });
    }
    prevRectsRef.current = next;
  }, [orderKey]);

  const register = (key: K) => (el: HTMLElement | null) => {
    if (el) elsRef.current.set(key, el);
    else elsRef.current.delete(key);
  };
  return { register };
}

function captureRects<K>(els: Map<K, HTMLElement>): Map<K, DOMRect> {
  const m = new Map<K, DOMRect>();
  for (const [k, el] of els) m.set(k, el.getBoundingClientRect());
  return m;
}
