import {
  Pin,
  PinOff,
  Palette,
  Archive,
  ArchiveRestore,
  Trash2,
  RotateCcw,
  Check,
} from "lucide-react";
import clsx from "clsx";
import { useRef, useState } from "react";
import type { Note } from "../types";
import { bgFor, borderFor } from "../colors";
import { useStore } from "../store";
import { api } from "../api";
import { ColorPicker } from "./ColorPicker";
import { IconBtn } from "./IconBtn";
import { useClickOutside } from "../hooks/useClickOutside";
import { daysLeftInTrash } from "../lib/trashRetention";

interface Props {
  note: Note;
}

export function NoteCard({ note }: Props) {
  const section = useStore((s) => s.section);
  const dark = useStore((s) => s.dark);
  const openEditor = useStore((s) => s.openEditor);
  const showToast = useStore((s) => s.showToast);
  const patchNote = useStore((s) => s.patchNote);
  const removeNote = useStore((s) => s.removeNote);
  const trashRetentionDays = useStore((s) => s.trashRetentionDays);
  const selectedIds = useStore((s) => s.selectedIds);
  const toggleSelected = useStore((s) => s.toggleSelected);
  const selectMode = selectedIds.size > 0;
  const isSelected = selectedIds.has(note.id);
  const [colorOpen, setColorOpen] = useState(false);
  const popoverRef = useRef<HTMLDivElement>(null);
  useClickOutside(popoverRef, colorOpen, () => setColorOpen(false));

  const inTrash = section.kind === "trash";
  const inArchive = section.kind === "archive";

  const bg = bgFor(note.color, dark);
  const border = borderFor(note.color, dark);

  // EI-17 — every mutation runs inside a try/catch so a Rust error reaches
  // the user as a toast instead of an unhandled promise rejection.
  const withToast = async (label: string, fn: () => Promise<void>) => {
    try {
      await fn();
    } catch (e) {
      showToast(`Could not ${label}: ${String(e)}`);
    }
  };

  const togglePin = (e: React.MouseEvent) => {
    e.stopPropagation();
    return withToast("update note", async () => {
      const next = !note.pinned;
      await api.setPinned(note.id, next);
      // EI-24 — patch in place. setPinned also clears `archived` server-side.
      patchNote(note.id, {
        pinned: next,
        archived: false,
        updated_at: new Date().toISOString(),
      });
    });
  };

  const toggleArchive = (e: React.MouseEvent) => {
    e.stopPropagation();
    return withToast("archive note", async () => {
      const becomingArchived = !note.archived;
      await api.setArchived(note.id, becomingArchived);
      patchNote(note.id, {
        archived: becomingArchived,
        trashed: false,
        trashed_at: null,
        updated_at: new Date().toISOString(),
      });
      // EI-15 — Undo for archive (Keep parity).
      showToast(becomingArchived ? "Note archived" : "Note unarchived", {
        action: {
          label: "Undo",
          onClick: async () => {
            await api.setArchived(note.id, !becomingArchived);
            patchNote(note.id, {
              archived: !becomingArchived,
              updated_at: new Date().toISOString(),
            });
          },
        },
      });
    });
  };

  const trash = (e: React.MouseEvent) => {
    e.stopPropagation();
    return withToast("trash note", async () => {
      const now = new Date().toISOString();
      await api.setTrashed(note.id, true);
      patchNote(note.id, {
        trashed: true,
        archived: false,
        pinned: false,
        trashed_at: now,
        updated_at: now,
      });
      // EI-15 — Undo for trash (Keep parity, 5s window).
      showToast("Note moved to Trash", {
        action: {
          label: "Undo",
          onClick: async () => {
            await api.setTrashed(note.id, false);
            patchNote(note.id, {
              trashed: false,
              trashed_at: null,
              updated_at: new Date().toISOString(),
            });
          },
        },
      });
    });
  };

  const restore = (e: React.MouseEvent) => {
    e.stopPropagation();
    return withToast("restore note", async () => {
      await api.setTrashed(note.id, false);
      patchNote(note.id, {
        trashed: false,
        trashed_at: null,
        updated_at: new Date().toISOString(),
      });
      showToast("Note restored");
    });
  };

  const deleteForever = (e: React.MouseEvent) => {
    e.stopPropagation();
    return withToast("delete note", async () => {
      await api.deleteNotePermanent(note.id);
      removeNote(note.id);
      showToast("Note deleted");
    });
  };

  const cardActivate = () => {
    if (selectMode) {
      toggleSelected(note.id);
      return;
    }
    if (inTrash) return;
    openEditor(note.id);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      cardActivate();
    } else if (e.key.toLowerCase() === "x") {
      e.preventDefault();
      toggleSelected(note.id);
    }
  };

  const setColor = (color: string) =>
    withToast("change color", async () => {
      await api.setColor(note.id, color);
      setColorOpen(false);
      patchNote(note.id, {
        color: color as Note["color"],
        updated_at: new Date().toISOString(),
      });
    });

  const cardLabel = note.title || (note.body ? note.body.slice(0, 60) : "Untitled note");
  const daysLeft = inTrash ? daysLeftInTrash(note, trashRetentionDays) : null;

  return (
    <div
      className={clsx(
        "note-card group relative rounded-lg border shadow-keep hover:shadow-keep-hover cursor-default",
        "transition-shadow motion-reduce:transition-none",
        isSelected && "ring-2 ring-[#1a73e8] ring-offset-1",
      )}
      style={{ background: bg, borderColor: border }}
      onClick={cardActivate}
      onKeyDown={onKeyDown}
      role="button"
      tabIndex={0}
      aria-label={cardLabel}
      aria-pressed={selectMode ? isSelected : undefined}
      data-note-id={note.id}
    >
      {/* NF-04 — select checkmark in the top-left corner. Visible on hover
          or whenever any other card is selected. Click toggles selection. */}
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          toggleSelected(note.id);
        }}
        aria-label={isSelected ? "Deselect note" : "Select note"}
        aria-pressed={isSelected}
        title={isSelected ? "Deselect" : "Select"}
        className={clsx(
          "absolute top-2 left-2 w-6 h-6 rounded-full grid place-items-center text-white transition-opacity motion-reduce:transition-none",
          isSelected
            ? "opacity-100 bg-[#1a73e8]"
            : "opacity-0 group-hover:opacity-100 focus:opacity-100 bg-black/40 hover:bg-black/60",
          selectMode && "opacity-100",
        )}
      >
        <Check size={14} aria-hidden />
      </button>
      {!inTrash && (
        <button
          onClick={togglePin}
          aria-label={note.pinned ? "Unpin note" : "Pin note"}
          aria-pressed={note.pinned}
          title={note.pinned ? "Unpin" : "Pin"}
          className={clsx(
            "absolute top-2 right-2 p-1.5 rounded-full hover:bg-black/10 dark:hover:bg-white/10 transition-opacity motion-reduce:transition-none",
            note.pinned ? "opacity-100" : "opacity-0 group-hover:opacity-100 focus:opacity-100",
          )}
        >
          {note.pinned ? <Pin size={18} aria-hidden /> : <PinOff size={18} aria-hidden />}
        </button>
      )}

      <div className="px-4 pt-3 pb-2 pr-10">
        {note.title && (
          <div className="font-medium text-base leading-snug break-words">
            {note.title}
          </div>
        )}
      </div>

      {note.kind === "text" ? (
        note.body && (
          <div className="px-4 pb-3 text-[14px] leading-snug whitespace-pre-wrap break-words max-h-[16rem] overflow-hidden">
            {note.body}
          </div>
        )
      ) : (
        <div className="px-2 pb-2">
          {note.checklist.slice(0, 12).map((it) => (
            <div
              key={it.id}
              className="flex items-start gap-2 px-2 py-1 text-[14px]"
            >
              <span
                className="w-4 h-4 mt-0.5 grid place-items-center border rounded-sm border-current opacity-70"
                role="img"
                aria-label={it.checked ? "Checked" : "Unchecked"}
              >
                {it.checked && <Check size={12} aria-hidden />}
              </span>
              <span
                className={clsx(
                  "flex-1 break-words",
                  it.checked && "line-through opacity-60",
                )}
              >
                {it.text}
              </span>
            </div>
          ))}
          {note.checklist.length > 12 && (
            <div className="px-3 py-1 text-xs opacity-70">
              + {note.checklist.length - 12} more
            </div>
          )}
        </div>
      )}

      {daysLeft !== null && (
        <div className="px-3 pb-1 text-[11px] uppercase tracking-wide font-medium opacity-70">
          {daysLeft === 1 ? "1 day left" : `${daysLeft} days left`}
        </div>
      )}

      <ChipsRow noteLabelIds={note.labels} />

      <div className="hover-actions flex items-center px-1 pb-1">
        {!inTrash && !inArchive && (
          <>
            <div className="relative" ref={popoverRef}>
              <IconBtn
                ariaLabel="Background options"
                onClick={(e) => {
                  e.stopPropagation();
                  setColorOpen((v) => !v);
                }}
              >
                <Palette size={18} aria-hidden />
              </IconBtn>
              {colorOpen && (
                <div
                  className="absolute z-20 top-9 left-0"
                  onClick={(e) => e.stopPropagation()}
                >
                  <ColorPicker
                    value={note.color}
                    onChange={setColor}
                    onClose={() => setColorOpen(false)}
                  />
                </div>
              )}
            </div>
            <IconBtn ariaLabel="Archive" onClick={toggleArchive}>
              <Archive size={18} aria-hidden />
            </IconBtn>
            <IconBtn ariaLabel="Delete" onClick={trash}>
              <Trash2 size={18} aria-hidden />
            </IconBtn>
          </>
        )}
        {inArchive && (
          <>
            <IconBtn ariaLabel="Unarchive" onClick={toggleArchive}>
              <ArchiveRestore size={18} aria-hidden />
            </IconBtn>
            <IconBtn ariaLabel="Delete" onClick={trash}>
              <Trash2 size={18} aria-hidden />
            </IconBtn>
          </>
        )}
        {inTrash && (
          <>
            <IconBtn ariaLabel="Restore" onClick={restore}>
              <RotateCcw size={18} aria-hidden />
            </IconBtn>
            <IconBtn ariaLabel="Delete forever" onClick={deleteForever}>
              <Trash2 size={18} aria-hidden />
            </IconBtn>
          </>
        )}
      </div>
    </div>
  );
}

function ChipsRow({ noteLabelIds }: { noteLabelIds: string[] }) {
  const labels = useStore((s) => s.labels);
  if (!noteLabelIds.length) return null;
  const visible = noteLabelIds
    .map((id) => labels.find((l) => l.id === id))
    .filter((l): l is { id: string; name: string } => !!l);
  if (!visible.length) return null;
  return (
    <div className="flex flex-wrap gap-1 px-3 pb-2">
      {visible.map((l) => (
        <span
          key={l.id}
          className="text-xs px-2 py-0.5 rounded-full bg-black/5 dark:bg-white/10"
        >
          {l.name}
        </span>
      ))}
    </div>
  );
}
