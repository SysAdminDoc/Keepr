import { useEffect, useRef } from "react";

/**
 * NF-V0.5-C App Lock — call the supplied `onIdle` callback after
 * `idleMinutes` of no user activity. Activity is anything the user
 * could plausibly do at the window: mousedown, keydown, touchstart,
 * pointerdown, scroll, plus a `visibilitychange` rearm so the timer
 * keeps running while the window is in the background (and trips when
 * the user comes back).
 *
 * The timer is rearmed on each activity event, so a user typing
 * continuously never trips the lock. Pass `active = false` to disable
 * the timer entirely (e.g. when App Lock is not configured).
 */
export function useIdleLock(
  idleMinutes: number,
  onIdle: () => void,
  active: boolean,
): void {
  const timerRef = useRef<number | null>(null);
  const onIdleRef = useRef(onIdle);
  onIdleRef.current = onIdle;

  useEffect(() => {
    if (!active) return;
    if (typeof window === "undefined") return;
    const ms = Math.max(1, idleMinutes) * 60 * 1000;
    const arm = () => {
      if (timerRef.current !== null) window.clearTimeout(timerRef.current);
      timerRef.current = window.setTimeout(() => {
        onIdleRef.current();
      }, ms);
    };
    const events: (keyof DocumentEventMap)[] = [
      "mousedown",
      "keydown",
      "touchstart",
      "pointerdown",
      "scroll",
      "visibilitychange",
    ];
    const handler = () => arm();
    for (const e of events) document.addEventListener(e, handler, { passive: true });
    arm();
    return () => {
      if (timerRef.current !== null) window.clearTimeout(timerRef.current);
      for (const e of events) document.removeEventListener(e, handler);
    };
  }, [active, idleMinutes]);
}
