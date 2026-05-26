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
import { useState } from "react";
import type { Note } from "../types";
import { bgFor, borderFor } from "../colors";
import { useStore } from "../store";
import { api } from "../api";
import { ColorPicker } from "./ColorPicker";

interface Props {
  note: Note;
}

export function NoteCard({ note }: Props) {
  const { section, dark, load, openEditor, showToast } = useStore();
  const [colorOpen, setColorOpen] = useState(false);

  const inTrash = section.kind === "trash";
  const inArchive = section.kind === "archive";

  const bg = bgFor(note.color, dark);
  const border = borderFor(note.color, dark);

  const togglePin = async (e: React.MouseEvent) => {
    e.stopPropagation();
    await api.setPinned(note.id, !note.pinned);
    await load();
  };

  const toggleArchive = async (e: React.MouseEvent) => {
    e.stopPropagation();
    await api.setArchived(note.id, !note.archived);
    await load();
    showToast(note.archived ? "Note unarchived" : "Note archived");
  };

  const trash = async (e: React.MouseEvent) => {
    e.stopPropagation();
    await api.setTrashed(note.id, true);
    await load();
    showToast("Note moved to Trash");
  };

  const restore = async (e: React.MouseEvent) => {
    e.stopPropagation();
    await api.setTrashed(note.id, false);
    await load();
    showToast("Note restored");
  };

  const deleteForever = async (e: React.MouseEvent) => {
    e.stopPropagation();
    await api.deleteNotePermanent(note.id);
    await load();
    showToast("Note deleted");
  };

  const onCardClick = () => {
    if (inTrash) return;
    openEditor(note.id);
  };

  const setColor = async (color: string) => {
    await api.setColor(note.id, color);
    setColorOpen(false);
    await load();
  };

  return (
    <div
      className={clsx(
        "note-card group relative rounded-lg border shadow-keep hover:shadow-keep-hover cursor-default",
        "transition-shadow",
      )}
      style={{ background: bg, borderColor: border }}
      onClick={onCardClick}
      tabIndex={0}
    >
      {!inTrash && (
        <button
          onClick={togglePin}
          className={clsx(
            "absolute top-2 right-2 p-1.5 rounded-full hover:bg-black/10 dark:hover:bg-white/10 transition-opacity",
            note.pinned ? "opacity-100" : "opacity-0 group-hover:opacity-100",
          )}
          title={note.pinned ? "Unpin" : "Pin"}
        >
          {note.pinned ? <Pin size={18} /> : <PinOff size={18} />}
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
              <span className="w-4 h-4 mt-0.5 grid place-items-center border rounded-sm border-current opacity-70">
                {it.checked && <Check size={12} />}
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
            <div className="relative">
              <IconBtn
                title="Background options"
                onClick={(e) => {
                  e.stopPropagation();
                  setColorOpen((v) => !v);
                }}
              >
                <Palette size={18} />
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
            <IconBtn title="Archive" onClick={toggleArchive}>
              <Archive size={18} />
            </IconBtn>
            <IconBtn title="Delete" onClick={trash}>
              <Trash2 size={18} />
            </IconBtn>
          </>
        )}
        {inArchive && (
          <>
            <IconBtn title="Unarchive" onClick={toggleArchive}>
              <ArchiveRestore size={18} />
            </IconBtn>
            <IconBtn title="Delete" onClick={trash}>
              <Trash2 size={18} />
            </IconBtn>
          </>
        )}
        {inTrash && (
          <>
            <IconBtn title="Restore" onClick={restore}>
              <RotateCcw size={18} />
            </IconBtn>
            <IconBtn title="Delete forever" onClick={deleteForever}>
              <Trash2 size={18} />
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
  title,
}: {
  children: React.ReactNode;
  onClick: (e: React.MouseEvent) => void;
  title: string;
}) {
  return (
    <button
      onClick={onClick}
      title={title}
      className="p-2 rounded-full hover:bg-black/10 dark:hover:bg-white/10"
    >
      {children}
    </button>
  );
}

function ChipsRow({ noteLabelIds }: { noteLabelIds: string[] }) {
  const { labels } = useStore();
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
