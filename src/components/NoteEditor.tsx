import { useCallback, useEffect, useRef, useState } from "react";
import clsx from "clsx";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  Pin,
  PinOff,
  Palette,
  Archive,
  ArchiveRestore,
  Trash2,
  CheckSquare,
  Square,
  Plus,
  X,
  Tag,
  ListChecks,
  AlignLeft,
  Copy,
  GripVertical,
} from "lucide-react";
import {
  DndContext,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext,
  arrayMove,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { useStore } from "../store";
import { api } from "../api";
import { bgFor, borderFor } from "../colors";
import { ColorPicker } from "./ColorPicker";
import { IconBtn } from "./IconBtn";
import type { ChecklistItemInput, ColorKey, Note, NoteKind, NoteInput } from "../types";

interface ChecklistRowProps {
  /** Sortable id — unique per row (we use the row's stable key). */
  sortId: string;
  /** Whether this row participates in the drag sortable (false for
   *  rows inside the "Checked items" group — moving them around does
   *  nothing the user cares about). */
  draggable: boolean;
  item: ChecklistItemInput;
  onToggle: () => void;
  onText: (t: string) => void;
  onEnter: () => void;
  onBackspaceEmpty?: () => void;
  onRemove: () => void;
}

function ChecklistRow({
  sortId,
  draggable,
  item,
  onToggle,
  onText,
  onEnter,
  onBackspaceEmpty,
  onRemove,
}: ChecklistRowProps) {
  // NF-05 — useSortable on every row. Listeners are attached only to
  // the GripVertical handle so dragging from the input field doesn't
  // hijack text selection.
  const sortable = useSortable({ id: sortId, disabled: !draggable });
  const style: React.CSSProperties = draggable
    ? {
        transform: CSS.Transform.toString(sortable.transform),
        transition: sortable.transition,
      }
    : {};
  return (
    <div
      ref={sortable.setNodeRef}
      style={style}
      className={clsx(
        "group/item flex items-center gap-1 px-2 py-1",
        sortable.isDragging && "opacity-50",
      )}
    >
      {draggable ? (
        <button
          type="button"
          aria-label="Reorder item"
          title="Drag to reorder"
          {...sortable.attributes}
          {...sortable.listeners}
          className="p-0.5 rounded opacity-0 group-hover/item:opacity-100 focus:opacity-100 cursor-grab active:cursor-grabbing"
        >
          <GripVertical size={14} aria-hidden />
        </button>
      ) : (
        <span className="w-[18px]" aria-hidden />
      )}
      <button
        type="button"
        onClick={onToggle}
        aria-pressed={item.checked}
        aria-label={item.checked ? "Uncheck item" : "Check item"}
        className="p-1 rounded hover:bg-black/10 dark:hover:bg-white/10"
      >
        {item.checked ? (
          <CheckSquare size={18} aria-hidden />
        ) : (
          <Square size={18} aria-hidden />
        )}
      </button>
      <input
        value={item.text}
        onChange={(e) => onText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            onEnter();
          } else if (
            e.key === "Backspace" &&
            item.text === "" &&
            onBackspaceEmpty
          ) {
            e.preventDefault();
            onBackspaceEmpty();
          }
        }}
        placeholder="List item"
        aria-label="List item"
        className={clsx(
          "flex-1 bg-transparent outline-none text-[14px]",
          item.checked && "line-through opacity-60",
        )}
      />
      <button
        type="button"
        onClick={onRemove}
        aria-label="Remove item"
        className="opacity-0 group-hover/item:opacity-100 focus:opacity-100 p-1 rounded hover:bg-black/10 dark:hover:bg-white/10"
      >
        <X size={16} aria-hidden />
      </button>
    </div>
  );
}

interface Draft {
  kind: NoteKind;
  title: string;
  body: string;
  color: ColorKey;
  pinned: boolean;
  checklist: ChecklistItemInput[];
  labels: string[];
}

const emptyDraft = (): Draft => ({
  kind: "text",
  title: "",
  body: "",
  color: "default",
  pinned: false,
  checklist: [],
  labels: [],
});

