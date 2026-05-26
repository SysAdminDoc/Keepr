import { create } from "zustand";
import type { Note, Label, Section, SearchFilters, Reminder } from "./types";
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
export type AutoBackupCadence = "off" | "daily" | "weekly";
export type SortMode = "modified" | "created" | "title" | "custom";

interface UIState {
  notes: Note[];
  labels: Label[];
  reminders: Reminder[];
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
  /** How the masonry grid orders unpinned (and pinned) notes (NF-05). */
  sortMode: SortMode;
  /** Days a note stays in Trash before being auto-purged. 0 = never (NF-17). */
  trashRetentionDays: number;
  /** When true, ticking a checklist item moves it into the "Checked items"
   *  group at the bottom of the editor (NF-20). Default ON, matches Keep. */
  moveCheckedToBottom: boolean;
  /** Auto-backup cadence (NF-15). "off" = no schedule; the manual
   *  Export button in Settings still works regardless. */
  autoBackupCadence: AutoBackupCadence;
  /** Folder Keepr writes auto-backups into. Picked once via Settings;
   *  point it at your Google Drive / OneDrive sync folder for cloud
   *  backups with no built-in sync code. */
  autoBackupFolder: string | null;
  /** ISO timestamp of the last auto-backup we wrote. */
  autoBackupLastAt: string | null;
  editorOpen: boolean;
  editorNoteId: string | null;
  settingsOpen: boolean;
  labelsManagerOpen: boolean;
  toasts: Toast[];
  /** Set of note IDs the user has currently multi-selected (NF-04). */
  selectedIds: Set<string>;
  /** NF-V0.5-C — when true, the App Lock overlay is shown and the rest
   *  of the UI is hidden. Computed from `appLockEnabled` at startup
   *  (locked-on-launch) and the idle timer (locked-after-N-minutes). */
  locked: boolean;
  /** Whether the user has set a PIN. Mirrored from Rust on startup. */
  appLockEnabled: boolean;
  /** Idle minutes before the UI auto-locks. */
  lockAfterMinutes: number;
  /** EI-18 — note IDs the FTS5 backend matched for the current search.
   *  `null` means "no FTS5 result available" (empty query, or running
   *  outside Tauri); in that case `filterNotes` falls back to the
   *  in-memory substring scan. A `Set` means "narrow to these IDs". */
  searchMatchIds: Set<string> | null;
  setSearchMatchIds: (ids: Set<string> | null) => void;
  /** NF-V0.5-C — Private Vault state. `initialized` = a vault DEK is
   *  wrapped on disk; `unlocked` = the renderer has unlocked it via
   *  unlock_vault. When unlocked = false, every vault note arrives with
   *  empty title/body/checklist and the UI shows a locked placeholder. */
  vaultInitialized: boolean;
  vaultUnlocked: boolean;
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
  setSortMode: (mode: SortMode) => void;
  setTrashRetentionDays: (days: number) => void;
  setMoveCheckedToBottom: (enabled: boolean) => void;
  setAutoBackupCadence: (c: AutoBackupCadence) => void;
  setAutoBackupFolder: (folder: string | null) => void;
  setAutoBackupLastAt: (iso: string | null) => void;
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
  upsertReminder: (reminder: Reminder) => void;
  removeReminder: (noteId: string) => void;
  /** Drop every note whose predicate matches (used by Empty Trash). */
  removeNotesWhere: (predicate: (n: Note) => boolean) => void;

  toggleSelected: (id: string) => void;
  setSelected: (ids: string[]) => void;
  clearSelection: () => void;
  /** NF-V0.5-C — load lock config from Rust then auto-lock if enabled. */
  initAppLock: () => Promise<void>;
  /** Set by the LockScreen after a successful PIN verify. */
  unlock: () => void;
  /** Set by the idle hook after the lock-after-N-minutes timeout. */
  lock: () => void;
  /** After a settings change (enable/disable/change minutes). */
  refreshAppLockState: () => Promise<void>;
  /** Reload vault status from Rust + re-fetch notes (so vaulted rows
   *  flip between locked-placeholder and decrypted). */
  refreshVaultState: () => Promise<void>;
  /** @internal — used by the system-theme matchMedia listener. */
  _setDarkFromSystem: (dark: boolean) => void;
}

