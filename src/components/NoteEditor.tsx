import { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react";
import clsx from "clsx";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  Pin,
  PinOff,
  Palette,
  Archive,
  ArchiveRestore,
  Trash2,
  Plus,
  X,
  Tag,
  ListChecks,
  AlignLeft,
  Copy,
  Image as ImageIcon,
  Bell,
  Lock,
  Unlock,
  History,
  MoreVertical,
} from "lucide-react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import type { Attachment } from "../types";
import { useStore } from "../store";
import { api } from "../api";
import { bgFor, borderFor } from "../colors";
import { ColorPicker } from "./ColorPicker";
import { IconBtn } from "./IconBtn";
import { extractHashtagsFromNote } from "../lib/hashtags";
import { recurrenceLabel } from "../lib/reminders";
import { useClickOutside } from "../hooks/useClickOutside";
import { ChecklistSection } from "./ChecklistSection";
import { AttachmentGrid } from "./AttachmentGrid";
// EI-V0.5-17 — both modals mount conditionally behind an open flag, so
// React.lazy keeps them out of the initial editor payload. ColorPicker
// stays eager (used on every editor open).
const ReminderPicker = lazy(() =>
  import("./ReminderPicker").then((m) => ({ default: m.ReminderPicker })),
);
const HistoryDrawer = lazy(() =>
  import("./HistoryDrawer").then((m) => ({ default: m.HistoryDrawer })),
);
import type {
  BackgroundPatternKey,
  ChecklistItemInput,
  ColorKey,
  Note,
  NoteKind,
  NoteInput,
} from "../types";
import { BACKGROUND_PATTERNS, normalizePattern } from "../lib/backgroundPatterns";

