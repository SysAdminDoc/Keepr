import { create } from "zustand";
import type { Note, Label, Section, SearchFilters } from "./types";
import { EMPTY_FILTERS } from "./types";
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

export type ThemeMode = "light" | "dark" | "system";
export type ViewMode = "grid" | "list";

interface UIState {
  notes: Note[];
  labels: Label[];
  loaded: boolean;
  section: Section;
  search: string;
  /** NF-09 facet filters applied alongside text search. */
  filters: SearchFilters;
  /** Resolved theme — true when the effective theme is dark. */
  dark: boolean;
  /** User preference. `system` follows prefers-color-scheme. */
  themeMode: ThemeMode;
  viewMode: ViewMode;
  /** Days a note stays in Trash before being auto-purged. 0 = never (NF-17). */
  trashRetentionDays: number;
  /** When true, ticking a checklist item moves it into the "Checked items"
   *  group at the bottom of the editor (NF-20). Default ON, matches Keep. */
  moveCheckedToBottom: boolean;
  editorOpen: boolean;
  editorNoteId: string | null;
  settingsOpen: boolean;
  labelsManagerOpen: boolean;
  toasts: Toast[];
  /** Set of note IDs the user has currently multi-selected (NF-04). */
  selectedIds: Set<string>;
  load: () => Promise<void>;
  setSection: (s: Section) => void;
  setSearch: (q: string) => void;
  setFilters: (f: SearchFilters) => void;
  clearFilters: () => void;
  /** Cycle through Light → Dark → System (legacy `toggleDark` retained). */
  toggleDark: () => void;
  setThemeMode: (mode: ThemeMode) => void;
  setViewMode: (mode: ViewMode) => void;
  toggleViewMode: () => void;
  setTrashRetentionDays: (days: number) => void;
  setMoveCheckedToBottom: (enabled: boolean) => void;
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

  toggleSelected: (id: string) => void;
  setSelected: (ids: string[]) => void;
  clearSelection: () => void;
  /** @internal — used by the system-theme matchMedia listener. */
  _setDarkFromSystem: (dark: boolean) => void;
}

let toastSeq = 0;
function nextToastId(): number {
  return ++toastSeq;
}

const THEME_KEY = "keepr:theme"; // values: "light" | "dark" | "system" | (legacy: undefined)
const VIEW_MODE_KEY = "keepr:view-mode"; // values: "grid" | "list"
const TRASH_RETENTION_KEY = "keepr:trash-retention-days"; // integer
const MOVE_CHECKED_KEY = "keepr:move-checked-to-bottom"; // "true" | "false"

const DEFAULT_TRASH_RETENTION_DAYS = 7;
const DEFAULT_MOVE_CHECKED_TO_BOTTOM = true;

// EI-37 — the inline boot script in `index.html` toggles the `.dark` class on
// <html> BEFORE the first React paint, so there's no flash of wrong theme.
// Here we just read the resulting class so the store's `dark` value matches.
function readInitialDark(): boolean {
  if (typeof document === "undefined") return false;
  return document.documentElement.classList.contains("dark");
}

function readInitialThemeMode(): ThemeMode {
  if (typeof localStorage === "undefined") return "system";
  const stored = localStorage.getItem(THEME_KEY);
  if (stored === "light" || stored === "dark" || stored === "system") return stored;
  // No stored preference → System default (NF-16). Migrates v0.2 users who
  // had no key set (we previously inferred from prefers-color-scheme).
  return "system";
}

function readInitialViewMode(): ViewMode {
  if (typeof localStorage === "undefined") return "grid";
  return localStorage.getItem(VIEW_MODE_KEY) === "list" ? "list" : "grid";
}

function readInitialTrashRetentionDays(): number {
  if (typeof localStorage === "undefined") return DEFAULT_TRASH_RETENTION_DAYS;
  const raw = localStorage.getItem(TRASH_RETENTION_KEY);
  if (raw == null) return DEFAULT_TRASH_RETENTION_DAYS;
  const n = parseInt(raw, 10);
  if (Number.isNaN(n) || n < 0 || n > 3650) return DEFAULT_TRASH_RETENTION_DAYS;
  return n;
}

