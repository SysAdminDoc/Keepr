import { useEffect, useMemo, useRef, useState } from "react";
import { Search, FileText, Tag, Settings, Bell, Archive, Trash2, Lightbulb, Pin, Lock, Unlock, Sun, Moon, Monitor } from "lucide-react";
import clsx from "clsx";
import { useStore } from "../store";
import type { Section } from "../types";

/**
 * Cmd/Ctrl+K command palette (v0.20.0).
 *
 * Fuzzy-matches across:
 * - every note title (open editor for that note)
 * - every section (Notes / Reminders / Archive / Trash)
 * - every label
 * - canned settings actions (open settings, toggle theme, etc.)
 *
 * Match scoring is intentionally simple — substring + prefix bonus.
 * No external fuzzy library; sub-50ms for ~1000 notes is well under the
 * keystroke budget.
 */

interface Cmd {
  id: string;
  label: string;
  hint?: string;
  icon: React.ReactNode;
  group: "Notes" | "Sections" | "Labels" | "Actions";
  invoke: () => void;
}

function score(query: string, label: string): number {
  if (!query) return 0;
  const q = query.toLowerCase();
  const l = label.toLowerCase();
  const idx = l.indexOf(q);
  if (idx === -1) return -1;
  // Prefix match worth more than middle match. Shorter labels rank
  // slightly higher when otherwise tied.
  let s = 100 - idx;
  if (idx === 0) s += 50;
  s -= Math.min(l.length, 30) * 0.5;
  return s;
}