/** EI-V0.5-15 — single row of the kebab "More actions" popover. */
function MoreMenuItem({
  icon,
  label,
  onClick,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      role="menuitem"
      onClick={onClick}
      className="flex items-center gap-2 w-full px-3 py-2 text-sm text-left hover:bg-black/5 dark:hover:bg-white/10"
    >
      {icon}
      <span>{label}</span>
    </button>
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
  /** NF-22 — empty string = no pattern. */
  backgroundPattern: BackgroundPatternKey;
}

const emptyDraft = (): Draft => ({
  kind: "text",
  title: "",
  body: "",
  color: "default",
  pinned: false,
  checklist: [],
  labels: [],
  backgroundPattern: "",
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
  const upsertReminder = useStore((s) => s.upsertReminder);
  const removeReminder = useStore((s) => s.removeReminder);
  const reminders = useStore((s) => s.reminders);
  const vaultInitialized = useStore((s) => s.vaultInitialized);
  const vaultUnlocked = useStore((s) => s.vaultUnlocked);
  // NF-01 — attachments live alongside draft state but aren't part of the
  // NoteInput payload (add/remove go through their own commands so the
  // file copy + DB write stay transactional). We snapshot existing
  // attachments on open and append optimistically on add.
  const [attachments, setAttachments] = useState<Attachment[]>([]);
  const [reminderPickerOpen, setReminderPickerOpen] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);
  // EI-V0.5-15 — kebab "More" overflow menu. Absorbs the lower-priority
  // actions (Make a copy, Move to vault, Version history) so the
  // toolbar stays within ~7 visible buttons even on narrow windows.
  const [moreMenuOpen, setMoreMenuOpen] = useState(false);
  const moreMenuRef = useRef<HTMLDivElement>(null);

  // EI-06 — Snapshot the existing note once on open. We deliberately do NOT
  // depend on the store's `notes` array because a background load() would
  // swap the array reference and clobber in-progress edits. The snapshot
  // is captured imperatively in the open effect below.
  const [existing, setExisting] = useState<Note | null>(null);
  const noteReminder =
    existing ? reminders.find((r) => r.noteId === existing.id) ?? null : null;

  const [draft, setDraft] = useState<Draft>(emptyDraft());
  const [colorOpen, setColorOpen] = useState(false);
  const [labelMenuOpen, setLabelMenuOpen] = useState(false);
  const [newLabelName, setNewLabelName] = useState("");
  const [dropActive, setDropActive] = useState(false);
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
          parentId: c.parentId ?? null,
        })),
        labels: [...found.labels],
        backgroundPattern: normalizePattern(found.backgroundPattern),
      });
      setAttachments([...found.attachments]);
    } else {
      setDraft(emptyDraft());
      setAttachments([]);
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

    // NF-07 / EI-V0.5-9 — hashtag → label merge.
    // The original behaviour added labels for every #tag in the note;
    // EI-V0.5-9 also REMOVES labels whose hashtag was present when the
    // editor opened but is no longer in the saved text. So if a user
    // types `#work`, saves, then deletes the `#work` text and saves
    // again, the "work" label auto-detaches.
    const allLabels = useStore.getState().labels;
    const labelByLower = new Map(
      allLabels.map((l) => [l.name.toLowerCase(), l]),
    );
    const currentTags = extractHashtagsFromNote({
      title: d.title,
      body: d.body,
      checklist: d.checklist,
    });
    const previousTags = ex
      ? extractHashtagsFromNote({
          title: ex.title,
          body: ex.body,
          checklist: ex.checklist,
        })
      : [];
    const currentTagSet = new Set(currentTags.map((t) => t.toLowerCase()));
    const removedTagSet = new Set(
      previousTags
        .map((t) => t.toLowerCase())
        .filter((t) => !currentTagSet.has(t)),
    );
    const mergedLabelIds = new Set(d.labels);
    // Auto-add labels for current tags.
    for (const tag of currentTags) {
      const existing = labelByLower.get(tag.toLowerCase());
      if (existing) {
        mergedLabelIds.add(existing.id);
      } else {
        try {
          const created = await api.createLabel(tag);
          useStore.getState().upsertLabel(created);
          mergedLabelIds.add(created.id);
        } catch {
          // Ignore failures (e.g. UNIQUE collision races) — note will
          // simply miss the label until the next save.
        }
      }
    }
    // Auto-remove labels whose hashtag disappeared from the text.
    for (const removed of removedTagSet) {
      const lbl = labelByLower.get(removed);
      if (lbl) mergedLabelIds.delete(lbl.id);
    }

    const payload: NoteInput = {
      kind: d.kind,
      title: d.title,
      body: d.body,
      color: d.color,
      pinned: d.pinned,
      checklist: d.checklist
        .filter((c) => c.text.trim().length > 0)
        .map((c, i) => ({ ...c, position: i })),
      labels: [...mergedLabelIds],
      backgroundPattern: d.backgroundPattern,
    };
    const isEmptyNow =
      !d.title.trim() &&
      !d.body.trim() &&
      !d.checklist.some((c) => c.text.trim());
    // EI-V0.5-3 — if the user attached a reminder or images to an
    // otherwise-empty note, don't discard. Otherwise the reminder
    // cascades away with the note and the user sees their reminder
    // vanish.
    const hasReminder =
      !!ex && useStore.getState().reminders.some((r) => r.noteId === ex.id);
    const hasAttachments = !!ex && attachments.length > 0;
    try {
      if (ex) {
        if (
          isEmptyNow
          && ex.kind === "text"
          && ex.title === ""
          && ex.body === ""
          && !hasReminder
          && !hasAttachments
        ) {
          // Was empty when opened, still empty, no reminder, no
          // attachments — safe to discard. We only delete if the
          // original had content.
          await api.deleteNotePermanent(ex.id);
          removeNote(ex.id);
          // Defensive: clear any stray reminder entry too.
          useStore.getState().removeReminder(ex.id);
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
  }, [closeEditor, removeNote, upsertNote, showToast, attachments.length]);

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

  // EI-V0.5-15 — `useClickOutside` *must* run before any early return so
  // its underlying useEffect is called on every render of NoteEditor.
  // Previously this lived below `if (!editorOpen) return null;`, which
  // caused a Rules-of-Hooks violation the first time editorOpen flipped
  // true and crashed the entire app tree (no error boundary).
  useClickOutside(moreMenuRef, moreMenuOpen, () => setMoreMenuOpen(false));

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

  // EI-V0.5-10 (v0.16) — the per-row helpers (addItem / setItem /
  // removeItem / indentItem / dedentItem / sortable bookkeeping / FLIP
  // animator) all moved into ChecklistSection. The editor just hands
  // it `draft.checklist` and an onChange that splats back into draft.

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
      backgroundPattern: d.backgroundPattern,
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

  const toggleVault = async () => {
    if (!existing) return;
    try {
      const updated = await flushDraft();
      if (updated) upsertNote(updated);
      const next =
        existing.vault === "vault"
          ? await api.moveNoteOutOfVault(existing.id)
          : await api.moveNoteToVault(existing.id);
      upsertNote(next);
      showToast(
        next.vault === "vault" ? "Moved to vault" : "Moved out of vault",
      );
    } catch (e) {
      showToast("Could not toggle vault: " + String(e));
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

  // NF-01 — image attachment flow. New notes must be saved first so we
  // have a note_id to attach to; we transparently call createNote then
  // promote the newly-created note to `existing` mid-edit.
  const ensureExistingId = async (): Promise<string | null> => {
    if (existing) return existing.id;
    const d = draftRef.current;
    try {
      const created = await api.createNote({
        kind: d.kind,
        title: d.title,
        body: d.body,
        color: d.color,
        pinned: d.pinned,
        checklist: d.checklist
          .filter((c) => c.text.trim().length > 0)
          .map((c, i) => ({ ...c, position: i })),
        labels: d.labels,
        backgroundPattern: d.backgroundPattern,
      });
      upsertNote(created);
      setExisting(created);
      return created.id;
    } catch (e) {
      showToast("Could not save note: " + String(e));
      return null;
    }
  };

  const addImage = async () => {
    try {
      const picked = await openFileDialog({
        title: "Add image to note",
        multiple: false,
        filters: [
          { name: "Image", extensions: ["png", "jpg", "jpeg", "gif", "webp"] },
        ],
      });
      if (!picked) return;
      const noteId = await ensureExistingId();
      if (!noteId) return;
      const att = await api.addImageAttachment(noteId, picked as string);
      setAttachments((prev) => [...prev, att]);
      // Refresh the note in the store so the card shows the new attachment.
      patchNote(noteId, {
        attachments: [...attachments, att],
        updated_at: new Date().toISOString(),
      });
    } catch (e) {
      showToast("Could not add image: " + String(e));
    }
  };

  // NF-V0.5-I — paste + drop image. Both paths fall through to a single
  // addImageBlob helper that wraps add_image_attachment_bytes. Supported
  // MIMEs match the file-picker filter (png/jpg/gif/webp); other types
  // are silently ignored so a paste of unrelated content doesn't surprise
  // the user.
  const SUPPORTED_PASTE_MIME = new Set([
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
  ]);

  const addImageBlob = async (file: File | Blob, hint?: string) => {
    const mime = file.type;
    if (!SUPPORTED_PASTE_MIME.has(mime)) {
      showToast(`Unsupported image type: ${mime || "unknown"}`);
      return;
    }
    try {
      const noteId = await ensureExistingId();
      if (!noteId) return;
      const buf = new Uint8Array(await file.arrayBuffer());
      // Tauri's invoke serializes Uint8Array via JSON-with-base64 by way
      // of the standard array conversion. Spread into a plain number[]
      // so the IPC layer doesn't choke on typed arrays.
      const bytes = Array.from(buf);
      const att = await api.addImageAttachmentBytes(noteId, bytes, mime, hint);
      setAttachments((prev) => [...prev, att]);
      patchNote(noteId, {
        attachments: [...attachments, att],
        updated_at: new Date().toISOString(),
      });
    } catch (e) {
      showToast("Could not add image: " + String(e));
    }
  };

  const onPaste = (e: React.ClipboardEvent) => {
    if (!e.clipboardData) return;
    for (const item of Array.from(e.clipboardData.items)) {
      if (item.kind === "file") {
        const file = item.getAsFile();
        if (file && SUPPORTED_PASTE_MIME.has(file.type)) {
          e.preventDefault();
          void addImageBlob(file, file.name);
          return;
        }
      }
    }
  };

  const onDragOver = (e: React.DragEvent) => {
    if (e.dataTransfer.types.includes("Files")) {
      e.preventDefault();
      setDropActive(true);
    }
  };
  const onDragLeave = () => setDropActive(false);
  const onDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setDropActive(false);
    for (const file of Array.from(e.dataTransfer.files)) {
      if (SUPPORTED_PASTE_MIME.has(file.type)) {
        void addImageBlob(file, file.name);
      }
    }
  };

  const removeAttachment = async (att: Attachment) => {
    try {
      await api.deleteAttachment(att.id);
      const next = attachments.filter((a) => a.id !== att.id);
      setAttachments(next);
      if (existing) {
        patchNote(existing.id, {
          attachments: next,
          updated_at: new Date().toISOString(),
        });
      }
    } catch (e) {
      showToast("Could not remove image: " + String(e));
    }
  };

  // NF-02 — reminder hookup. New notes must exist before they can carry
  // a reminder, so reuse the ensureExistingId helper from NF-01.
  const setReminderForNote = async (fireAtIso: string, rrule: string | null) => {
    setReminderPickerOpen(false);
    const noteId = await ensureExistingId();
    if (!noteId) return;
    try {
      const r = await api.setReminder(noteId, fireAtIso, rrule);
      upsertReminder(r);
      const when = new Date(fireAtIso).toLocaleString();
      showToast(
        rrule ? `Reminder set for ${when} (${recurrenceLabel(rrule)})` : `Reminder set for ${when}`,
      );
    } catch (e) {
      showToast("Could not set reminder: " + String(e));
    }
  };
  const snoozeReminderForNote = async (untilIso: string) => {
    setReminderPickerOpen(false);
    if (!existing) return;
    try {
      const r = await api.snoozeReminder(existing.id, untilIso);
      upsertReminder(r);
      showToast(`Snoozed until ${new Date(untilIso).toLocaleString()}`);
    } catch (e) {
      showToast("Could not snooze reminder: " + String(e));
    }
  };
  const clearReminderForNote = async () => {
    setReminderPickerOpen(false);
    if (!existing) return;
    try {
      await api.clearReminder(existing.id);
      removeReminder(existing.id);
      showToast("Reminder cleared");
    } catch (e) {
      showToast("Could not clear reminder: " + String(e));
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
    <div className="fixed inset-0 z-50 modal-backdrop grid place-items-center p-4" onClick={close}>
      <div
        className={clsx(
          "w-full max-w-xl rounded-lg border shadow-keep-hover overflow-hidden",
          dropActive && "ring-2 ring-[#1a73e8] ring-offset-2",
        )}
        style={{
          background: bg,
          borderColor: border,
          backgroundImage: BACKGROUND_PATTERNS[draft.backgroundPattern],
          backgroundRepeat: "repeat",
        }}
        onClick={(e) => e.stopPropagation()}
        onPaste={onPaste}
        onDragOver={onDragOver}
        onDragLeave={onDragLeave}
        onDrop={onDrop}
      >
        {attachments.length > 0 && (
          <AttachmentGrid
            attachments={attachments}
            onRemove={removeAttachment}
            editable
          />
        )}

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
          // EI-V0.5-10 (v0.16) — checklist editor extracted to its own
          // component. ChecklistSection owns the dnd-kit context, FLIP
          // animator, and the indent/dedent helpers; here we just hand
          // it the current items + onChange that splats into draft.
          <ChecklistSection
            items={draft.checklist}
            onChange={(next) => setDraft({ ...draft, checklist: next })}
            moveCheckedToBottom={moveCheckedToBottom}
          />
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
          <IconBtn
            ariaLabel={noteReminder ? "Edit reminder" : "Set reminder"}
            onClick={() => setReminderPickerOpen(true)}
            pressed={!!noteReminder}
          >
            <Bell size={18} aria-hidden />
          </IconBtn>
          <IconBtn ariaLabel="Add image" onClick={addImage}>
            <ImageIcon size={18} aria-hidden />
          </IconBtn>
          <IconBtn
            ariaLabel="Background options"
            onClick={() => setColorOpen((v) => !v)}
            pressed={colorOpen}
          >
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
                patternValue={draft.backgroundPattern}
                onPatternChange={(p) => {
                  setDraft({ ...draft, backgroundPattern: p });
                  // Don't close on pattern pick — let the user iterate
                  // through the swatches without re-opening the popover.
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
            <IconBtn
              ariaLabel="Labels"
              onClick={() => setLabelMenuOpen((v) => !v)}
              pressed={labelMenuOpen}
            >
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
          {existing && !existing.trashed && (
            // EI-V0.5-15 — "More" kebab. Houses the lower-priority
            // actions (Make a copy, Version history, Move to/out of
            // vault) so the always-visible toolbar stays compact.
            <div className="relative" ref={moreMenuRef}>
              <IconBtn
                ariaLabel="More actions"
                onClick={() => setMoreMenuOpen((v) => !v)}
                pressed={moreMenuOpen}
              >
                <MoreVertical size={18} aria-hidden />
              </IconBtn>
              {moreMenuOpen && (
                <div
                  className="absolute z-30 bottom-12 left-0 w-52 rounded-lg shadow-lg border bg-white dark:bg-[#2d2e30] dark:border-[#5f6368] py-1"
                  role="menu"
                  onClick={(e) => e.stopPropagation()}
                >
                  <MoreMenuItem
                    icon={<Copy size={16} aria-hidden />}
                    label="Make a copy"
                    onClick={() => {
                      setMoreMenuOpen(false);
                      duplicate();
                    }}
                  />
                  <MoreMenuItem
                    icon={<History size={16} aria-hidden />}
                    label="Version history"
                    onClick={() => {
                      setMoreMenuOpen(false);
                      setHistoryOpen(true);
                    }}
                  />
                  {vaultInitialized && vaultUnlocked && (
                    <MoreMenuItem
                      icon={
                        existing.vault === "vault" ? (
                          <Unlock size={16} aria-hidden />
                        ) : (
                          <Lock size={16} aria-hidden />
                        )
                      }
                      label={
                        existing.vault === "vault"
                          ? "Move out of vault"
                          : "Move to vault"
                      }
                      onClick={() => {
                        setMoreMenuOpen(false);
                        toggleVault();
                      }}
                    />
                  )}
                </div>
              )}
            </div>
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

      {/* EI-V0.5-17 — only mount the lazy chunks when actually open so
          the import doesn't fire on every editor open. fallback={null}
          because both modals already gate their render on `open`. */}
      {reminderPickerOpen && (
        <Suspense fallback={null}>
          <ReminderPicker
            open={reminderPickerOpen}
            existingFireAt={noteReminder?.fireAt ?? null}
            existingRrule={noteReminder?.rrule ?? null}
            onSet={setReminderForNote}
            onSnooze={snoozeReminderForNote}
            onClear={clearReminderForNote}
            onClose={() => setReminderPickerOpen(false)}
          />
        </Suspense>
      )}

      {historyOpen && (
        <Suspense fallback={null}>
          <HistoryDrawer
            open={historyOpen}
            noteId={existing?.id ?? null}
            onClose={() => setHistoryOpen(false)}
            onRestored={() => {
              // Refresh the editor's view by reloading the note in the store.
              if (existing) {
                void api.getNote(existing.id).then((updated) => {
                  if (updated) upsertNote(updated);
                });
              }
            }}
          />
        </Suspense>
      )}
    </div>
  );
}

