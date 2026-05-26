import { useEffect, useRef, useState } from "react";
import {
  Menu,
  Search,
  RefreshCw,
  Settings,
  Moon,
  Sun,
  X,
  LayoutGrid,
  Rows3,
  ArrowUpDown,
} from "lucide-react";
import { useClickOutside } from "../hooks/useClickOutside";
import type { SortMode } from "../store";
import clsx from "clsx";
import { useStore } from "../store";

interface Props {
  onMenu: () => void;
}

const SEARCH_DEBOUNCE_MS = 150;

export function TopBar({ onMenu }: Props) {
  const search = useStore((s) => s.search);
  const setSearch = useStore((s) => s.setSearch);
  const dark = useStore((s) => s.dark);
  const toggleDark = useStore((s) => s.toggleDark);
  const viewMode = useStore((s) => s.viewMode);
  const toggleViewMode = useStore((s) => s.toggleViewMode);
  const sortMode = useStore((s) => s.sortMode);
  const setSortMode = useStore((s) => s.setSortMode);
  const openSettings = useStore((s) => s.openSettings);
  const load = useStore((s) => s.load);
  const [sortMenuOpen, setSortMenuOpen] = useState(false);
  const sortRef = useRef<HTMLDivElement>(null);
  useClickOutside(sortRef, sortMenuOpen, () => setSortMenuOpen(false));

  // EI-18 — local input state with debounced commit to the store. Typing in
  // the input no longer triggers a re-render of every card on every
  // keystroke; the store only updates after the user stops typing for 150 ms.
  const [localSearch, setLocalSearch] = useState(search);
  // Keep local input in sync if the store changes externally (e.g. "X" click
  // from somewhere else, or section change clearing).
  useEffect(() => {
    setLocalSearch(search);
  }, [search]);
  useEffect(() => {
    const t = setTimeout(() => {
      if (localSearch !== search) setSearch(localSearch);
    }, SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(t);
  }, [localSearch, search, setSearch]);

  // EI-16 — visible refresh feedback so the user knows the click did
  // something.
  const [refreshing, setRefreshing] = useState(false);
  const spinTimer = useRef<number | null>(null);
  const onRefresh = async () => {
    setRefreshing(true);
    try {
      await load();
    } finally {
      // Hold the spin at least 250ms so a fast load still shows feedback.
      if (spinTimer.current) window.clearTimeout(spinTimer.current);
      spinTimer.current = window.setTimeout(() => setRefreshing(false), 250);
    }
  };
  useEffect(() => () => {
    if (spinTimer.current) window.clearTimeout(spinTimer.current);
  }, []);

  return (
    <header className="sticky top-0 z-30 flex items-center h-16 px-2 bg-white dark:bg-[#202124] border-b border-gray-200 dark:border-[#5f6368]">
      <button
        className="p-3 rounded-full hover:bg-gray-200 dark:hover:bg-[#3c4043]"
        onClick={onMenu}
        aria-label="Toggle sidebar"
        title="Menu"
      >
        <Menu size={20} aria-hidden />
      </button>
      <div className="flex items-center gap-2 px-2 mr-4 select-none">
        <div
          className="w-8 h-8 rounded-md bg-[#FBBC04] grid place-items-center text-[#202124] font-bold"
          aria-hidden
        >
          K
        </div>
        <span className="text-[22px] text-gray-700 dark:text-gray-200 font-product hidden sm:inline">
          Keepr
        </span>
      </div>
      <div className="flex-1 max-w-2xl mx-auto">
        <div className="flex items-center bg-[#f1f3f4] dark:bg-[#3c4043] rounded-lg px-3 h-12 focus-within:bg-white dark:focus-within:bg-[#202124] focus-within:shadow-md">
          <Search size={20} className="text-gray-500 dark:text-gray-400" aria-hidden />
          <input
            type="search"
            placeholder="Search"
            aria-label="Search notes"
            className="flex-1 bg-transparent outline-none px-3 text-base"
            value={localSearch}
            onChange={(e) => setLocalSearch(e.target.value)}
          />
          {localSearch && (
            <button
              onClick={() => {
                setLocalSearch("");
                setSearch("");
              }}
              aria-label="Clear search"
              title="Clear search"
              className="p-1 rounded-full hover:bg-gray-300 dark:hover:bg-[#5f6368]"
            >
              <X size={18} aria-hidden />
            </button>
          )}
        </div>
      </div>
      <div className="flex items-center pl-2">
        <button
          className="p-3 rounded-full hover:bg-gray-200 dark:hover:bg-[#3c4043]"
          onClick={onRefresh}
          aria-label="Refresh notes"
          title="Refresh"
        >
          <RefreshCw
            size={20}
            aria-hidden
            className={clsx(refreshing && "animate-spin motion-reduce:animate-none")}
          />
        </button>
        <div className="relative" ref={sortRef}>
          <button
            className="p-3 rounded-full hover:bg-gray-200 dark:hover:bg-[#3c4043]"
            onClick={() => setSortMenuOpen((v) => !v)}
            aria-label="Sort notes"
            aria-haspopup="true"
            aria-expanded={sortMenuOpen}
            title="Sort"
          >
            <ArrowUpDown size={20} aria-hidden />
          </button>
          {sortMenuOpen && (
            <div
              className="absolute right-0 top-12 z-30 w-44 rounded-lg shadow-lg border bg-white dark:bg-[#2d2e30] dark:border-[#5f6368] p-1"
              role="menu"
              onClick={(e) => e.stopPropagation()}
            >
              <div className="text-[11px] font-medium uppercase tracking-wide px-3 py-1 opacity-60">
                Sort by
              </div>
              {(
                [
                  ["modified", "Date modified"],
                  ["created", "Date created"],
                  ["title", "Title (A–Z)"],
                  ["custom", "Custom"],
                ] as [SortMode, string][]
              ).map(([m, label]) => (
                <button
                  key={m}
                  type="button"
                  role="menuitemradio"
                  aria-checked={sortMode === m}
                  onClick={() => {
                    setSortMode(m);
                    setSortMenuOpen(false);
                  }}
                  className={
                    sortMode === m
                      ? "block w-full text-left text-sm px-3 py-1.5 rounded text-[#1a73e8] font-medium"
                      : "block w-full text-left text-sm px-3 py-1.5 rounded hover:bg-black/5 dark:hover:bg-white/10"
                  }
                >
                  {label}
                </button>
              ))}
            </div>
          )}
        </div>
        <button
          className="p-3 rounded-full hover:bg-gray-200 dark:hover:bg-[#3c4043]"
          onClick={toggleViewMode}
          aria-label={
            viewMode === "grid"
              ? "Switch to list view (Ctrl+G)"
              : "Switch to grid view (Ctrl+G)"
          }
          title={viewMode === "grid" ? "List view" : "Grid view"}
        >
          {viewMode === "grid" ? (
            <Rows3 size={20} aria-hidden />
          ) : (
            <LayoutGrid size={20} aria-hidden />
          )}
        </button>
        <button
          className="p-3 rounded-full hover:bg-gray-200 dark:hover:bg-[#3c4043]"
          onClick={toggleDark}
          aria-label={dark ? "Switch to light theme" : "Switch to dark theme"}
          title={dark ? "Light mode" : "Dark mode"}
        >
          {dark ? <Sun size={20} aria-hidden /> : <Moon size={20} aria-hidden />}
        </button>
        <button
          className="p-3 rounded-full hover:bg-gray-200 dark:hover:bg-[#3c4043]"
          onClick={openSettings}
          aria-label="Open settings"
          title="Settings"
        >
          <Settings size={20} aria-hidden />
        </button>
      </div>
    </header>
  );
}