export function CommandPalette() {
  const open = useStore((s) => s.commandPaletteOpen);
  const close = useStore((s) => s.closeCommandPalette);
  const notes = useStore((s) => s.notes);
  const labels = useStore((s) => s.labels);
  const setSection = useStore((s) => s.setSection);
  const openEditor = useStore((s) => s.openEditor);
  const openSettings = useStore((s) => s.openSettings);
  const openLabelsManager = useStore((s) => s.openLabelsManager);
  const setThemeMode = useStore((s) => s.setThemeMode);
  const themeMode = useStore((s) => s.themeMode);
  const toggleViewMode = useStore((s) => s.toggleViewMode);
  const vaultUnlocked = useStore((s) => s.vaultUnlocked);
  const vaultInitialized = useStore((s) => s.vaultInitialized);

  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);

  // Reset state every open so previous query doesn't linger.
  useEffect(() => {
    if (open) {
      setQuery("");
      setActive(0);
      // Focus on next tick so the input is mounted.
      setTimeout(() => inputRef.current?.focus(), 0);
    }
  }, [open]);

  const commands: Cmd[] = useMemo(() => {
    const out: Cmd[] = [];
    // Sections
    const section = (s: Section, label: string, icon: React.ReactNode): Cmd => ({
      id: `section:${label}`,
      label: `Go to ${label}`,
      group: "Sections",
      icon,
      invoke: () => {
        setSection(s);
        close();
      },
    });
    out.push(section({ kind: "notes" }, "Notes", <Lightbulb size={16} />));
    out.push(section({ kind: "reminders" }, "Reminders", <Bell size={16} />));
    out.push(section({ kind: "archive" }, "Archive", <Archive size={16} />));
    out.push(section({ kind: "trash" }, "Trash", <Trash2 size={16} />));
    // Labels
    for (const l of labels) {
      out.push({
        id: `label:${l.id}`,
        label: `Label: ${l.name}`,
        group: "Labels",
        icon: <Tag size={16} />,
        invoke: () => {
          setSection({ kind: "label", labelId: l.id });
          close();
        },
      });
    }
    // Actions
    out.push({
      id: "action:settings",
      label: "Open Settings",
      group: "Actions",
      icon: <Settings size={16} />,
      invoke: () => {
        openSettings();
        close();
      },
    });
    out.push({
      id: "action:labels",
      label: "Manage labels",
      group: "Actions",
      icon: <Tag size={16} />,
      invoke: () => {
        openLabelsManager();
        close();
      },
    });
    out.push({
      id: "action:theme-light",
      label: "Theme: Light",
      hint: themeMode === "light" ? "current" : undefined,
      group: "Actions",
      icon: <Sun size={16} />,
      invoke: () => {
        setThemeMode("light");
        close();
      },
    });
    out.push({
      id: "action:theme-dark",
      label: "Theme: Dark",
      hint: themeMode === "dark" ? "current" : undefined,
      group: "Actions",
      icon: <Moon size={16} />,
      invoke: () => {
        setThemeMode("dark");
        close();
      },
    });
    out.push({
      id: "action:theme-system",
      label: "Theme: Follow system",
      hint: themeMode === "system" ? "current" : undefined,
      group: "Actions",
      icon: <Monitor size={16} />,
      invoke: () => {
        setThemeMode("system");
        close();
      },
    });
    out.push({
      id: "action:view-toggle",
      label: "Toggle grid / list view",
      group: "Actions",
      icon: <Pin size={16} />,
      invoke: () => {
        toggleViewMode();
        close();
      },
    });
    if (vaultInitialized) {
      out.push({
        id: "action:vault",
        label: vaultUnlocked ? "Vault: open Settings to lock" : "Vault: open Settings to unlock",
        group: "Actions",
        icon: vaultUnlocked ? <Unlock size={16} /> : <Lock size={16} />,
        invoke: () => {
          openSettings();
          close();
        },
      });
    }
    // Notes (titles only, untitled notes skipped)
    for (const n of notes) {
      if (n.trashed) continue;
      const title = n.title.trim() || (n.body.trim().slice(0, 60) || "Untitled note");
      out.push({
        id: `note:${n.id}`,
        label: title,
        group: "Notes",
        icon: <FileText size={16} />,
        invoke: () => {
          openEditor(n.id);
          close();
        },
      });
    }
    return out;
  }, [
    notes,
    labels,
    setSection,
    openEditor,
    openSettings,
    openLabelsManager,
    setThemeMode,
    themeMode,
    toggleViewMode,
    vaultInitialized,
    vaultUnlocked,
    close,
  ]);

  const ranked = useMemo(() => {
    if (!query.trim()) {
      // No query: show first 30 commands ordered by group priority.
      const groupOrder: Cmd["group"][] = ["Actions", "Sections", "Labels", "Notes"];
      const sorted = [...commands].sort(
        (a, b) => groupOrder.indexOf(a.group) - groupOrder.indexOf(b.group),
      );
      return sorted.slice(0, 30);
    }
    const scored = commands
      .map((c) => ({ c, s: score(query, c.label) }))
      .filter((x) => x.s >= 0)
      .sort((a, b) => b.s - a.s);
    return scored.slice(0, 50).map((x) => x.c);
  }, [query, commands]);

  // Clamp active when ranked list shortens.
  useEffect(() => {
    if (active >= ranked.length) setActive(Math.max(0, ranked.length - 1));
  }, [ranked, active]);

  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        close();
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        setActive((a) => Math.min(a + 1, ranked.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setActive((a) => Math.max(a - 1, 0));
      } else if (e.key === "Enter") {
        e.preventDefault();
        const cmd = ranked[active];
        if (cmd) cmd.invoke();
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [open, close, ranked, active]);

  // Scroll the active item into view inside the list.
  useEffect(() => {
    const el = listRef.current?.querySelector<HTMLLIElement>(
      `li[data-idx="${active}"]`,
    );
    el?.scrollIntoView({ block: "nearest" });
  }, [active]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-[60] modal-backdrop grid place-items-start pt-[15vh] p-4"
      role="dialog"
      aria-modal="true"
      aria-label="Command palette"
      onClick={close}
    >
      <div
        className="w-full max-w-xl rounded-lg shadow-2xl border border-gray-300 dark:border-[#5f6368] bg-white dark:bg-[#2d2e30] overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2 px-4 py-3 border-b border-gray-200 dark:border-[#5f6368]">
          <Search size={18} aria-hidden className="text-gray-500 dark:text-gray-400" />
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setActive(0);
            }}
            placeholder="Type a command, note title, label, or section…"
            aria-label="Command palette query"
            className="flex-1 bg-transparent outline-none text-base"
          />
          <kbd className="text-xs px-1.5 py-0.5 rounded border border-gray-300 dark:border-[#5f6368] text-gray-500 dark:text-gray-400">
            Esc
          </kbd>
        </div>
        <ul
          ref={listRef}
          role="listbox"
          aria-label="Results"
          className="max-h-[50vh] overflow-y-auto"
        >
          {ranked.length === 0 && (
            <li className="px-4 py-6 text-sm text-center text-gray-500 dark:text-gray-400">
              No matches
            </li>
          )}
          {ranked.map((c, i) => (
            <li
              key={c.id}
              data-idx={i}
              role="option"
              aria-selected={i === active}
              onMouseEnter={() => setActive(i)}
              onClick={c.invoke}
              className={clsx(
                "flex items-center gap-3 px-4 py-2 cursor-pointer text-sm",
                i === active
                  ? "bg-[var(--keepr-accent)] text-white"
                  : "hover:bg-black/5 dark:hover:bg-white/10",
              )}
            >
              <span aria-hidden className={i === active ? "text-white" : "text-gray-500 dark:text-gray-400"}>
                {c.icon}
              </span>
              <span className="flex-1 truncate">{c.label}</span>
              {c.hint && (
                <span className={clsx("text-xs", i === active ? "opacity-90" : "opacity-60")}>
                  {c.hint}
                </span>
              )}
              <span className={clsx("text-[10px] uppercase tracking-wide", i === active ? "opacity-90" : "opacity-50")}>
                {c.group}
              </span>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
