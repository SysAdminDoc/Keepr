import { useEffect } from "react";

/**
 * Run `onEscape` whenever Escape is pressed and `active` is true. The handler
 * stops propagation so an outer Escape handler (e.g. modal stacked on modal)
 * doesn't also fire.
 */
export function useEscape(active: boolean, onEscape: () => void): void {
  useEffect(() => {
    if (!active) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onEscape();
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [active, onEscape]);
}
