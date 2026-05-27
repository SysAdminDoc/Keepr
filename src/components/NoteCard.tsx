import {
  Pin,
  PinOff,
  Palette,
  Archive,
  ArchiveRestore,
  Trash2,
  RotateCcw,
  Check,
  Bell,
  Lock,
} from "lucide-react";
import clsx from "clsx";
import { useRef, useState } from "react";
import type { Note } from "../types";
import { bgFor, borderFor } from "../colors";
import { BACKGROUND_PATTERNS, normalizePattern } from "../lib/backgroundPatterns";
import { useStore } from "../store";
import { api } from "../api";
import { ColorPicker } from "./ColorPicker";
import { ConfirmDialog } from "./ConfirmDialog";
import { IconBtn } from "./IconBtn";
import { AttachmentGrid } from "./AttachmentGrid";
import { effectiveFireAt, isActive, recurrenceLabel } from "../lib/reminders";
import { useClickOutside } from "../hooks/useClickOutside";
import { daysLeftInTrash } from "../lib/trashRetention";
import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";

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
  const reminder = useStore((s) =>
    s.reminders.find((r) => r.noteId === note.id) ?? null,
  );
  const selectMode = selectedIds.size > 0;
  const isSelected = selectedIds.has(note.id);

  // NF-05 — make the whole card a sortable handle whenever we're in the
  // Notes section (EI-V0.5-1 — Archive/Trash/Label sections must not
  // drag-reorder; reorder_notes would corrupt the active-Notes
  // ordering). Drag works in every sort mode now; on the first drop
  // under non-Custom modes NoteGrid auto-switches the sort to Custom
  // so the user actually sees their reorder. useSortable returns no-op
  // refs when there's no surrounding SortableContext, so this is safe
  // in any combination.
  const dragEnabled = section.kind === "notes";
  const sortable = useSortable({
    id: note.id,
    disabled: !dragEnabled,
  });
  const sortableStyle: React.CSSProperties = dragEnabled
    ? {
        transform: CSS.Transform.toString(sortable.transform),
        transition: sortable.transition,
      }
    : {};
  const [colorOpen, setColorOpen] = useState(false);
  const popoverRef = useRef<HTMLDivElement>(null);
  useClickOutside(popoverRef, colorOpen, () => setColorOpen(false));
  // v0.22.10 hardening — "Delete forever" was a one-click destructive
  // action with no recovery (snapshots cascade-delete with the note).
  // Wrap it in a confirm to prevent accidental clicks from costing data.
  const [deleteConfirmOpen, setDeleteConfirmOpen] = useState(false);

  const inTrash = section.kind === "trash";
  const inArchive = section.kind === "archive";
  const vaultUnlocked = useStore((s) => s.vaultUnlocked);
  const lockedVault = note.vault === "vault" && !vaultUnlocked;

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

  const requestDeleteForever = (e: React.MouseEvent) => {
    e.stopPropagation();
    setDeleteConfirmOpen(true);
  };
  const confirmDeleteForever = () => {
    setDeleteConfirmOpen(false);
    void withToast("delete note", async () => {
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
    if (lockedVault) {
      showToast("Unlock the vault in Settings to view this note");
      return;
    }
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
      ref={sortable.setNodeRef}
      {...(dragEnabled ? sortable.attributes : {})}
      {...(dragEnabled ? sortable.listeners : {})}
      className={clsx(
        "note-card group relative rounded-lg border shadow-keep hover:shadow-keep-hover cursor-default",
        "transition-shadow motion-reduce:transition-none",
        isSelected && "ring-2 ring-[var(--keepr-accent)] ring-offset-1",
        sortable.isDragging && "opacity-50",
      )}
      style={{
        ...sortableStyle,
        background: bg,
        borderColor: border,
        // NF-22 — tiled SVG pattern overlay. Empty key → empty string,
        // which the browser treats as "no image" so no visual change.
        backgroundImage: BACKGROUND_PATTERNS[normalizePattern(note.backgroundPattern)],
        backgroundRepeat: "repeat",
      }}
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
            ? "opacity-100 bg-[var(--keepr-accent)]"
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

      {note.attachments.length > 0 && (
        <AttachmentGrid
          attachments={note.attachments}
          maxVisible={4}
          preferThumb
        />
      )}

      <div className="px-4 pt-3 pb-2 pr-10">
        {lockedVault ? (
          <div className="flex items-center gap-2 text-sm opacity-70">
            <Lock size={14} aria-hidden /> Locked vault note
          </div>
        ) : (
          note.title && (
            <div className="font-medium text-base leading-snug break-words">
              <HighlightHashtags text={note.title} />
            </div>
          )
        )}
      </div>

      {!lockedVault && (
        note.kind === "text" ? (
          note.body && (
            <div
              className="px-4 pb-3 leading-snug whitespace-pre-wrap break-words max-h-[16rem] overflow-hidden"
              style={{ fontSize: "var(--keepr-note-font-size)" }}
            >
              <HighlightHashtags text={note.body} />
            </div>
          )
        ) : (
          <div className="px-2 pb-2">
            {note.checklist.slice(0, 12).map((it) => (
              <div
                key={it.id}
                className="flex items-start gap-2 px-2 py-1"
                style={{ fontSize: "var(--keepr-note-font-size)" }}
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
        )
      )}

      {daysLeft !== null && (
        <div className="px-3 pb-1 text-[11px] uppercase tracking-wide font-medium opacity-70">
          {daysLeft === 1 ? "1 day left" : `${daysLeft} days left`}
        </div>
      )}

      {reminder && isActive(reminder) && !lockedVault && (
        <div className="flex flex-wrap gap-1 px-3 pb-1">
          <span className="inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded-full bg-black/5 dark:bg-white/10">
            <Bell size={11} aria-hidden /> {formatReminder(effectiveFireAt(reminder))}
            {reminder.rrule && (
              <span className="opacity-70">· {recurrenceLabel(reminder.rrule)}</span>
            )}
          </span>
        </div>
      )}
      {note.vault === "vault" && vaultUnlocked && (
        <div className="flex flex-wrap gap-1 px-3 pb-1">
          <span className="inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded-full bg-[#fdd663]/30 text-[#594300] dark:bg-[#41331c] dark:text-[#fdd663]">
            <Lock size={11} aria-hidden /> Vaulted
          </span>
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
            <IconBtn ariaLabel="Delete forever" onClick={requestDeleteForever}>
              <Trash2 size={18} aria-hidden />
            </IconBtn>
          </>
        )}
      </div>
      <ConfirmDialog
        open={deleteConfirmOpen}
        title="Delete this note forever?"
        body={
          note.title.trim()
            ? `"${note.title.trim().slice(0, 80)}" and any attachments will be permanently deleted. This cannot be undone.`
            : "This note and any attachments will be permanently deleted. This cannot be undone."
        }
        confirmLabel="Delete forever"
        cancelLabel="Cancel"
        destructive
        onConfirm={confirmDeleteForever}
        onCancel={() => setDeleteConfirmOpen(false)}
      />
    </div>
  );
}

/**
 * NF-07 — render `#hashtag` tokens in a slightly different color so the
 * inline-tag pattern is visible in the card preview. Mirrors the parser
 * in src/lib/hashtags.ts; this is read-only (text remains text in
 * SQLite) so renderer drift doesn't lose data.
 */
function HighlightHashtags({ text }: { text: string }) {
  // Same regex as src/lib/hashtags.ts but expressed as a split so we get
  // the surrounding text segments back.
  const parts: React.ReactNode[] = [];
  const re = /(^|\s)#([\p{L}_][\p{L}\p{N}_-]*)/gu;
  let lastIndex = 0;
  let match: RegExpExecArray | null;
  let key = 0;
  while ((match = re.exec(text)) !== null) {
    const [whole, lead, tag] = match;
    const start = match.index;
    if (start > lastIndex) parts.push(text.slice(lastIndex, start));
    if (lead) parts.push(lead);
    parts.push(
      <span
        key={key++}
        className="text-[var(--keepr-accent)] dark:text-[#8ab4f8] font-medium"
      >
        #{tag}
      </span>,
    );
    lastIndex = start + whole.length;
  }
  if (lastIndex < text.length) parts.push(text.slice(lastIndex));
  return <>{parts}</>;
}

/** NF-02 — turn an ISO fire_at into a Keep-shaped relative date string.
 *  "Today, 3:00 PM", "Tomorrow, 8:00 AM", "Mon, May 26, 3:00 PM",
 *  or "May 12, 3:00 PM" if more than a week out. */
function formatReminder(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const now = new Date();
  const sameDay =
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate();
  const tomorrow = new Date(now);
  tomorrow.setDate(tomorrow.getDate() + 1);
  const isTomorrow =
    d.getFullYear() === tomorrow.getFullYear() &&
    d.getMonth() === tomorrow.getMonth() &&
    d.getDate() === tomorrow.getDate();
  const time = d.toLocaleTimeString([], {
    hour: "numeric",
    minute: "2-digit",
  });
  if (sameDay) return `Today, ${time}`;
  if (isTomorrow) return `Tomorrow, ${time}`;
  const dayDelta = (d.getTime() - now.getTime()) / 86_400_000;
  if (dayDelta > 0 && dayDelta < 7) {
    return d.toLocaleDateString([], {
      weekday: "short",
    }) + `, ${time}`;
  }
  return d.toLocaleDateString([], { month: "short", day: "numeric" }) + `, ${time}`;
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
