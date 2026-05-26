import { invoke } from "@tauri-apps/api/core";
import type { Attachment, Note, NoteInput, NoteSnapshot, Label, Reminder } from "./types";

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
  addImageAttachmentBytes: (
    noteId: string,
    bytes: number[],
    mime: string,
    filenameHint?: string,
  ) =>
    invoke<Attachment>("add_image_attachment_bytes", {
      noteId,
      bytes,
      mime,
      filenameHint,
    }),
  deleteAttachment: (id: string) =>
    invoke<void>("delete_attachment", { id }),
  setReminder: (noteId: string, fireAt: string, rrule?: string | null) =>
    invoke<Reminder>("set_reminder", { noteId, fireAt, rrule: rrule ?? null }),
  snoozeReminder: (noteId: string, until: string) =>
    invoke<Reminder>("snooze_reminder", { noteId, until }),
  clearReminder: (noteId: string) =>
    invoke<void>("clear_reminder", { noteId }),
  listReminders: () => invoke<Reminder[]>("list_reminders"),
  exportRemindersIcs: (dest: string) =>
    invoke<string>("export_reminders_ics", { dest }),
  exportVault: (destDir: string) =>
    invoke<string>("export_vault", { destDir }),
  importTakeout: (src: string) =>
    invoke<number>("import_takeout", { src }),
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
  getAppLockSettings: () =>
    invoke<{ enabled: boolean; lockAfterMinutes: number }>("get_app_lock_settings"),
  enableAppLock: (pin: string, lockAfterMinutes: number) =>
    invoke<void>("enable_app_lock", { pin, lockAfterMinutes }),
  disableAppLock: (currentPin: string) =>
    invoke<void>("disable_app_lock", { currentPin }),
  verifyAppLockPin: (pin: string) =>
    invoke<boolean>("verify_app_lock_pin", { pin }),
  setAppLockMinutes: (lockAfterMinutes: number) =>
    invoke<void>("set_app_lock_minutes", { lockAfterMinutes }),
  getVaultStatus: () =>
    invoke<{ initialized: boolean; unlocked: boolean }>("get_vault_status"),
  initVault: (password: string) => invoke<void>("init_vault", { password }),
  unlockVault: (password: string) =>
    invoke<boolean>("unlock_vault", { password }),
  lockVault: () => invoke<void>("lock_vault"),
  changeVaultPassword: (currentPassword: string, newPassword: string) =>
    invoke<void>("change_vault_password", { currentPassword, newPassword }),
  moveNoteToVault: (id: string) =>
    invoke<Note>("move_note_to_vault", { id }),
  moveNoteOutOfVault: (id: string) =>
    invoke<Note>("move_note_out_of_vault", { id }),
  listSnapshots: (noteId: string) =>
    invoke<NoteSnapshot[]>("list_snapshots", { noteId }),
  restoreSnapshot: (snapshotId: string) =>
    invoke<Note>("restore_snapshot", { snapshotId }),
};
