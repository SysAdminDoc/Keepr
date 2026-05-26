import { useEffect, useRef, useState } from "react";
import { History, X, RotateCcw, Lock } from "lucide-react";
import type { NoteSnapshot } from "../types";
import { api } from "../api";
import { useStore } from "../store";
import { useEscape } from "../hooks/useEscape";
import { useFocusTrap } from "../hooks/useFocusTrap";

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
  const [snapshots, setSnapshots] = useState<NoteSnapshot[]>([]);
  const [loading, setLoading] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);

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
              {snapshots.map((s) => (
                <li key={s.id} className="py-3 px-2 flex gap-3">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
                      <span>{formatTimestamp(s.takenAt)}</span>
                      {s.vault === "vault" && (
                        <span className="inline-flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded-full bg-[#fdd663]/30 text-[#594300] dark:bg-[#41331c] dark:text-[#fdd663]">
                          <Lock size={9} aria-hidden /> ciphertext
                        </span>
                      )}
                    </div>
                    {s.vault === "vault" ? (
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
                        {s.body && (
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
                </li>
              ))}
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
