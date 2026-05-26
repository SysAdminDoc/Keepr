import { useStore } from "../store";
import { api } from "../api";
import { useGlobalHotkey } from "./useGlobalHotkey";

/**
 * Bind the Keep-canonical app-level shortcuts. Mounted once at App level.
 * Card-focus shortcuts (f / e / #) read the currently focused .note-card
 * via document.activeElement.dataset.noteId, so the renderer doesn't have
 * to centralize a "selected card" state.
 *
 * Suppressed inside text inputs (the useGlobalHotkey default), so typing
 * "/" or "c" in the editor still works as a literal character.
 */
export function useKeepShortcuts(openHelp: () => void): void {
  const openEditor = useStore((s) => s.openEditor);
  const showToast = useStore((s) => s.showToast);
  const patchNote = useStore((s) => s.patchNote);

  // Application shortcuts.
  useGlobalHotkey({ key: "c" }, () => openEditor(null));
  useGlobalHotkey({ key: "l" }, () => openEditor(null));
  useGlobalHotkey({ key: "/" }, () => {
    const input = document.querySelector<HTMLInputElement>(
      'input[type="search"]',
    );
    input?.focus();
    input?.select();
  });
  useGlobalHotkey({ key: "?" }, openHelp);

  // j / k navigation between cards.
  useGlobalHotkey({ key: "j" }, () => moveCardFocus(1));
  useGlobalHotkey({ key: "k" }, () => moveCardFocus(-1));

  // Per-card actions — operate on the card that currently has focus.
  useGlobalHotkey({ key: "f" }, async () => {
    const id = focusedCardNoteId();
    if (!id) return;
    const note = useStore.getState().notes.find((n) => n.id === id);
    if (!note) return;
    try {
      await api.setPinned(id, !note.pinned);
      patchNote(id, {
        pinned: !note.pinned,
        archived: false,
        updated_at: new Date().toISOString(),
      });
    } catch (e) {
      showToast("Could not pin: " + String(e));
    }
  });
  useGlobalHotkey({ key: "e" }, async () => {
    const id = focusedCardNoteId();
    if (!id) return;
    const note = useStore.getState().notes.find((n) => n.id === id);
    if (!note) return;
    try {
      const next = !note.archived;
      await api.setArchived(id, next);
      patchNote(id, {
        archived: next,
        trashed: false,
        trashed_at: null,
        updated_at: new Date().toISOString(),
      });
      showToast(next ? "Note archived" : "Note unarchived");
    } catch (e) {
      showToast("Could not archive: " + String(e));
    }
  });
  useGlobalHotkey({ key: "#", shift: true }, async () => {
    const id = focusedCardNoteId();
    if (!id) return;
    try {
      const now = new Date().toISOString();
      await api.setTrashed(id, true);
      patchNote(id, {
        trashed: true,
        archived: false,
        pinned: false,
        trashed_at: now,
        updated_at: now,
      });
      showToast("Note moved to Trash");
    } catch (e) {
      showToast("Could not trash: " + String(e));
    }
  });
}

function cardElements(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>(".note-card"));
}

function focusedCardNoteId(): string | null {
  const el = document.activeElement;
  if (!(el instanceof HTMLElement)) return null;
  const card = el.closest<HTMLElement>(".note-card");
  return card?.dataset.noteId ?? null;
}

function moveCardFocus(delta: number): void {
  const cards = cardElements();
  if (cards.length === 0) return;
  const active = document.activeElement;
  const i = active instanceof HTMLElement ? cards.indexOf(active.closest<HTMLElement>(".note-card")!) : -1;
  const next =
    i < 0
      ? delta > 0
        ? cards[0]
        : cards[cards.length - 1]
      : cards[(i + delta + cards.length) % cards.length];
  next?.focus();
}
