import { useEffect } from "react";

/**
 * Hotkey descriptor. `key` matches `KeyboardEvent.key` (case-insensitive).
 * For Cmd-on-mac / Ctrl-on-Windows, set `mod: true` (uses `metaKey` on
 * macOS, `ctrlKey` elsewhere).
 */
export interface Hotkey {
  key: string;
  mod?: boolean;
  shift?: boolean;
  alt?: boolean;
  /** Run even when focus is inside an input / textarea / contenteditable.
   *  Defaults to false so naked-letter shortcuts (`c`, `/`, etc.) don't
   *  hijack text input. */
  whenTyping?: boolean;
}

const isMac =
  typeof navigator !== "undefined" &&
  /Mac|iPhone|iPad/.test(navigator.platform);

function isEditable(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return true;
  if (target.isContentEditable) return true;
  return false;
}

function matches(e: KeyboardEvent, hk: Hotkey): boolean {
  if (e.key.toLowerCase() !== hk.key.toLowerCase()) return false;
  const mod = isMac ? e.metaKey : e.ctrlKey;
  if (Boolean(hk.mod) !== mod) return false;
  // For keys that are inherently shifted (like "?" or "#"), the user's
  // shift state matches the typed character — we don't need to require
  // an explicit `shift: true` in the descriptor. Only when the descriptor
  // *requires* a shift modifier on an unshifted key (e.g. Shift+J) do we
  // enforce it.
  if (hk.shift !== undefined && Boolean(hk.shift) !== e.shiftKey) return false;
  if (Boolean(hk.alt) !== e.altKey) return false;
  return true;
}

/**
 * Register a document-level keydown handler that fires `onTrigger` when
 * the event matches `hotkey`. Listener is removed on unmount. Multiple
 * useGlobalHotkey calls coexist via independent listeners.
 *
 * Shortcuts are suppressed when focus is in a text input unless you set
 * `whenTyping: true` (useful for editor-only Ctrl+Enter etc.).
 */
export function useGlobalHotkey(
  hotkey: Hotkey,
  onTrigger: (e: KeyboardEvent) => void,
  active = true,
): void {
  useEffect(() => {
    if (!active) return;
    const handler = (e: KeyboardEvent) => {
      if (!hotkey.whenTyping && isEditable(e.target)) return;
      if (matches(e, hotkey)) {
        e.preventDefault();
        onTrigger(e);
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
    // Hotkey identity is by value; consumers usually pass a literal.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    active,
    hotkey.key,
    hotkey.mod,
    hotkey.shift,
    hotkey.alt,
    hotkey.whenTyping,
    onTrigger,
  ]);
}
