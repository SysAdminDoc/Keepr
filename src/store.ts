import { create } from "zustand";
import type { Note, Label, Section } from "./types";
import { api } from "./api";

export interface ToastAction {
  label: string;
  onClick: () => void | Promise<void>;
}

export interface Toast {
  id: number;
  text: string;
  action?: ToastAction;
  durationMs: number;
}

interface UIState {
  notes: Note[];
  labels: Label[];
  loaded: boolean;
  section: Section;
  search: string;
  dark: boolean;
  editorOpen: boolean;
  editorNoteId: string | null;
  settingsOpen: boolean;
  labelsManagerOpen: boolean;
  toasts: Toast[];
  load: () => Promise<void>;
  setSection: (s: Section) => void;
  setSearch: (q: string) => void;
  toggleDark: () => void;
  openEditor: (id: string | null) => void;
  closeEditor: () => void;
  openSettings: () => void;
  closeSettings: () => void;
  openLabelsManager: () => void;
  closeLabelsManager: () => void;
  /** Backwards-compatible signature. Pass an action to surface an Undo
   *  button (5s default if action present, 2.5s default otherwise). */
  showToast: (text: string, opts?: { action?: ToastAction; durationMs?: number }) => void;
  dismissToast: (id: number) => void;
}

let toastSeq = 0;
function nextToastId(): number {
  return ++toastSeq;
}

const THEME_KEY = "keepr:theme";

// EI-37 — the inline boot script in `index.html` toggles the `.dark` class on
// <html> BEFORE the first React paint, so there's no flash of wrong theme.
// Here we just read the resulting class so the store's `dark` value matches.
function readInitialDark(): boolean {
  if (typeof document === "undefined") return false;
  return document.documentElement.classList.contains("dark");
}

export const useStore = create<UIState>((set, get) => ({
  notes: [],
  labels: [],
  loaded: false,
  section: { kind: "notes" },
  search: "",
  dark: readInitialDark(),
  editorOpen: false,
  editorNoteId: null,
  settingsOpen: false,
  labelsManagerOpen: false,
  toasts: [],
  load: async () => {
    try {
      const [notes, labels] = await Promise.all([
        api.listNotes(),
        api.listLabels(),
      ]);
      set({ notes, labels, loaded: true });
    } catch (e) {
      // Even on failure we mark loaded so the user sees an error toast,
      // not an infinite spinner.
      set({ loaded: true });
      get().showToast("Could not load notes: " + String(e));
    }
  },
  // EI-40 — don't wipe the search input when the user switches sections.
  setSection: (s) => set({ section: s }),
  setSearch: (q) => set({ search: q }),
  toggleDark: () => {
    const next = !get().dark;
    if (next) {
      document.documentElement.classList.add("dark");
      localStorage.setItem(THEME_KEY, "dark");
    } else {
      document.documentElement.classList.remove("dark");
      localStorage.setItem(THEME_KEY, "light");
    }
    set({ dark: next });
  },
  openEditor: (id) => set({ editorOpen: true, editorNoteId: id }),
  closeEditor: () => set({ editorOpen: false, editorNoteId: null }),
  openSettings: () => set({ settingsOpen: true }),
  closeSettings: () => set({ settingsOpen: false }),
  openLabelsManager: () => set({ labelsManagerOpen: true }),
  closeLabelsManager: () => set({ labelsManagerOpen: false }),
  showToast: (text, opts) => {
    const id = nextToastId();
    const durationMs =
      opts?.durationMs ?? (opts?.action ? 5000 : 2500);
    const toast: Toast = { id, text, action: opts?.action, durationMs };
    set((s) => ({ toasts: [...s.toasts, toast] }));
    if (durationMs > 0) {
      setTimeout(() => {
        set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }));
      }, durationMs);
    }
  },
  dismissToast: (id) => {
    set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }));
  },
}));
