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

  // EI-24 — optimistic in-place reducers. Call site flow is:
  //   await api.foo(...);            // command returns the new Note (or void)
  //   useStore.getState().patchNote(id, { pinned: true });
  // No full load() round-trip per mutation. Sorting is recomputed below so
  // pin/unpin still moves the card into the right section.
  upsertNote: (note: Note) => void;
  patchNote: (id: string, patch: Partial<Note>) => void;
  removeNote: (id: string) => void;
  upsertLabel: (label: Label) => void;
  patchLabel: (id: string, patch: Partial<Label>) => void;
  removeLabel: (id: string) => void;
  /** Drop every note whose predicate matches (used by Empty Trash). */
  removeNotesWhere: (predicate: (n: Note) => boolean) => void;
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

  upsertNote: (note) =>
    set((s) => {
      const i = s.notes.findIndex((n) => n.id === note.id);
      const next = [...s.notes];
      if (i >= 0) next[i] = note;
      else next.unshift(note);
      return { notes: sortNotes(next) };
    }),
  patchNote: (id, patch) =>
    set((s) => ({
      notes: sortNotes(
        s.notes.map((n) => (n.id === id ? { ...n, ...patch } : n)),
      ),
    })),
  removeNote: (id) =>
    set((s) => ({ notes: s.notes.filter((n) => n.id !== id) })),
  removeNotesWhere: (predicate) =>
    set((s) => ({ notes: s.notes.filter((n) => !predicate(n)) })),
  upsertLabel: (label) =>
    set((s) => {
      const i = s.labels.findIndex((l) => l.id === label.id);
      const next = [...s.labels];
      if (i >= 0) next[i] = label;
      else next.push(label);
      return { labels: sortLabels(next) };
    }),
  patchLabel: (id, patch) =>
    set((s) => ({
      labels: sortLabels(
        s.labels.map((l) => (l.id === id ? { ...l, ...patch } : l)),
      ),
    })),
  removeLabel: (id) =>
    set((s) => ({
      labels: s.labels.filter((l) => l.id !== id),
      // Also strip the deleted label from any note that referenced it.
      notes: s.notes.map((n) => ({
        ...n,
        labels: n.labels.filter((lid) => lid !== id),
      })),
    })),
}));

// --- sort helpers (mirror Rust's list_notes ORDER BY) ---

function sortNotes(notes: Note[]): Note[] {
  // Sort by pinned DESC, then updated_at DESC. Match SQL's behavior so the
  // grid stays in sync without a re-fetch.
  return [...notes].sort((a, b) => {
    if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
    return a.updated_at < b.updated_at ? 1 : a.updated_at > b.updated_at ? -1 : 0;
  });
}

function sortLabels(labels: Label[]): Label[] {
  return [...labels].sort((a, b) =>
    a.name.localeCompare(b.name, undefined, { sensitivity: "base" }),
  );
}
