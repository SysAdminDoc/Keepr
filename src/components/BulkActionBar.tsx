import { useRef, useState } from "react";
import {
  X,
  Pin,
  PinOff,
  Palette,
  Archive,
  ArchiveRestore,
  Trash2,
  Tag,
} from "lucide-react";
import { useStore } from "../store";
import { api } from "../api";
import { IconBtn } from "./IconBtn";
import { ColorPicker } from "./ColorPicker";
import { useClickOutside } from "../hooks/useClickOutside";
import type { ColorKey } from "../types";

/**
 * Top-bar swap when the user has at least one note selected (NF-04).
 * Replaces the entire TopBar header content so the action bar feels like
 * a modal context — matches Keep's pattern.
 *
 * Bulk operations iterate the selected ids and call the existing single-
 * note commands. The optimistic store patches (EI-24) keep the UI live
 * without a reload; failures surface per-batch as a single toast.
 */
export function BulkActionBar() {
  const selectedIds = useStore((s) => s.selectedIds);
  const notes = useStore((s) => s.notes);
  const labels = useStore((s) => s.labels);
  const section = useStore((s) => s.section);
  const clearSelection = useStore((s) => s.clearSelection);
  const patchNote = useStore((s) => s.patchNote);
  const removeNote = useStore((s) => s.removeNote);
  const showToast = useStore((s) => s.showToast);

  const [colorOpen, setColorOpen] = useState(false);
  const [labelOpen, setLabelOpen] = useState(false);
  const colorRef = useRef<HTMLDivElement>(null);
  const labelRef = useRef<HTMLDivElement>(null);
  useClickOutside(colorRef, colorOpen, () => setColorOpen(false));
  useClickOutside(labelRef, labelOpen, () => setLabelOpen(false));

  const inTrash = section.kind === "trash";
  const inArchive = section.kind === "archive";

  const selected = notes.filter((n) => selectedIds.has(n.id));
  const allPinned = selected.length > 0 && selected.every((n) => n.pinned);
  const allArchived = selected.length > 0 && selected.every((n) => n.archived);

  const runBulk = async (
    label: string,
    op: (id: string) => Promise<void>,
  ) => {
    const ids = [...selectedIds];
    const failures: string[] = [];
    for (const id of ids) {
      try {
        await op(id);
      } catch (e) {
        failures.push(`${id}: ${String(e)}`);
      }
    }
    clearSelection();
    if (failures.length > 0) {
      showToast(`${label}: ${failures.length} failed`);
    } else {
      showToast(`${label} (${ids.length})`);
    }
  };

  const bulkPin = () =>
    runBulk(allPinned ? "Unpinned" : "Pinned", async (id) => {
      const next = !allPinned;
      await api.setPinned(id, next);
      patchNote(id, {
        pinned: next,
        archived: false,
        updated_at: new Date().toISOString(),
      });
    });

  const bulkArchive = () =>
    runBulk(allArchived ? "Unarchived" : "Archived", async (id) => {
      const next = !allArchived;
      await api.setArchived(id, next);
      patchNote(id, {
        archived: next,
        trashed: false,
        trashed_at: null,
        updated_at: new Date().toISOString(),
      });
    });

  const bulkTrash = () =>
    runBulk("Moved to Trash", async (id) => {
      const now = new Date().toISOString();
      await api.setTrashed(id, true);
      patchNote(id, {
        trashed: true,
        archived: false,
        pinned: false,
        trashed_at: now,
        updated_at: now,
      });
    });

  const bulkRestore = () =>
    runBulk("Restored", async (id) => {
      await api.setTrashed(id, false);
      patchNote(id, {
        trashed: false,
        trashed_at: null,
        updated_at: new Date().toISOString(),
      });
    });

  const bulkDeleteForever = () =>
    runBulk("Deleted", async (id) => {
      await api.deleteNotePermanent(id);
      removeNote(id);
    });

  const bulkColor = (color: ColorKey) => {
    setColorOpen(false);
    return runBulk("Color updated", async (id) => {
      await api.setColor(id, color);
      patchNote(id, { color, updated_at: new Date().toISOString() });
    });
  };

  const bulkToggleLabel = (labelId: string, add: boolean) => {
    setLabelOpen(false);
    return runBulk(add ? "Label added" : "Label removed", async (id) => {
      const n = useStore.getState().notes.find((x) => x.id === id);
      if (!n) return;
      const nextLabels = add
        ? [...new Set([...n.labels, labelId])]
        : n.labels.filter((x) => x !== labelId);
      await api.setNoteLabels(id, nextLabels);
      patchNote(id, { labels: nextLabels, updated_at: new Date().toISOString() });
    });
  };

  return (
    <header className="sticky top-0 z-40 flex items-center h-16 px-2 bg-[#feefc3] dark:bg-[#41331c] border-b border-[#fbbc04] dark:border-[#fdd663]">
      <IconBtn ariaLabel="Clear selection" onClick={clearSelection}>
        <X size={20} aria-hidden />
      </IconBtn>
      <span className="ml-2 mr-4 font-medium text-[#202124] dark:text-[#fdd663]">
        {selectedIds.size} selected
      </span>
      <div className="flex-1" />

      {!inTrash && (
        <IconBtn ariaLabel={allPinned ? "Unpin" : "Pin"} onClick={bulkPin}>
          {allPinned ? <PinOff size={20} aria-hidden /> : <Pin size={20} aria-hidden />}
        </IconBtn>
      )}

      {!inTrash && (
        <div className="relative" ref={colorRef}>
          <IconBtn ariaLabel="Change color" onClick={() => setColorOpen((v) => !v)}>
            <Palette size={20} aria-hidden />
          </IconBtn>
          {colorOpen && (
            <div
              className="absolute z-20 top-12 right-0"
              onClick={(e) => e.stopPropagation()}
            >
              <ColorPicker
                value={(selected[0]?.color ?? "default") as ColorKey}
                onChange={(c) => bulkColor(c)}
              />
            </div>
          )}
        </div>
      )}

      {!inTrash && (
        <div className="relative" ref={labelRef}>
          <IconBtn ariaLabel="Add or remove labels" onClick={() => setLabelOpen((v) => !v)}>
            <Tag size={20} aria-hidden />
          </IconBtn>
          {labelOpen && (
            <div
              className="absolute z-20 top-12 right-0 w-64 rounded-lg shadow-lg border bg-white dark:bg-[#2d2e30] dark:border-[#5f6368] p-2"
              onClick={(e) => e.stopPropagation()}
            >
              <div className="text-xs font-medium px-1 pb-1 opacity-70">
                Add or remove labels
              </div>
              <div className="max-h-48 overflow-y-auto">
                {labels.map((l) => {
                  // Tri-state count: how many selected notes have this label?
                  const count = selected.filter((n) => n.labels.includes(l.id)).length;
                  const all = count === selected.length;
                  return (
                    <label
                      key={l.id}
                      className="flex items-center gap-2 px-2 py-1 rounded hover:bg-black/5 dark:hover:bg-white/10 cursor-pointer text-sm"
                    >
                      <input
                        type="checkbox"
                        checked={all}
                        ref={(el) => {
                          // Indeterminate when some-but-not-all have the label.
                          if (el) el.indeterminate = count > 0 && !all;
                        }}
                        onChange={() => bulkToggleLabel(l.id, !all)}
                      />
                      <span className="truncate">{l.name}</span>
                    </label>
                  );
                })}
                {!labels.length && (
                  <div className="text-sm opacity-60 px-2 py-2">
                    No labels yet
                  </div>
                )}
              </div>
            </div>
          )}
        </div>
      )}

      {!inTrash && !inArchive && (
        <IconBtn ariaLabel="Archive" onClick={bulkArchive}>
          <Archive size={20} aria-hidden />
        </IconBtn>
      )}
      {inArchive && (
        <IconBtn ariaLabel="Unarchive" onClick={bulkArchive}>
          <ArchiveRestore size={20} aria-hidden />
        </IconBtn>
      )}
      {!inTrash && (
        <IconBtn ariaLabel="Move to Trash" onClick={bulkTrash}>
          <Trash2 size={20} aria-hidden />
        </IconBtn>
      )}
      {inTrash && (
        <>
          <IconBtn ariaLabel="Restore from Trash" onClick={bulkRestore}>
            <ArchiveRestore size={20} aria-hidden />
          </IconBtn>
          <IconBtn ariaLabel="Delete forever" onClick={bulkDeleteForever}>
            <Trash2 size={20} aria-hidden />
          </IconBtn>
        </>
      )}
    </header>
  );
}
