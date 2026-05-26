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
import { useClickOutside } from "../hooks/useClickOutside";

interface Props {
  note: Note;
}

export function NoteCard({ note }: Props) {
  const section = useStore((s) => s.section);
  const dark = useStore((s) => s.dark);
  const load = useStore((s) => s.load);
  const openEditor = useStore((s) => s.openEditor);
  const showToast = useStore((s) => s.showToast);
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
      await api.setPinned(note.id, !note.pinned);
      await load();
    });
  };

  const toggleArchive = (e: React.MouseEvent) => {
    e.stopPropagation();
    return withToast("archive note", async () => {
      const becomingArchived = !note.archived;
      await api.setArchived(note.id, becomingArchived);
      await load();
      // EI-15 — Undo for archive (Keep parity).
      showToast(becomingArchived ? "Note archived" : "Note unarchived", {
        action: {
          label: "Undo",
          onClick: async () => {
            await api.setArchived(note.id, !becomingArchived);
            await load();
          },
        },
      });
    });
  };

  const trash = (e: React.MouseEvent) => {
    e.stopPropagation();
    return withToast("trash note", async () => {
      await api.setTrashed(note.id, true);
      await load();
      // EI-15 — Undo for trash (Keep parity, 5s window).
      showToast("Note moved to Trash", {
        action: {
          label: "Undo",
          onClick: async () => {
            await api.setTrashed(note.id, false);
            await load();
          },
        },
      });
    });
  };

  const restore = (e: React.MouseEvent) => {
    e.stopPropagation();
    return withToast("restore note", async () => {
      await api.setTrashed(note.id, false);
      await load();
      showToast("Note restored");
    });
  };

  const deleteForever = (e: React.MouseEvent) => {
    e.stopPropagation();
    return withToast("delete note", async () => {
      await api.deleteNotePermanent(note.id);
      await load();
      showToast("Note deleted");
    });
  };

  const openIfNotTrash = () => {
    if (inTrash) return;
    openEditor(note.id);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      openIfNotTrash();
    }
  };

  const setColor = (color: string) =>
    withToast("change color", async () => {
      await api.setColor(note.id, color);
      setColorOpen(false);
      await load();
    });

  const cardLabel = note.title || (note.body ? note.body.slice(0, 60) : "Untitled note");

  return (
    <div
      className={clsx(
        "note-card group relative rounded-lg border shadow-keep hover:shadow-keep-hover cursor-default",
        "transition-shadow motion-reduce:transition-none",
      )}
      style={{ background: bg, borderColor: border }}
      onClick={openIfNotTrash}
      onKeyDown={onKeyDown}
      role="button"
      tabIndex={0}
      aria-label={cardLabel}
    >
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

function IconBtn({
  children,
  onClick,
  ariaLabel,
}: {
  children: React.ReactNode;
  onClick: (e: React.MouseEvent) => void;
  ariaLabel: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={ariaLabel}
      title={ariaLabel}
      className="p-2 rounded-full hover:bg-black/10 dark:hover:bg-white/10"
    >
      {children}
    </button>
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
