import { useEffect, useMemo, useRef, useState } from "react";
import { History, X, RotateCcw, Lock, ChevronRight, ChevronDown } from "lucide-react";
import clsx from "clsx";
import type { NoteSnapshot } from "../types";
import { api } from "../api";
import { useStore } from "../store";
import { useEscape } from "../hooks/useEscape";
import { useFocusTrap } from "../hooks/useFocusTrap";

/**
 * v0.22.0 — naive line-level diff. Splits both sides on `\n`, walks
 * the LCS (longest-common-subsequence) of lines to align kept/removed/
 * added rows. O(n*m) memory + time which is fine for the per-snapshot
 * body cap (~10 KB). Returns rows in display order.
 *
 * Why not diff-match-patch / fast-myers-diff: bundle size cost (17 KB+)
 * for a polish feature that runs once per snapshot expand, on bodies
 * that rarely exceed a few KB. The LCS table here is ~50 lines and has
 * zero dependencies.
 */
export type DiffRow = { kind: "same" | "removed" | "added"; line: string };

export function lineDiff(a: string, b: string): DiffRow[] {
  const aLines = a.split("\n");
  const bLines = b.split("\n");
  // Build LCS table.
  const m = aLines.length;
  const n = bLines.length;
  const t: number[][] = Array.from({ length: m + 1 }, () => new Array(n + 1).fill(0));
  for (let i = 0; i < m; i++) {
    for (let j = 0; j < n; j++) {
      if (aLines[i] === bLines[j]) t[i + 1][j + 1] = t[i][j] + 1;
      else t[i + 1][j + 1] = Math.max(t[i + 1][j], t[i][j + 1]);
    }
  }
  // Walk back.
  const out: DiffRow[] = [];
  let i = m;
  let j = n;
  while (i > 0 && j > 0) {
    if (aLines[i - 1] === bLines[j - 1]) {
      out.push({ kind: "same", line: aLines[i - 1] });
      i--;
      j--;
    } else if (t[i - 1][j] > t[i][j - 1]) {
      // Prefer "up" (a-side, removed) only when strictly better. On
      // ties prefer "left" (b-side, added) so the reversed output
      // emits removed-before-added for typical inline edits, which
      // reads more naturally ("the old line went, the new one came").
      out.push({ kind: "removed", line: aLines[i - 1] });
      i--;
    } else {
      out.push({ kind: "added", line: bLines[j - 1] });
      j--;
    }
  }
  while (i > 0) {
    out.push({ kind: "removed", line: aLines[i - 1] });
    i--;
  }
  while (j > 0) {
    out.push({ kind: "added", line: bLines[j - 1] });
    j--;
  }
  return out.reverse();
}

interface Props {
  open: boolean;
  noteId: string | null;
  onClose: () => void;
  onRestored: () => void;
}

/**
 * NF-V0.5-D — note history drawer. Lists the up-to-20 snapshots Rust
 * has captured for this note, with relative timestamps and a Restore
 * button per row. Body preview is the first ~6 lines of the snapshot's
 * body. Vault snapshots show "Vault ciphertext" instead of plaintext;
 * restoring them puts the ciphertext back without needing the DEK.
 */
