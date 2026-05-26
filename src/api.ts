import { invoke } from "@tauri-apps/api/core";
import type { Attachment, Note, NoteInput, Label, Reminder } from "./types";

export const api = {
  listNotes: () => invoke<Note[]>("list_notes"),
  getNote: (id: string) => invoke<Note | null>("get_note", { id }),
  createNote: (input: NoteInput) => invoke<Note>("create_note", { input }),
  updateNote: (id: string, input: NoteInput) =>
    invoke<Note>("update_note", { id, input }),
  duplicateNote: (id: string) => invoke<Note>("duplicate_note", { id }),
  reorderNotes: (ids: string[]) => invoke<void>("reorder_notes", { ids }),
  addImageAttachment: (noteId: string, srcPath: string) =>
    invoke<Attachment>("add_image_attachment", { noteId, srcPath }),
  deleteAttachment: (id: string) =>
    invoke<void>("delete_attachment", { id }),
  setReminder: (noteId: string, fireAt: string) =>
    invoke<Reminder>("set_reminder", { noteId, fireAt }),
  clearReminder: (noteId: string) =>
    invoke<void>("clear_reminder", { noteId }),
  listReminders: () => invoke<Reminder[]>("list_reminders"),
  deleteNotePermanent: (id: string) =>
    invoke<void>("delete_note_permanent", { id }),
  setArchived: (id: string, archived: boolean) =>
    invoke<void>("set_archived", { id, archived }),
  setTrashed: (id: string, trashed: boolean) =>
    invoke<void>("set_trashed", { id, trashed }),
  setPinned: (id: string, pinned: boolean) =>
    invoke<void>("set_pinned", { id, pinned }),
  setColor: (id: string, color: string) =>
    invoke<void>("set_color", { id, color }),
  listLabels: () => invoke<Label[]>("list_labels"),
  createLabel: (name: string) => invoke<Label>("create_label", { name }),
  renameLabel: (id: string, name: string) =>
    invoke<void>("rename_label", { id, name }),
  deleteLabel: (id: string) => invoke<void>("delete_label", { id }),
  setNoteLabels: (noteId: string, labelIds: string[]) =>
    invoke<void>("set_note_labels", { noteId, labelIds }),
  emptyTrash: () => invoke<void>("empty_trash"),
  exportZip: (dest: string) => invoke<string>("export_zip", { dest }),
  importZip: (src: string) => invoke<void>("import_zip", { src }),
  getDataDir: () => invoke<string>("get_data_dir"),
};
