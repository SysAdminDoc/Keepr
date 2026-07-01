import { invoke } from "@tauri-apps/api/core";
import type {
  Attachment,
  Note,
  NoteInput,
  NoteSnapshot,
  Label,
  Reminder,
  MarkdownVaultImportSummary,
  SmartLabel,
  SpeechModelStatus,
  TranscriptRecord,
} from "./types";

export const api = {
  listNotes: () => invoke<Note[]>("list_notes"),
  getNote: (id: string) => invoke<Note | null>("get_note", { id }),
  searchNotes: (query: string) => invoke<string[]>("search_notes", { query }),
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
  importMarkdownVault: (srcDir: string) =>
    invoke<MarkdownVaultImportSummary>("import_markdown_vault", { srcDir }),
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
  getAppVersion: () => invoke<string>("get_app_version"),
  getDataDir: () => invoke<string>("get_data_dir"),
  getLogDir: () => invoke<string>("get_log_dir"),
  openAppDir: (kind: "data" | "log") => invoke<void>("open_app_dir", { kind }),
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
  vaultHasRecoverySeed: () => invoke<boolean>("vault_has_recovery_seed"),
  setupVaultRecoverySeed: (currentPassword: string) =>
    invoke<string>("setup_vault_recovery_seed", { currentPassword }),
  removeVaultRecoverySeed: () => invoke<void>("remove_vault_recovery_seed"),
  recoverVaultWithSeed: (mnemonic: string, newPassword: string) =>
    invoke<void>("recover_vault_with_seed", { mnemonic, newPassword }),
  moveNotesToVault: (ids: string[]) =>
    invoke<number>("move_notes_to_vault", { ids }),
  moveNotesOutOfVault: (ids: string[]) =>
    invoke<number>("move_notes_out_of_vault", { ids }),
  addAudioAttachmentBytes: (
    noteId: string,
    bytes: number[],
    mime: string,
    filenameHint?: string,
  ) =>
    invoke<Attachment>("add_audio_attachment_bytes", {
      noteId,
      bytes,
      mime,
      filenameHint,
    }),
  pruneAutoBackups: (folder: string, keep: number) =>
    invoke<number>("prune_auto_backups", { folder, keep }),
  listSmartLabels: () => invoke<SmartLabel[]>("list_smart_labels"),
  createSmartLabel: (name: string, queryJson: string) =>
    invoke<SmartLabel>("create_smart_label", { name, queryJson }),
  updateSmartLabel: (id: string, patch: { name?: string; queryJson?: string }) =>
    invoke<SmartLabel>("update_smart_label", {
      id,
      name: patch.name ?? null,
      queryJson: patch.queryJson ?? null,
    }),
  deleteSmartLabel: (id: string) => invoke<void>("delete_smart_label", { id }),
  listSnapshots: (noteId: string) =>
    invoke<NoteSnapshot[]>("list_snapshots", { noteId }),
  restoreSnapshot: (snapshotId: string) =>
    invoke<Note>("restore_snapshot", { snapshotId }),
  // v0.23.0 — opt-in offline speech transcription.
  getSpeechModelStatus: () =>
    invoke<SpeechModelStatus>("get_speech_model_status"),
  downloadSpeechModel: () => invoke<void>("download_speech_model"),
  deleteSpeechModel: () => invoke<void>("delete_speech_model"),
  getTranscript: (attachmentId: string) =>
    invoke<TranscriptRecord | null>("get_transcript", { attachmentId }),
  transcribeAudioAttachment: (attachmentId: string) =>
    invoke<string>("transcribe_audio_attachment", { attachmentId }),
  // v0.24.0 — Web Clipper localhost server info.
  getWebClipperInfo: () =>
    invoke<{ port: number | null; token: string | null }>("get_web_clipper_info"),
  regenerateWebClipperToken: () =>
    invoke<string>("regenerate_web_clipper_token"),
  // v0.26.0 — LAN-only P2P sync.
  getSyncSettings: () =>
    invoke<{
      enabled: boolean;
      deviceId: string;
      deviceName: string;
      port: number | null;
      lastSync: string | null;
    }>("get_sync_settings"),
  getSyncPeers: () =>
    invoke<
      {
        deviceId: string;
        deviceName: string;
        host: string;
        port: number;
        lastSeen: string;
      }[]
    >("get_sync_peers"),
  getSyncStatus: () =>
    invoke<"disabled" | "idle" | "syncing" | "error">("get_sync_status"),
  setSyncEnabled: (enabled: boolean) =>
    invoke<void>("set_sync_enabled", { enabled }),
  syncNow: () =>
    invoke<
      {
        notesPulled: number;
        notesPushed: number;
        labelsMerged: number;
        attachmentsTransferred: number;
        peerName: string;
      }[]
    >("sync_now"),
};