export function HistoryDrawer({ open, noteId, onClose, onRestored }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  useEscape(open, onClose);
  useFocusTrap(containerRef, open);
  const showToast = useStore((s) => s.showToast);
  const notes = useStore((s) => s.notes);
  const [snapshots, setSnapshots] = useState<NoteSnapshot[]>([]);
  const [loading, setLoading] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [expandedId, setExpandedId] = useState<string | null>(null);

  // Read the live body from the store so the diff has something to
  // compare against. If the note isn't found (deleted while drawer
  // is open) we fall back to an empty string.
  const currentBody = useMemo(() => {
    if (!noteId) return "";
    return notes.find((n) => n.id === noteId)?.body ?? "";
  }, [noteId, notes]);

  useEffect(() => {
    if (!open || !noteId) return;
    let cancelled = false;
    setLoading(true);
    api
      .listSnapshots(noteId)
      .then((s) => {
        if (!cancelled) setSnapshots(s);
      })
      .catch((e) => {
        if (!cancelled) showToast("Could not load history: " + String(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [open, noteId, showToast]);

  if (!open) return null;

  const restore = async (snapshotId: string) => {
    if (busyId !== null) return;
    setBusyId(snapshotId);
    try {
      await api.restoreSnapshot(snapshotId);
      showToast("Restored from history");
      onRestored();
      onClose();
    } catch (e) {
      showToast("Restore failed: " + String(e));
    } finally {
      setBusyId(null);
    }
  };

  return (
    <div
      className="fixed inset-0 z-[60] modal-backdrop grid place-items-center p-4"
      onClick={onClose}
      role="dialog"
      aria-modal="true"
      aria-labelledby="history-drawer-title"
    >
      <div
        ref={containerRef}
        className="w-full max-w-2xl max-h-[80vh] flex flex-col rounded-lg shadow-keep-hover bg-white dark:bg-[#2d2e30] text-gray-800 dark:text-gray-100"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between px-5 py-3 border-b border-gray-200 dark:border-[#5f6368]">
          <h2
            id="history-drawer-title"
            className="text-base font-medium flex items-center gap-2"
          >
            <History size={16} aria-hidden /> Version history
          </h2>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close history"
            className="p-2 rounded-full hover:bg-black/5 dark:hover:bg-white/10"
          >
            <X size={18} aria-hidden />
          </button>
        </div>
        <div className="flex-1 overflow-y-auto px-3 py-2">
          {loading ? (
            <div className="text-sm text-gray-500 dark:text-gray-400 px-2 py-4">
              Loading…
            </div>
          ) : snapshots.length === 0 ? (
            <div className="text-sm text-gray-500 dark:text-gray-400 px-2 py-6 text-center">
              No history yet. Edits to this note will start populating here.
            </div>
          ) : (
            <ul className="divide-y divide-gray-200 dark:divide-[#5f6368]">
              {snapshots.map((s) => {
                const isExpanded = expandedId === s.id;
                const isVault = s.vault === "vault";
                const canDiff = !isVault;
                return (
                  <li key={s.id} className="py-3 px-2">
                    <div className="flex gap-3">
                      <button
                        type="button"
                        onClick={() => canDiff && setExpandedId(isExpanded ? null : s.id)}
                        disabled={!canDiff}
                        aria-expanded={isExpanded}
                        aria-label={isExpanded ? "Collapse diff" : "Expand diff vs current"}
                        className="mt-0.5 p-1 rounded hover:bg-black/5 dark:hover:bg-white/10 disabled:opacity-30 disabled:hover:bg-transparent"
                      >
                        {isExpanded ? (
                          <ChevronDown size={14} aria-hidden />
                        ) : (
                          <ChevronRight size={14} aria-hidden />
                        )}
                      </button>
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
                          <span>{formatTimestamp(s.takenAt)}</span>
                          {isVault && (
                            <span className="inline-flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded bg-[#fdd663]/30 text-[#594300] dark:bg-[#41331c] dark:text-[#fdd663]">
                              <Lock size={9} aria-hidden /> ciphertext
                            </span>
                          )}
                        </div>
                        {isVault ? (
                          <div className="mt-1 text-sm opacity-70 italic">
                            Encrypted vault payload — restore returns the
                            note to this ciphertext.
                          </div>
                        ) : (
                          <>
                            {s.title && (
                              <div className="mt-1 font-medium truncate">
                                {s.title}
                              </div>
                            )}
                            {s.body && !isExpanded && (
                              <div className="mt-1 text-sm whitespace-pre-wrap line-clamp-6 opacity-90">
                                {s.body}
                              </div>
                            )}
                            {s.checklist.length > 0 && (
                              <div className="mt-1 text-sm opacity-80">
                                {s.checklist.length} checklist item
                                {s.checklist.length === 1 ? "" : "s"}
                              </div>
                            )}
                          </>
                        )}
                      </div>
                      <div className="shrink-0">
                        <button
                          type="button"
                          onClick={() => restore(s.id)}
                          disabled={busyId !== null}
                          className="inline-flex items-center gap-1 px-3 py-1.5 text-sm rounded border border-gray-300 dark:border-[#5f6368] hover:bg-black/5 dark:hover:bg-white/10 disabled:opacity-50"
                        >
                          <RotateCcw size={14} aria-hidden />
                          {busyId === s.id ? "Restoring…" : "Restore"}
                        </button>
                      </div>
                    </div>
                    {isExpanded && !isVault && (
                      <DiffView
                        oldBody={s.body ?? ""}
                        newBody={currentBody}
                      />
                    )}
                  </li>
                );
              })}
            </ul>
          )}
        </div>
        <div className="px-5 py-3 border-t border-gray-200 dark:border-[#5f6368] text-xs text-gray-500 dark:text-gray-400">
          Keepr keeps the last 20 versions per note. Restoring is itself
          captured as a new snapshot, so it can be undone.
        </div>
      </div>
    </div>
  );
}

function DiffView({ oldBody, newBody }: { oldBody: string; newBody: string }) {
  const rows = useMemo(() => lineDiff(oldBody, newBody), [oldBody, newBody]);
  if (oldBody === newBody) {
    return (
      <div className="mt-2 ml-8 text-xs opacity-60 italic">
        No change between this snapshot and the current note.
      </div>
    );
  }
  return (
    <div className="mt-2 ml-8 text-xs font-mono rounded border border-gray-200 dark:border-[#5f6368] overflow-hidden">
      <div className="px-2 py-1 bg-black/5 dark:bg-white/5 text-[11px] opacity-70">
        Diff: this snapshot → current note
      </div>
      <div className="max-h-64 overflow-y-auto">
        {rows.map((r, i) => (
          <div
            key={i}
            className={clsx(
              "px-2 py-0.5 whitespace-pre-wrap break-words",
              r.kind === "removed" && "bg-red-100 dark:bg-red-950/40 text-red-900 dark:text-red-200 line-through",
              r.kind === "added" && "bg-green-100 dark:bg-green-950/40 text-green-900 dark:text-green-200",
            )}
          >
            <span className="opacity-50 mr-1 select-none">
              {r.kind === "removed" ? "−" : r.kind === "added" ? "+" : " "}
            </span>
            {r.line || " "}
          </div>
        ))}
      </div>
    </div>
  );
}

function formatTimestamp(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const now = Date.now();
  const diffMs = now - d.getTime();
  const min = Math.floor(diffMs / 60_000);
  if (min < 1) return "Just now";
  if (min < 60) return `${min} minute${min === 1 ? "" : "s"} ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr} hour${hr === 1 ? "" : "s"} ago`;
  const day = Math.floor(hr / 24);
  if (day < 30) return `${day} day${day === 1 ? "" : "s"} ago`;
  return d.toLocaleString();
}
