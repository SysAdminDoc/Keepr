import { create } from "zustand";
import type { Note, Label, Section } from "./types";
import { api } from "./api";

interface UIState {
  notes: Note[];
  labels: Label[];
  section: Section;
  search: string;
  dark: boolean;
  editorOpen: boolean;
  editorNoteId: string | null;
  settingsOpen: boolean;
  labelsManagerOpen: boolean;
  toast: string | null;
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
  showToast: (msg: string) => void;
}

const THEME_KEY = "keepr:theme";

const initialDark =
  typeof window !== "undefined" &&
  (localStorage.getItem(THEME_KEY) === "dark" ||
    (!localStorage.getItem(THEME_KEY) &&
      window.matchMedia?.("(prefers-color-scheme: dark)").matches));

if (initialDark) document.documentElement.classList.add("dark");

export const useStore = create<UIState>((set, get) => ({
  notes: [],
  labels: [],
  section: { kind: "notes" },
  search: "",
  dark: !!initialDark,
  editorOpen: false,
  editorNoteId: null,
  settingsOpen: false,
  labelsManagerOpen: false,
  toast: null,
  load: async () => {
    const [notes, labels] = await Promise.all([api.listNotes(), api.listLabels()]);
    set({ notes, labels });
  },
  setSection: (s) => set({ section: s, search: "" }),
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
  showToast: (msg) => {
    set({ toast: msg });
    setTimeout(() => {
      if (get().toast === msg) set({ toast: null });
    }, 2500);
  },
}));
