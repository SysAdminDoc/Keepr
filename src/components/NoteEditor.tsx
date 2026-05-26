import { useEffect, useMemo, useRef, useState } from "react";
import clsx from "clsx";
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
} from "lucide-react";
import { useStore } from "../store";
import { api } from "../api";
import { bgFor, borderFor } from "../colors";
import { ColorPicker } from "./ColorPicker";
import type { ChecklistItemInput, ColorKey, NoteKind, NoteInput } from "../types";

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
  const {
    editorOpen,
    editorNoteId,
    closeEditor,
    notes,
    labels,
    dark,
    load,
    showToast,
  } = useStore();
  const existing = useMemo(
    () => notes.find((n) => n.id === editorNoteId) || null,
    [notes, editorNoteId],
  );

  const [draft, setDraft] = useState<Draft>(emptyDraft());
  const [colorOpen, setColorOpen] = useState(false);
  const [labelMenuOpen, setLabelMenuOpen] = useState(false);
  const [newLabelName, setNewLabelName] = useState("");
  const titleRef = useRef<HTMLTextAreaElement>(null);
  const bodyRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (!editorOpen) return;
    if (existing) {
      setDraft({
        kind: existing.kind,
        title: existing.title,
        body: existing.body,
        color: existing.color,
        pinned: existing.pinned,
        checklist: existing.checklist.map((c) => ({
          id: c.id,
          text: c.text,
          checked: c.checked,
          position: c.position,
        })),
        labels: [...existing.labels],
      });
    } else {
      setDraft(emptyDraft());
    }
    setColorOpen(false);
    setLabelMenuOpen(false);
  }, [editorOpen, existing]);

  useEffect(() => {
    if (!editorOpen) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [editorOpen, draft, existing]);

  if (!editorOpen) return null;

  const bg = bgFor(draft.color, dark);
  const border = borderFor(draft.color, dark);

  const isEmpty =
    !draft.title.trim() &&
    !draft.body.trim() &&
    !draft.checklist.some((c) => c.text.trim());

  const close = async () => {
    const payload: NoteInput = {
      kind: draft.kind,
      title: draft.title,
      body: draft.body,
      color: draft.color,
      pinned: draft.pinned,
      checklist: draft.checklist
        .filter((c) => c.text.trim().length > 0)
        .map((c, i) => ({ ...c, position: i })),
      labels: draft.labels,
    };
    try {
      if (existing) {
        if (isEmpty) {
          await api.deleteNotePermanent(existing.id);
          showToast("Empty note discarded");
        } else {
          await api.updateNote(existing.id, payload);
        }
      } else if (!isEmpty) {
        await api.createNote(payload);
      }
      await load();
    } finally {
      closeEditor();
    }
  };

  const setKind = (k: NoteKind) => {
    if (k === "list" && draft.kind === "text") {
      const lines = draft.body
        .split(/\r?\n/)
        .map((s) => s.trim())
        .filter(Boolean);
      const items: ChecklistItemInput[] = lines.length
        ? lines.map((t, i) => ({ text: t, checked: false, position: i }))
        : [{ text: "", checked: false, position: 0 }];
      setDraft({ ...draft, kind: "list", body: "", checklist: items });
    } else if (k === "text" && draft.kind === "list") {
      const body = draft.checklist.map((c) => c.text).join("\n");
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
    const lbl = await api.createLabel(name);
    setNewLabelName("");
    await load();
    setDraft((d) => ({ ...d, labels: [...new Set([...d.labels, lbl.id])] }));
  };

  const archive = async () => {
    if (!existing) return;
    await api.setArchived(existing.id, !existing.archived);
    await load();
    closeEditor();
    showToast(existing.archived ? "Note unarchived" : "Note archived");
  };

  const trash = async () => {
    if (!existing) return;
    await api.setTrashed(existing.id, true);
    await load();
    closeEditor();
    showToast("Note moved to Trash");
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
          <div className="px-2 py-1">
            {draft.checklist.map((it, i) => (
              <div
                key={i}
                className="group/item flex items-center gap-2 px-2 py-1"
              >
                <button
                  onClick={() => setItem(i, { checked: !it.checked })}
                  className="p-1 rounded hover:bg-black/10 dark:hover:bg-white/10"
                >
                  {it.checked ? (
                    <CheckSquare size={18} />
                  ) : (
                    <Square size={18} />
                  )}
                </button>
                <input
                  value={it.text}
                  onChange={(e) => setItem(i, { text: e.target.value })}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      addItem();
                    } else if (
                      e.key === "Backspace" &&
                      it.text === "" &&
                      draft.checklist.length > 1
                    ) {
                      e.preventDefault();
                      removeItem(i);
                    }
                  }}
                  placeholder="List item"
                  className={clsx(
                    "flex-1 bg-transparent outline-none text-[14px]",
                    it.checked && "line-through opacity-60",
                  )}
                />
                <button
                  onClick={() => removeItem(i)}
                  className="opacity-0 group-hover/item:opacity-100 p-1 rounded hover:bg-black/10 dark:hover:bg-white/10"
                >
                  <X size={16} />
                </button>
              </div>
            ))}
            <button
              onClick={addItem}
              className="flex items-center gap-2 px-3 py-2 text-sm opacity-70 hover:opacity-100"
            >
              <Plus size={18} /> List item
            </button>
          </div>
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
          <IconBtn title="Background options" onClick={() => setColorOpen((v) => !v)}>
            <Palette size={18} />
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
            title={draft.kind === "list" ? "Show as text" : "Show as checklist"}
            onClick={() => setKind(draft.kind === "list" ? "text" : "list")}
          >
            {draft.kind === "list" ? <AlignLeft size={18} /> : <ListChecks size={18} />}
          </IconBtn>
          <div className="relative">
            <IconBtn title="Labels" onClick={() => setLabelMenuOpen((v) => !v)}>
              <Tag size={18} />
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
            <IconBtn title={existing.archived ? "Unarchive" : "Archive"} onClick={archive}>
              {existing.archived ? <ArchiveRestore size={18} /> : <Archive size={18} />}
            </IconBtn>
          )}
          {existing && !existing.trashed && (
            <IconBtn title="Delete" onClick={trash}>
              <Trash2 size={18} />
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

function IconBtn({
  children,
  onClick,
  title,
}: {
  children: React.ReactNode;
  onClick: () => void;
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
