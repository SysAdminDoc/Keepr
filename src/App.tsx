import { lazy, Suspense, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { TopBar } from "./components/TopBar";
import { Sidebar } from "./components/Sidebar";
import { NoteGrid } from "./components/NoteGrid";
import { NewNoteBar } from "./components/NewNoteBar";
import { NoteEditor } from "./components/NoteEditor";
// EI-V0.5-17 — code-split secondary modals. They mount conditionally
// behind `*open` flags so React.lazy is a clean fit; the initial bundle
// no longer includes the LabelsManager / SettingsModal / HelpOverlay
// trees, which only show up the first time the user opens them.
const SettingsModal = lazy(() =>
  import("./components/SettingsModal").then((m) => ({ default: m.SettingsModal })),
);
const LabelsManager = lazy(() =>
  import("./components/LabelsManager").then((m) => ({ default: m.LabelsManager })),
);
const HelpOverlay = lazy(() =>
  import("./components/HelpOverlay").then((m) => ({ default: m.HelpOverlay })),
);
const LockScreen = lazy(() =>
  import("./components/LockScreen").then((m) => ({ default: m.LockScreen })),
);
const CommandPalette = lazy(() =>
  import("./components/CommandPalette").then((m) => ({ default: m.CommandPalette })),
);
import { ConfirmDialog } from "./components/ConfirmDialog";
import { accentHoverFor, useStore } from "./store";
import { api } from "./api";
import { filterNotes } from "./lib/filterNotes";
import { findExpiredTrashed } from "./lib/trashRetention";
import { backupFilename, backupPath, isBackupDue } from "./lib/autoBackup";
import { useGlobalHotkey } from "./hooks/useGlobalHotkey";
import { useKeepShortcuts } from "./hooks/useKeepShortcuts";
import { useIdleLock } from "./hooks/useIdleLock";
import { BulkActionBar } from "./components/BulkActionBar";
import { FilterChips } from "./components/FilterChips";
import { Lightbulb, Archive, Trash2, Tag, Loader2, Bell } from "lucide-react";

export default function App() {
  const notes = useStore((s) => s.notes);
  const labels = useStore((s) => s.labels);
  const section = useStore((s) => s.section);
  const search = useStore((s) => s.search);
  const load = useStore((s) => s.load);
  const toasts = useStore((s) => s.toasts);
  const dismissToast = useStore((s) => s.dismissToast);
  const showToast = useStore((s) => s.showToast);
  const removeNotesWhere = useStore((s) => s.removeNotesWhere);
  const loaded = useStore((s) => s.loaded);

  const toggleViewMode = useStore((s) => s.toggleViewMode);
  const trashRetentionDays = useStore((s) => s.trashRetentionDays);
  const selectedIds = useStore((s) => s.selectedIds);
  const setSelected = useStore((s) => s.setSelected);
  const clearSelection = useStore((s) => s.clearSelection);
  const [sidebarExpanded, setSidebarExpanded] = useState(true);
  const [emptyTrashOpen, setEmptyTrashOpen] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);

  // NF-23 — Ctrl+G toggles between grid and list view.
  useGlobalHotkey({ key: "g", mod: true }, toggleViewMode);

  // NF-06 — open a fresh note when the Rust tray icon / Ctrl+Alt+N global
  // hotkey emits the quick-capture event. NF-02 — re-fetch reminders when
  // the scheduler fires one, so the bell badge drops.
  useEffect(() => {
    const unlisteners: (() => void)[] = [];
    let cancelled = false;
    (async () => {
      try {
        const u1 = await listen("keepr://quick-capture", () => {
          useStore.getState().openEditor(null);
        });
        const u2 = await listen<string>("keepr://reminder-fired", async (e) => {
          const noteId = e.payload;
          try {
            const next = await api.listReminders();
            useStore.setState({ reminders: next });
          } catch {
            /* ignore */
          }
          // In-app toast in addition to the OS notification so the user
          // sees the fire while the window is focused (OS notifications
          // are coalesced or suppressed in that case on some platforms).
          if (typeof noteId === "string" && noteId.length > 0) {
            useStore.getState().showToast("Reminder", {
              durationMs: 8000,
              action: {
                label: "View note",
                onClick: () => useStore.getState().openEditor(noteId),
              },
            });
          }
        });
        // EI-V0.5-7 — surface global-hotkey registration failure (most
        // commonly: another app already owns Ctrl+Alt+N).
        const u3 = await listen<string>("keepr://hotkey-status", (e) => {
          if (e.payload && e.payload !== "ok") {
            useStore.getState().showToast(
              "Ctrl+Alt+N quick-capture is unavailable — another app may already use that shortcut.",
              { durationMs: 8000 },
            );
          }
        });
        if (cancelled) {
          u1();
          u2();
          u3();
        } else {
          unlisteners.push(u1, u2, u3);
        }
      } catch {
        // listen() fails outside Tauri (browser preview, vitest).
      }
    })();
    return () => {
      cancelled = true;
      for (const u of unlisteners) u();
    };
  }, []);
  // NF-03 — bind Keep's canonical shortcuts (c, l, /, ?, j, k, f, e, #).
  useKeepShortcuts(() => setHelpOpen(true));
  // NF-04 — Ctrl+A selects every note visible in the current section/search.
  useGlobalHotkey({ key: "a", mod: true }, () => {
    setSelected(currentFilteredRef.current.map((n) => n.id));
  });
  // NF-04 — Escape clears the selection (in addition to closing modals).
  // EI-V0.5-5 — gate behind no-modal-open so we don't double-fire when
  // the editor / settings / labels / help / confirm modals are already
  // handling Escape themselves. Otherwise Escape inside the editor
  // would close the editor AND clear an unrelated selection.
  useGlobalHotkey({ key: "Escape" }, () => {
    const s = useStore.getState();
    const aModalIsOpen =
      s.editorOpen || s.settingsOpen || s.labelsManagerOpen || helpOpen;
    if (!aModalIsOpen && s.selectedIds.size > 0) clearSelection();
  });

  // NF-17 — sweep expired trashed notes once after the initial load() and
  // again every hour while the app is open. Errors are swallowed; the
  // notes simply stay in Trash and get caught next sweep.
  useEffect(() => {
    if (!loaded) return;
    const sweep = async () => {
      const expired = findExpiredTrashed(
        useStore.getState().notes,
        useStore.getState().trashRetentionDays,
      );
      for (const n of expired) {
        try {
          await api.deleteNotePermanent(n.id);
          useStore.getState().removeNote(n.id);
        } catch {
          /* try again next sweep */
        }
      }
    };
    void sweep();
    const t = window.setInterval(sweep, 60 * 60 * 1000); // hourly
    return () => window.clearInterval(t);
  }, [loaded, trashRetentionDays]);

  // NF-15 — auto-backup cadence. On startup and every 30 min, check if a
  // backup is due (cadence + folder + elapsed time) and run export_zip
  // into the configured folder. A single failure surfaces a toast; the
  // next tick retries.
  //
  // v0.21.0 — after a successful write, prune older auto-backups in the
  // same folder to enforce the retention cap (default 12). Lexical sort
  // of `keepr-autobackup-<ISO>.zip` filenames is equivalent to
  // chronological order, so we keep the latest N regardless of cadence
  // (daily and weekly both append to the same series).
  useEffect(() => {
    if (!loaded) return;
    const tick = async () => {
      const s = useStore.getState();
      if (!isBackupDue(s.autoBackupCadence, s.autoBackupFolder, s.autoBackupLastAt)) {
        return;
      }
      try {
        const dest = backupPath(s.autoBackupFolder!, backupFilename());
        await api.exportZip(dest);
        s.setAutoBackupLastAt(new Date().toISOString());
        try {
          await api.pruneAutoBackups(s.autoBackupFolder!, s.autoBackupRetention);
        } catch (pruneErr) {
          // Rotation failure shouldn't roll back a successful backup.
          console.warn("auto-backup rotation failed:", pruneErr);
        }
      } catch (e) {
        s.showToast("Auto-backup failed: " + String(e));
      }
    };
    void tick();
    const t = window.setInterval(tick, 30 * 60 * 1000); // every 30 min
    return () => window.clearInterval(t);
  }, [loaded]);

  const performEmptyTrash = async () => {
    setEmptyTrashOpen(false);
    try {
      await api.emptyTrash();
      removeNotesWhere((n) => n.trashed);
      showToast("Trash emptied");
    } catch (e) {
      showToast("Could not empty trash: " + String(e));
    }
  };

  useEffect(() => {
    load();
  }, [load]);

  // NF-V0.5-C — load App Lock config and auto-lock on launch if enabled.
  const initAppLock = useStore((s) => s.initAppLock);
  const lock = useStore((s) => s.lock);
  const locked = useStore((s) => s.locked);
  const appLockEnabled = useStore((s) => s.appLockEnabled);
  const lockAfterMinutes = useStore((s) => s.lockAfterMinutes);
  const refreshVaultState = useStore((s) => s.refreshVaultState);
  useEffect(() => {
    initAppLock();
    refreshVaultState();
  }, [initAppLock, refreshVaultState]);
  // Idle timer is only armed while App Lock is configured AND the UI is
  // currently unlocked — locked → already on the overlay, no need to
  // re-fire.
  useIdleLock(lockAfterMinutes, lock, appLockEnabled && !locked);

  // Mirror user-tunable design tokens onto the document root so every
  // CSS rule + every Tailwind arbitrary class that references one of
  // these vars (`var(--keepr-accent)`, `var(--keepr-note-font-size)`,
  // etc.) updates live when the user changes the setting.
  const accentColor = useStore((s) => s.accentColor);
  const noteFontSize = useStore((s) => s.noteFontSize);
  useEffect(() => {
    const root = document.documentElement;
    root.style.setProperty("--keepr-accent", accentColor);
    root.style.setProperty("--keepr-accent-hover", accentHoverFor(accentColor));
    root.style.setProperty("--keepr-note-font-size", `${noteFontSize}px`);
  }, [accentColor, noteFontSize]);

  // Cmd/Ctrl+K opens the command palette. Bound globally so it works
  // from any focused element except text inputs (the input fires the
  // event too — we don't gate, the palette overlays anyway).
  const openCommandPalette = useStore((s) => s.openCommandPalette);
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        openCommandPalette();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [openCommandPalette]);

  // Ctrl+Wheel anywhere in the app resizes the masonry cards. Up = zoom
  // in (wider), down = zoom out (narrower). Bound to the window with
  // `passive: false` so we can preventDefault and stop WebView2's
  // built-in page-zoom from also firing.
  const bumpCardWidth = useStore((s) => s.bumpCardWidth);
  useEffect(() => {
    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey) return;
      e.preventDefault();
      // deltaY > 0 = scroll down = zoom out (narrower cards = negative step).
      // deltaY < 0 = scroll up   = zoom in  (wider cards   = positive step).
      const steps = e.deltaY > 0 ? -1 : 1;
      bumpCardWidth(steps);
    };
    window.addEventListener("wheel", onWheel, { passive: false });
    return () => window.removeEventListener("wheel", onWheel);
  }, [bumpCardWidth]);

  const filters = useStore((s) => s.filters);
  const reminders = useStore((s) => s.reminders);
  const searchMatchIds = useStore((s) => s.searchMatchIds);
  const filtered = useMemo(
    () => filterNotes(notes, section, search, filters, reminders, searchMatchIds),
    [notes, section, search, filters, reminders, searchMatchIds],
  );
  // NF-04 — Ctrl+A handler reaches the latest filtered list via this ref
  // so we don't need to register a new hotkey on every keystroke.
  const currentFilteredRef = useRef(filtered);
  useEffect(() => {
    currentFilteredRef.current = filtered;
  }, [filtered]);
  // Clear stale selections when notes are removed (e.g. trash sweep).
  useEffect(() => {
    if (selectedIds.size === 0) return;
    const ids = new Set(notes.map((n) => n.id));
    const stale = [...selectedIds].filter((id) => !ids.has(id));
    if (stale.length > 0) {
      const next = [...selectedIds].filter((id) => ids.has(id));
      setSelected(next);
    }
  }, [notes, selectedIds, setSelected]);

  const pinned = filtered.filter((n) => n.pinned && section.kind === "notes");
  const others = section.kind === "notes" ? filtered.filter((n) => !n.pinned) : filtered;

  const showNewBar = section.kind === "notes" && !search.trim();

  const headerLabel = (() => {
    if (section.kind === "label") {
      const l = labels.find((x) => x.id === section.labelId);
      return l?.name || "Label";
    }
    if (section.kind === "archive") return "Archive";
    if (section.kind === "trash") return "Trash";
    if (section.kind === "reminders") return "Reminders";
    return "Notes";
  })();

  return (
    <div className="h-full flex flex-col bg-white dark:bg-[#202124] text-gray-800 dark:text-gray-100">
      {selectedIds.size > 0 ? (
        <BulkActionBar visibleIds={filtered.map((n) => n.id)} />
      ) : (
        <TopBar onMenu={() => setSidebarExpanded((v) => !v)} />
      )}
      <div className="flex-1 min-h-0 flex">
        <Sidebar expanded={sidebarExpanded} />
        <main className="flex-1 min-w-0 overflow-y-auto">
          {loaded && <FilterChips />}
          <div className="px-4 sm:px-8 pt-2 pb-6">
          {!loaded ? (
            <LoadingState />
          ) : (
            <>
              {showNewBar && <NewNoteBar />}

              {section.kind === "trash" && (
                <div className="max-w-5xl mx-auto mb-4 flex items-center justify-between">
                  <p className="text-sm text-gray-600 dark:text-gray-400">
                    Notes in Trash can be restored or deleted forever.
                  </p>
                  {filtered.length > 0 && (
                    <button
                      onClick={() => setEmptyTrashOpen(true)}
                      className="text-sm px-3 py-1.5 rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10"
                    >
                      Empty Trash
                    </button>
                  )}
                </div>
              )}

              {filtered.length === 0 ? (
                <EmptyState section={section.kind} headerLabel={headerLabel} />
              ) : (
                <div className="mx-auto">
                  {section.kind === "notes" && pinned.length > 0 && (
                    <>
                      <SectionLabel text="PINNED" />
                      {/* stable-grid: each pinned card sits in its
                          position slot; unpinning a middle card leaves
                          a blank cell so the remaining pinned cards
                          stay in place. */}
                      <NoteGrid notes={pinned} layout="stable-grid" />
                      {others.length > 0 && <SectionLabel text="OTHERS" />}
                    </>
                  )}
                  {others.length > 0 && <NoteGrid notes={others} />}
                </div>
              )}
            </>
          )}
          </div>
        </main>
      </div>

      <NoteEditor />
      <Suspense fallback={null}>
        <SettingsModal />
        <LabelsManager />
        <HelpOverlay open={helpOpen} onClose={() => setHelpOpen(false)} />
        <LockScreen />
        <CommandPalette />
      </Suspense>

      <ConfirmDialog
        open={emptyTrashOpen}
        title="Empty Trash?"
        body="All notes in Trash will be permanently deleted. This cannot be undone."
        confirmLabel="Empty Trash"
        cancelLabel="Cancel"
        destructive
        onConfirm={performEmptyTrash}
        onCancel={() => setEmptyTrashOpen(false)}
      />

      <div
        role="status"
        aria-live="polite"
        aria-atomic="false"
        className="fixed left-1/2 -translate-x-1/2 bottom-6 z-50 flex flex-col-reverse items-center gap-2 pointer-events-none"
      >
        {toasts.map((t) => (
          <div
            key={t.id}
            className="px-4 py-2 rounded bg-[#3c4043] text-white text-sm shadow-lg pointer-events-auto flex items-center gap-3 max-w-md"
          >
            <span className="truncate">{t.text}</span>
            {t.action && (
              <button
                type="button"
                className="text-[#8ab4f8] font-medium hover:text-white px-1"
                onClick={async () => {
                  await t.action!.onClick();
                  dismissToast(t.id);
                }}
              >
                {t.action.label}
              </button>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

function SectionLabel({ text }: { text: string }) {
  return (
    <div className="text-[11px] font-medium tracking-widest text-gray-500 dark:text-gray-400 px-2 mt-2 mb-2 select-none">
      {text}
    </div>
  );
}

function LoadingState() {
  return (
    <div
      className="flex flex-col items-center justify-center text-gray-400 dark:text-gray-500 mt-32"
      role="status"
      aria-live="polite"
    >
      <Loader2 size={48} className="animate-spin motion-reduce:animate-none" aria-hidden />
      <div className="mt-4 text-sm">Loading your notes…</div>
    </div>
  );
}

function EmptyState({
  section,
  headerLabel,
}: {
  section: "notes" | "archive" | "trash" | "label" | "reminders";
  headerLabel: string;
}) {
  const map: Record<string, { icon: React.ReactNode; text: string }> = {
    notes: {
      icon: <Lightbulb size={120} strokeWidth={1.2} />,
      text: "Notes you add appear here",
    },
    archive: {
      icon: <Archive size={120} strokeWidth={1.2} />,
      text: "Your archived notes appear here",
    },
    trash: {
      icon: <Trash2 size={120} strokeWidth={1.2} />,
      text: "No notes in Trash",
    },
    label: {
      icon: <Tag size={120} strokeWidth={1.2} />,
      text: `No notes with label "${headerLabel}"`,
    },
    reminders: {
      icon: <Bell size={120} strokeWidth={1.2} />,
      text: "Notes with upcoming reminders appear here",
    },
  };
  const { icon, text } = map[section];
  return (
    <div className="flex flex-col items-center justify-center text-gray-400 dark:text-gray-500 mt-20">
      <div aria-hidden>{icon}</div>
      <div className="mt-4 text-lg">{text}</div>
    </div>
  );
}
