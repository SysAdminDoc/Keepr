import { useEffect, type RefObject } from "react";

/**
 * Call `onOutside` when a mousedown happens outside the given element. Used
 * by popovers (color picker, label menu) to dismiss on click-away (EI-19).
 */
export function useClickOutside(
  ref: RefObject<HTMLElement | null>,
  active: boolean,
  onOutside: () => void,
): void {
  useEffect(() => {
    if (!active) return;
    const handler = (e: MouseEvent) => {
      const target = e.target as Node | null;
      if (!target) return;
      if (ref.current && !ref.current.contains(target)) {
        onOutside();
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [ref, active, onOutside]);
}