export function NoteEditor() {
  const editorOpen = useStore((s) => s.editorOpen);
  const editorNoteId = useStore((s) => s.editorNoteId);
  const closeEditor = useStore((s) => s.closeEditor);
  const labels = useStore((s) => s.labels);
  const dark = useStore((s) => s.dark);
  const showToast = useStore((s) => s.showToast);
  const upsertNote = useStore((s) => s.upsertNote);
  const removeNote = useStore((s) => s.removeNote);
  const patchNote = useStore((s) => s.patchNote);
  const upsertLabel = useStore((s) => s.upsertLabel);
  const moveCheckedToBottom = useStore((s) => s.moveCheckedToBottom);
  const [checkedCollapsed, setCheckedCollapsed] = useState(false);

  // EI-06 — Snapshot the existing note once on open. We deliberately do NOT
  // depend on the store's `notes` array because a background load() would
  // swap the array reference and clobber in-progress edits. The snapshot
  // is captured imperatively in the open effect below.
  const [existing, setExisting] = useState<Note | null>(null);

  const [draft, setDraft] = useState<Draft>(emptyDraft());
  const [colorOpen, setColorOpen] = useState(false);
  const [labelMenuOpen, setLabelMenuOpen] = useState(false);
  const [newLabelName, setNewLabelName] = useState("");
  const titleRef = useRef<HTMLTextAreaElement>(null);
  const bodyRef = useRef<HTMLTextAreaElement>(null);

  // EI-07 — re-entrant guard on close().
  const closingRef = useRef(false);
  // Stable reference to the latest draft for the close handlers / OS
  // close-requested listener (which capture by closure at registration).
  const draftRef = useRef(draft);
  const existingRef = useRef<Note | null>(null);
  useEffect(() => {
    draftRef.current = draft;
  }, [draft]);
  useEffect(() => {
    existingRef.current = existing;
  }, [existing]);

  useEffect(() => {
    if (!editorOpen) return;
    closingRef.current = false;
    // Read latest store snapshot once — does not subscribe.
    const found = useStore
      .getState()
      .notes.find((n) => n.id === editorNoteId) || null;
    setExisting(found);
    if (found) {
      setDraft({
        kind: found.kind,
        title: found.title,
        body: found.body,
        color: found.color,
        pinned: found.pinned,
        checklist: found.checklist.map((c) => ({
          id: c.id,
          text: c.text,
          checked: c.checked,
          position: c.position,
        })),
        labels: [...found.labels],
      });
    } else {
      setDraft(emptyDraft());
    }
    setColorOpen(false);
    setLabelMenuOpen(false);
    // Auto-focus title for new notes (audit item 96).
    if (!editorNoteId) {
      // RAF so the textarea is mounted before we focus.
      requestAnimationFrame(() => titleRef.current?.focus());
    }
  }, [editorOpen, editorNoteId]);

  // Save + close. Stable identity via useCallback that reads from refs so
  // it never closes over stale state (EI-07).
  const close = useCallback(async () => {
    if (closingRef.current) return; // re-entrant guard
    closingRef.current = true;
    const d = draftRef.current;
    const ex = existingRef.current;
    const payload: NoteInput = {
      kind: d.kind,
      title: d.title,
      body: d.body,
      color: d.color,
      pinned: d.pinned,
      checklist: d.checklist
        .filter((c) => c.text.trim().length > 0)
        .map((c, i) => ({ ...c, position: i })),
      labels: d.labels,
    };
    const isEmptyNow =
      !d.title.trim() &&
      !d.body.trim() &&
      !d.checklist.some((c) => c.text.trim());
    try {
      if (ex) {
        if (isEmptyNow && ex.kind === "text" && ex.title === "" && ex.body === "") {
          // Was empty when opened, still empty — keep as a permanent empty
          // note (EI-23). We only delete if the original had content.
          await api.deleteNotePermanent(ex.id);
          removeNote(ex.id);
          showToast("Empty note discarded");
        } else {
          const updated = await api.updateNote(ex.id, payload);
          upsertNote(updated);
        }
      } else if (!isEmptyNow) {
        const created = await api.createNote(payload);
        upsertNote(created);
      }
    } catch (e) {
      showToast("Could not save: " + String(e));
    } finally {
      closeEditor();
    }
  }, [closeEditor, removeNote, upsertNote, showToast]);

  // Escape closes (EI-45). One stable handler, only re-binds when editor
  // open/close flips, not on every keystroke.
  useEffect(() => {
    if (!editorOpen) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        close();
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [editorOpen, close]);

  // EI-07 — ALT-F4 / OS close button must flush the in-progress draft.
  useEffect(() => {
    if (!editorOpen) return;
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    (async () => {
      try {
        const win = getCurrentWebviewWindow();
        const u = await win.onCloseRequested(async (event) => {
          // Prevent the immediate close so we have time to await save.
          event.preventDefault();
          await close();
          // Now actually close.
          await win.destroy();
        });
        if (cancelled) {
          u();
        } else {
          unlisten = u;
        }
      } catch {
        // No-op outside Tauri (e.g. vitest, pure browser preview).
      }
    })();
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, [editorOpen, close]);

  if (!editorOpen) return null;

  const bg = bgFor(draft.color, dark);
  const border = borderFor(draft.color, dark);

  const isEmpty =
    !draft.title.trim() &&
    !draft.body.trim() &&
    !draft.checklist.some((c) => c.text.trim());

  // EI-22 — `setKind` round-trip is now lossless. text -> list parses
  // GFM-style `- [x] item` / `- [ ] item` markers; list -> text writes
  // the same markers so subsequent text -> list recovers `checked`.
  const setKind = (k: NoteKind) => {
    if (k === "list" && draft.kind === "text") {
      const lines = draft.body
        .split(/\r?\n/)
        .map((s) => s.replace(/^\s+|\s+$/g, ""))
        .filter(Boolean);
      const items: ChecklistItemInput[] = lines.length
        ? lines.map((line, i) => {
            const m = /^[-*]?\s*\[( |x|X)\]\s+(.*)$/.exec(line);
            if (m) {
              return { text: m[2], checked: m[1].toLowerCase() === "x", position: i };
            }
            return { text: line, checked: false, position: i };
          })
        : [{ text: "", checked: false, position: 0 }];
      setDraft({ ...draft, kind: "list", body: "", checklist: items });
    } else if (k === "text" && draft.kind === "list") {
      const body = draft.checklist
        .map((c) => `- [${c.checked ? "x" : " "}] ${c.text}`)
        .join("\n");
      setDraft({ ...draft, kind: "text", body, checklist: [] });
    }
  };

  const addItem = () =>
    setDraft({
      ...draft,
      checklist: [
        ...draft.checklist,
        { text: "", checked: false, position: draft.checklist.length },
      ],
    });

  const setItem = (idx: number, patch: Partial<ChecklistItemInput>) => {
    const next = [...draft.checklist];
    next[idx] = { ...next[idx], ...patch };
    setDraft({ ...draft, checklist: next });
  };

  const removeItem = (idx: number) => {
    const next = draft.checklist.filter((_, i) => i !== idx);
    setDraft({ ...draft, checklist: next });
  };

  // NF-05 — sortable id per row. We mint a stable sort-id by using the
  // item's underlying id if present, falling back to its original index
  // (new items added during this editing session won't yet have an id).
  const sortIdFor = (idx: number): string => {
    const it = draft.checklist[idx];
    return it?.id ?? `__new:${idx}`;
  };
  // Map sort id -> original draft.checklist index, so drag-end can
  // resolve the array slot reliably even after group splitting.
  const indexOfSortId = (id: string): number => {
    for (let i = 0; i < draft.checklist.length; i++) {
      if (sortIdFor(i) === id) return i;
    }
    return -1;
  };

  const checklistSensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
  );

  const onChecklistDragEnd = (e: DragEndEvent) => {
    const { active, over } = e;
    if (!over || active.id === over.id) return;
    const oldIdx = indexOfSortId(String(active.id));
    const newIdx = indexOfSortId(String(over.id));
    if (oldIdx < 0 || newIdx < 0) return;
    const next = arrayMove(draft.checklist, oldIdx, newIdx).map((it, i) => ({
      ...it,
      position: i,
    }));
    setDraft({ ...draft, checklist: next });
  };

  const toggleLabel = (id: string) => {
    setDraft({
      ...draft,
      labels: draft.labels.includes(id)
        ? draft.labels.filter((x) => x !== id)
        : [...draft.labels, id],
    });
  };

  const addNewLabel = async () => {
    const name = newLabelName.trim();
    if (!name) return;
    try {
      const lbl = await api.createLabel(name);
      setNewLabelName("");
      upsertLabel(lbl);
      setDraft((d) => ({ ...d, labels: [...new Set([...d.labels, lbl.id])] }));
    } catch (e) {
      showToast("Could not create label: " + String(e));
    }
  };

  // EI-21 — flush the in-progress draft before archive/trash so the
  // user's most recent edits aren't discarded. Returns the updated Note
  // so the caller can upsert it into the store.
  const flushDraft = async (): Promise<Note | null> => {
    if (!existing) return null;
    const d = draftRef.current;
    const payload: NoteInput = {
      kind: d.kind,
      title: d.title,
      body: d.body,
      color: d.color,
      pinned: d.pinned,
      checklist: d.checklist
        .filter((c) => c.text.trim().length > 0)
        .map((c, i) => ({ ...c, position: i })),
      labels: d.labels,
    };
    return await api.updateNote(existing.id, payload);
  };

  const archive = async () => {
    if (!existing) return;
    try {
      const updated = await flushDraft();
      if (updated) upsertNote(updated);
      const becomingArchived = !existing.archived;
      await api.setArchived(existing.id, becomingArchived);
      patchNote(existing.id, {
        archived: becomingArchived,
        trashed: false,
        trashed_at: null,
        updated_at: new Date().toISOString(),
      });
      showToast(becomingArchived ? "Note archived" : "Note unarchived");
    } catch (e) {
      showToast("Could not archive: " + String(e));
    } finally {
      closeEditor();
    }
  };

  const trash = async () => {
    if (!existing) return;
    try {
      const updated = await flushDraft();
      if (updated) upsertNote(updated);
      const now = new Date().toISOString();
      await api.setTrashed(existing.id, true);
      patchNote(existing.id, {
        trashed: true,
        archived: false,
        pinned: false,
        trashed_at: now,
        updated_at: now,
      });
      showToast("Note moved to Trash");
    } catch (e) {
      showToast("Could not trash: " + String(e));
    } finally {
      closeEditor();
    }
  };

  // NF-18 — duplicate the open note. Flushes any unsaved edits to the
  // source first, then asks the Rust side to copy.
  const duplicate = async () => {
    if (!existing) return;
    try {
      const updated = await flushDraft();
      if (updated) upsertNote(updated);
      const copy = await api.duplicateNote(existing.id);
      upsertNote(copy);
      showToast("Copy made");
    } catch (e) {
      showToast("Could not duplicate: " + String(e));
    } finally {
      closeEditor();
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 modal-backdrop grid place-items-center p-4"
      onClick={close}
    >
      <div
        className="w-full max-w-xl rounded-lg border shadow-keep-hover overflow-hidden"
        style={{ background: bg, borderColor: border }}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="relative">
          <textarea
            ref={titleRef}
            value={draft.title}
            onChange={(e) => setDraft({ ...draft, title: e.target.value })}
            placeholder="Title"
            rows={1}
            className="w-full resize-none bg-transparent outline-none px-4 pt-3 pb-1 pr-12 text-lg font-medium placeholder-gray-500 dark:placeholder-gray-400"
          />
          <button
            onClick={() => setDraft({ ...draft, pinned: !draft.pinned })}
            className="absolute top-2 right-2 p-2 rounded-full hover:bg-black/10 dark:hover:bg-white/10"
            title={draft.pinned ? "Unpin" : "Pin"}
          >
            {draft.pinned ? <Pin size={18} /> : <PinOff size={18} />}
          </button>
        </div>

        {draft.kind === "text" ? (
          <textarea
            ref={bodyRef}
            value={draft.body}
            onChange={(e) => setDraft({ ...draft, body: e.target.value })}
            placeholder="Take a note…"
            rows={4}
            className="w-full resize-none bg-transparent outline-none px-4 pb-3 text-[14px] placeholder-gray-500 dark:placeholder-gray-400 min-h-[6rem]"
          />
        ) : (
          (() => {
            // NF-20 — when moveCheckedToBottom is on, split into two groups:
            // unchecked items render in their stored order at the top, then a
            // collapsible "Checked items (N)" header, then the checked items.
            // We preserve each item's original index so setItem/removeItem
            // (which key off `draft.checklist`) still work.
            const indexed = draft.checklist.map((item, originalIndex) => ({
              item,
              originalIndex,
            }));
            const uncheckedRows = moveCheckedToBottom
              ? indexed.filter(({ item }) => !item.checked)
              : indexed;
            const checkedRows = moveCheckedToBottom
              ? indexed.filter(({ item }) => item.checked)
              : [];
            const uncheckedSortIds = uncheckedRows.map(({ originalIndex }) =>
              sortIdFor(originalIndex),
            );
            return (
              <div className="px-2 py-1">
                <DndContext
                  sensors={checklistSensors}
                  collisionDetection={closestCenter}
                  onDragEnd={onChecklistDragEnd}
                >
                  <SortableContext
                    items={uncheckedSortIds}
                    strategy={verticalListSortingStrategy}
                  >
                    {uncheckedRows.map(({ item: it, originalIndex: i }) => (
                      <ChecklistRow
                        key={sortIdFor(i)}
                        sortId={sortIdFor(i)}
                        draggable
                        item={it}
                        onToggle={() => setItem(i, { checked: !it.checked })}
                        onText={(t) => setItem(i, { text: t })}
                        onEnter={addItem}
                        onBackspaceEmpty={
                          draft.checklist.length > 1
                            ? () => removeItem(i)
                            : undefined
                        }
                        onRemove={() => removeItem(i)}
                      />
                    ))}
                  </SortableContext>
                </DndContext>
                <button
                  type="button"
                  onClick={addItem}
                  className="flex items-center gap-2 px-3 py-2 text-sm opacity-70 hover:opacity-100"
                >
                  <Plus size={18} aria-hidden /> List item
                </button>
                {checkedRows.length > 0 && (
                  <div className="mt-2 pt-2 border-t border-current/10">
                    <button
                      type="button"
                      onClick={() => setCheckedCollapsed((v) => !v)}
                      aria-expanded={!checkedCollapsed}
                      className="flex items-center gap-2 px-3 py-1 text-xs uppercase tracking-wide opacity-70 hover:opacity-100"
                    >
                      <span
                        className="inline-block transition-transform motion-reduce:transition-none"
                        style={{
                          transform: checkedCollapsed
                            ? "rotate(-90deg)"
                            : "rotate(0deg)",
                        }}
                        aria-hidden
                      >
                        ▾
                      </span>
                      {checkedRows.length} Checked item
                      {checkedRows.length === 1 ? "" : "s"}
                    </button>
                    {!checkedCollapsed &&
                      checkedRows.map(({ item: it, originalIndex: i }) => (
                        <ChecklistRow
                          key={sortIdFor(i)}
                          sortId={sortIdFor(i)}
                          draggable={false}
                          item={it}
                          onToggle={() => setItem(i, { checked: !it.checked })}
                          onText={(t) => setItem(i, { text: t })}
                          onEnter={addItem}
                          onBackspaceEmpty={
                            draft.checklist.length > 1
                              ? () => removeItem(i)
                              : undefined
                          }
                          onRemove={() => removeItem(i)}
                        />
                      ))}
                  </div>
                )}
              </div>
            );
          })()
        )}

        {draft.labels.length > 0 && (
          <div className="flex flex-wrap gap-1 px-3 pb-2">
            {draft.labels.map((id) => {
              const lbl = labels.find((l) => l.id === id);
              if (!lbl) return null;
              return (
                <span
                  key={id}
                  className="flex items-center gap-1 text-xs px-2 py-0.5 rounded-full bg-black/5 dark:bg-white/10"
                >
                  {lbl.name}
                  <button onClick={() => toggleLabel(id)} title="Remove">
                    <X size={12} />
                  </button>
                </span>
              );
            })}
          </div>
        )}

        <div className="flex items-center px-1 pb-1 relative">
          <IconBtn ariaLabel="Background options" onClick={() => setColorOpen((v) => !v)}>
            <Palette size={18} aria-hidden />
          </IconBtn>
          {colorOpen && (
            <div className="absolute z-30 top-12 left-2">
              <ColorPicker
                value={draft.color}
                onChange={(c) => {
                  setDraft({ ...draft, color: c });
                  setColorOpen(false);
                }}
              />
            </div>
          )}
          <IconBtn
            ariaLabel={draft.kind === "list" ? "Hide checkboxes" : "Show checkboxes"}
            onClick={() => setKind(draft.kind === "list" ? "text" : "list")}
          >
            {draft.kind === "list" ? (
              <AlignLeft size={18} aria-hidden />
            ) : (
              <ListChecks size={18} aria-hidden />
            )}
          </IconBtn>
          <div className="relative">
            <IconBtn ariaLabel="Labels" onClick={() => setLabelMenuOpen((v) => !v)}>
              <Tag size={18} aria-hidden />
            </IconBtn>
            {labelMenuOpen && (
              <div
                className="absolute z-30 top-12 left-0 w-64 rounded-lg shadow-lg border bg-white dark:bg-[#2d2e30] dark:border-[#5f6368] p-2"
                onClick={(e) => e.stopPropagation()}
              >
                <div className="text-xs font-medium px-1 pb-1 opacity-70">
                  Label note
                </div>
                <div className="max-h-48 overflow-y-auto">
                  {labels.map((l) => (
                    <label
                      key={l.id}
                      className="flex items-center gap-2 px-2 py-1 rounded hover:bg-black/5 dark:hover:bg-white/10 cursor-pointer"
                    >
                      <input
                        type="checkbox"
                        checked={draft.labels.includes(l.id)}
                        onChange={() => toggleLabel(l.id)}
                      />
                      <span className="text-sm truncate">{l.name}</span>
                    </label>
                  ))}
                  {!labels.length && (
                    <div className="text-sm opacity-60 px-2 py-2">
                      No labels yet
                    </div>
                  )}
                </div>
                <div className="flex items-center gap-1 pt-2 border-t border-gray-200 dark:border-[#5f6368] mt-1">
                  <input
                    value={newLabelName}
                    onChange={(e) => setNewLabelName(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") addNewLabel();
                    }}
                    placeholder="Create new label"
                    className="flex-1 bg-transparent outline-none text-sm px-2 py-1"
                  />
                  <button
                    onClick={addNewLabel}
                    className="p-1 rounded hover:bg-black/5 dark:hover:bg-white/10"
                  >
                    <Plus size={16} />
                  </button>
                </div>
              </div>
            )}
          </div>
          {existing && !existing.trashed && (
            <IconBtn ariaLabel="Make a copy" onClick={duplicate}>
              <Copy size={18} aria-hidden />
            </IconBtn>
          )}
          {existing && !existing.trashed && (
            <IconBtn
              ariaLabel={existing.archived ? "Unarchive" : "Archive"}
              onClick={archive}
            >
              {existing.archived ? (
                <ArchiveRestore size={18} aria-hidden />
              ) : (
                <Archive size={18} aria-hidden />
              )}
            </IconBtn>
          )}
          {existing && !existing.trashed && (
            <IconBtn ariaLabel="Delete" onClick={trash}>
              <Trash2 size={18} aria-hidden />
            </IconBtn>
          )}
          <div className="flex-1" />
          <button
            onClick={close}
            className="px-4 py-1.5 text-sm font-medium rounded hover:bg-black/5 dark:hover:bg-white/10"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
}