let toastSeq = 0;
function nextToastId(): number {
  return ++toastSeq;
}

const THEME_KEY = "keepr:theme"; // values: "light" | "dark" | "system" | (legacy: undefined)
const VIEW_MODE_KEY = "keepr:view-mode"; // values: "grid" | "list"
const SORT_MODE_KEY = "keepr:sort-mode"; // values: "modified"|"created"|"title"|"custom"
const TRASH_RETENTION_KEY = "keepr:trash-retention-days"; // integer
const MOVE_CHECKED_KEY = "keepr:move-checked-to-bottom"; // "true" | "false"
const AUTOBACKUP_CADENCE_KEY = "keepr:autobackup-cadence"; // "off"|"daily"|"weekly"
const AUTOBACKUP_FOLDER_KEY = "keepr:autobackup-folder"; // absolute path
const AUTOBACKUP_LAST_KEY = "keepr:autobackup-last-at"; // ISO

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

function readInitialSortMode(): SortMode {
  if (typeof localStorage === "undefined") return "modified";
  const raw = localStorage.getItem(SORT_MODE_KEY);
  if (raw === "modified" || raw === "created" || raw === "title" || raw === "custom") {
    return raw;
  }
  return "modified";
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

function readInitialAutoBackupCadence(): AutoBackupCadence {
  if (typeof localStorage === "undefined") return "off";
  const raw = localStorage.getItem(AUTOBACKUP_CADENCE_KEY);
  return raw === "daily" || raw === "weekly" ? raw : "off";
}

function readInitialAutoBackupFolder(): string | null {
  if (typeof localStorage === "undefined") return null;
  const raw = localStorage.getItem(AUTOBACKUP_FOLDER_KEY);
  return raw && raw.length > 0 ? raw : null;
}

function readInitialAutoBackupLastAt(): string | null {
  if (typeof localStorage === "undefined") return null;
  return localStorage.getItem(AUTOBACKUP_LAST_KEY);
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
  reminders: [],
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
  sortMode: readInitialSortMode(),
  trashRetentionDays: readInitialTrashRetentionDays(),
  moveCheckedToBottom: readInitialMoveCheckedToBottom(),
  autoBackupCadence: readInitialAutoBackupCadence(),
  autoBackupFolder: readInitialAutoBackupFolder(),
  autoBackupLastAt: readInitialAutoBackupLastAt(),
  toasts: [],
  selectedIds: new Set(),
  locked: false,
  appLockEnabled: false,
  lockAfterMinutes: 5,
  searchMatchIds: null,
  setSearchMatchIds: (ids) => set({ searchMatchIds: ids }),
  vaultInitialized: false,
  vaultUnlocked: false,
  load: async () => {
    try {
      const [notes, labels, reminders] = await Promise.all([
        api.listNotes(),
        api.listLabels(),
        api.listReminders().catch(() => []),
      ]);
      // Apply the user's preferred sort over the SQL-default order.
      set((s) => ({
        notes: sortNotes(notes, s.sortMode),
        labels,
        reminders,
        loaded: true,
      }));
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
  setSortMode: (mode) => {
    localStorage.setItem(SORT_MODE_KEY, mode);
    set({ sortMode: mode });
    // Re-sort current notes immediately.
    set((s) => ({ notes: sortNotes(s.notes, mode) }));
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
  setAutoBackupCadence: (cadence) => {
    localStorage.setItem(AUTOBACKUP_CADENCE_KEY, cadence);
    set({ autoBackupCadence: cadence });
  },
  setAutoBackupFolder: (folder) => {
    if (folder) localStorage.setItem(AUTOBACKUP_FOLDER_KEY, folder);
    else localStorage.removeItem(AUTOBACKUP_FOLDER_KEY);
    set({ autoBackupFolder: folder });
  },
  setAutoBackupLastAt: (iso) => {
    if (iso) localStorage.setItem(AUTOBACKUP_LAST_KEY, iso);
    else localStorage.removeItem(AUTOBACKUP_LAST_KEY);
    set({ autoBackupLastAt: iso });
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
      return { notes: sortNotes(next, s.sortMode) };
    }),
  patchNote: (id, patch) =>
    set((s) => {
      const next = s.notes.map((n) => (n.id === id ? { ...n, ...patch } : n));
      // EI-V0.5-8 — skip the O(n log n) re-sort when the patch can't
      // possibly affect ordering. Sort keys for each mode:
      //   modified: updated_at
      //   created:  created_at
      //   title:    title
      //   custom:   position
      // Pinned always wins, so any pin change must re-sort.
      const affectsSort =
        "pinned" in patch ||
        "updated_at" in patch ||
        "position" in patch ||
        ("title" in patch && s.sortMode === "title") ||
        ("created_at" in patch && s.sortMode === "created");
      return {
        notes: affectsSort ? sortNotes(next, s.sortMode) : next,
      };
    }),
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
  upsertReminder: (reminder) =>
    set((s) => {
      const i = s.reminders.findIndex((r) => r.noteId === reminder.noteId);
      const next = [...s.reminders];
      if (i >= 0) next[i] = reminder;
      else next.push(reminder);
      return { reminders: next };
    }),
  removeReminder: (noteId) =>
    set((s) => ({ reminders: s.reminders.filter((r) => r.noteId !== noteId) })),

  toggleSelected: (id) =>
    set((s) => {
      const next = new Set(s.selectedIds);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return { selectedIds: next };
    }),
  setSelected: (ids) => set({ selectedIds: new Set(ids) }),
  clearSelection: () => set({ selectedIds: new Set() }),
  initAppLock: async () => {
    try {
      const cfg = await api.getAppLockSettings();
      // Lock at startup if the user enabled App Lock at all — the
      // idle timer takes over once they unlock the first time.
      set({
        appLockEnabled: cfg.enabled,
        lockAfterMinutes: cfg.lockAfterMinutes,
        locked: cfg.enabled,
      });
    } catch {
      // Older binaries without the command — leave defaults.
    }
  },
  unlock: () => set({ locked: false }),
  lock: () => {
    const s = get();
    if (s.appLockEnabled) set({ locked: true });
    // NF-V0.5-C — locking the UI also drops the unlocked vault DEK so
    // an attacker grabbing the laptop while Keepr is locked can't peek
    // at vault notes via the Rust backend's still-resident key.
    if (s.vaultUnlocked) {
      void api.lockVault().then(() => s.refreshVaultState());
    }
  },
  refreshAppLockState: async () => {
    try {
      const cfg = await api.getAppLockSettings();
      set({
        appLockEnabled: cfg.enabled,
        lockAfterMinutes: cfg.lockAfterMinutes,
      });
    } catch {
      /* ignore */
    }
  },
  refreshVaultState: async () => {
    try {
      const status = await api.getVaultStatus();
      set({
        vaultInitialized: status.initialized,
        vaultUnlocked: status.unlocked,
      });
      // Reload notes so vaulted rows flip between locked/unlocked text.
      await get().load();
    } catch {
      /* ignore — older Rust binary may not have the command */
    }
  },
}));

// Wire the forward-declared ref now that useStore exists. The
// system-theme matchMedia listener above uses this to reach the store
// without creating an import cycle.
useStoreRef = useStore;

// --- sort helpers (mirror Rust's list_notes ORDER BY) ---

export function sortNotes(notes: Note[], mode: SortMode = "modified"): Note[] {
  // Pinned-first is universal; the secondary key changes per mode (NF-05).
  const cmp = (a: Note, b: Note): number => {
    if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
    if (mode === "created") {
      return a.created_at < b.created_at ? 1 : a.created_at > b.created_at ? -1 : 0;
    }
    if (mode === "title") {
      return a.title.localeCompare(b.title, undefined, { sensitivity: "base" });
    }
    if (mode === "custom") {
      // Lower position first; ties broken by updated_at DESC for stability.
      if (a.position !== b.position) return a.position - b.position;
      return a.updated_at < b.updated_at ? 1 : a.updated_at > b.updated_at ? -1 : 0;
    }
    // "modified" — default
    return a.updated_at < b.updated_at ? 1 : a.updated_at > b.updated_at ? -1 : 0;
  };
  return [...notes].sort(cmp);
}

function sortLabels(labels: Label[]): Label[] {
  return [...labels].sort((a, b) =>
    a.name.localeCompare(b.name, undefined, { sensitivity: "base" }),
  );
}