function readInitialMoveCheckedToBottom(): boolean {
  if (typeof localStorage === "undefined") return DEFAULT_MOVE_CHECKED_TO_BOTTOM;
  const raw = localStorage.getItem(MOVE_CHECKED_KEY);
  if (raw === "true") return true;
  if (raw === "false") return false;
  return DEFAULT_MOVE_CHECKED_TO_BOTTOM;
}

function effectiveDark(mode: ThemeMode): boolean {
  if (mode === "dark") return true;
  if (mode === "light") return false;
  if (typeof window === "undefined" || !window.matchMedia) return false;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

function applyDarkClass(dark: boolean) {
  if (typeof document === "undefined") return;
  document.documentElement.classList.toggle("dark", dark);
}

// Forward declaration so the matchMedia listener below can reach the store
// without creating an import cycle. Assigned at the bottom of this module.
let useStoreRef: typeof useStore | null = null;

// Watch system theme so a `themeMode === "system"` user follows OS-level
// changes live. Registered once on module load.
if (typeof window !== "undefined" && window.matchMedia) {
  const mql = window.matchMedia("(prefers-color-scheme: dark)");
  const onSystemChange = () => {
    const s = useStoreRef?.getState?.();
    if (s && s.themeMode === "system") {
      const dark = effectiveDark("system");
      applyDarkClass(dark);
      s._setDarkFromSystem(dark);
    }
  };
  mql.addEventListener?.("change", onSystemChange);
}

export const useStore = create<UIState>((set, get) => ({
  notes: [],
  labels: [],
  loaded: false,
  section: { kind: "notes" },
  search: "",
  filters: EMPTY_FILTERS,
  dark: readInitialDark(),
  editorOpen: false,
  editorNoteId: null,
  settingsOpen: false,
  labelsManagerOpen: false,
  themeMode: readInitialThemeMode(),
  viewMode: readInitialViewMode(),
  trashRetentionDays: readInitialTrashRetentionDays(),
  moveCheckedToBottom: readInitialMoveCheckedToBottom(),
  toasts: [],
  selectedIds: new Set(),
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
  setFilters: (f) => set({ filters: f }),
  clearFilters: () => set({ filters: EMPTY_FILTERS }),
  toggleDark: () => {
    // Two-state toggle off the resolved `dark` value. Sets an explicit
    // light/dark preference (loses the "system" choice — use setThemeMode
    // from Settings if you want to go back).
    const next = !get().dark;
    get().setThemeMode(next ? "dark" : "light");
  },
  setThemeMode: (mode) => {
    localStorage.setItem(THEME_KEY, mode);
    const dark = effectiveDark(mode);
    applyDarkClass(dark);
    set({ themeMode: mode, dark });
  },
  setViewMode: (mode) => {
    localStorage.setItem(VIEW_MODE_KEY, mode);
    set({ viewMode: mode });
  },
  toggleViewMode: () => {
    const next: ViewMode = get().viewMode === "grid" ? "list" : "grid";
    get().setViewMode(next);
  },
  setTrashRetentionDays: (days) => {
    const clamped = Math.max(0, Math.min(3650, Math.round(days)));
    localStorage.setItem(TRASH_RETENTION_KEY, String(clamped));
    set({ trashRetentionDays: clamped });
  },
  setMoveCheckedToBottom: (enabled) => {
    localStorage.setItem(MOVE_CHECKED_KEY, enabled ? "true" : "false");
    set({ moveCheckedToBottom: enabled });
  },
  _setDarkFromSystem: (dark) => set({ dark }),
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

  toggleSelected: (id) =>
    set((s) => {
      const next = new Set(s.selectedIds);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return { selectedIds: next };
    }),
  setSelected: (ids) => set({ selectedIds: new Set(ids) }),
  clearSelection: () => set({ selectedIds: new Set() }),
}));

// Wire the forward-declared ref now that useStore exists. The
// system-theme matchMedia listener above uses this to reach the store
// without creating an import cycle.
useStoreRef = useStore;

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
