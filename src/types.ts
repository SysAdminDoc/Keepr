export type NoteKind = "text" | "list";

export type ColorKey =
  | "default"
  | "red"
  | "orange"
  | "yellow"
  | "green"
  | "teal"
  | "blue"
  | "darkblue"
  | "purple"
  | "pink"
  | "brown"
  | "gray";

export interface ChecklistItem {
  id: string;
  text: string;
  checked: boolean;
  position: number;
  /** NF-21 (v0.14+): one-level nesting. When set, this item is indented
   *  under the referenced sibling. Rust server-side validates that the
   *  referenced parent itself has `parentId === null` (single level). */
  parentId?: string | null;
}

export interface ChecklistItemInput {
  id?: string;
  text: string;
  checked: boolean;
  position: number;
  parentId?: string | null;
}

/** NF-22 (v0.14+) — Keep's 9 background patterns + "none" sentinel. */
export type BackgroundPatternKey =
  | ""
  | "groceries"
  | "food"
  | "music"
  | "recipes"
  | "notes"
  | "places"
  | "travel"
  | "video"
  | "celebration";

export interface Attachment {
  id: string;
  noteId: string;
  kind: "image" | "drawing" | "audio" | "file";
  mime: string;
  filename: string;
  byteSize: number;
  width: number | null;
  height: number | null;
  position: number;
  createdAt: string;
  resourcePath?: string | null;
  thumbPath?: string | null;
}

export type VaultState = "plain" | "vault";

export interface Note {
  id: string;
  kind: NoteKind;
  title: string;
  body: string;
  color: ColorKey;
  pinned: boolean;
  archived: boolean;
  trashed: boolean;
  position: number;
  created_at: string;
  updated_at: string;
  trashed_at: string | null;
  checklist: ChecklistItem[];
  labels: string[];
  attachments: Attachment[];
  /** NF-V0.5-C — "plain" (default) or "vault". When "vault" and the
   *  vault is locked, title/body/checklist arrive empty and the UI
   *  must show a "🔒 Locked vault note" placeholder. When unlocked,
   *  the fields are decrypted server-side and behave like a normal note.
   *  Optional so pre-v0.8 fixtures and Rust payloads without the field
   *  (older binaries) still deserialize cleanly. */
  vault?: VaultState;
  /** NF-22 (v0.14+): background pattern key (or "" = none). Optional so
   *  pre-v0.14 fixtures keep working. */
  backgroundPattern?: BackgroundPatternKey;
}

export interface NoteInput {
  kind: NoteKind;
  title: string;
  body: string;
  color: ColorKey;
  pinned: boolean;
  checklist: ChecklistItemInput[];
  labels: string[];
  /** NF-22 (v0.14+). Required on the wire so Rust knows whether to
   *  clear or set the column; renderer always passes a value (defaults
   *  to "" in `emptyDraft()`). */
  backgroundPattern: BackgroundPatternKey;
}

export interface Label {
  id: string;
  name: string;
}

export interface Reminder {
  /** The owning note's id. Reminders are keyed on `noteId` (one per note);
   *  schema v8 dropped the separate `reminders.id` PK that v0.4 had. */
  noteId: string;
  fireAt: string;
  rrule: string | null;
  snoozeUntil: string | null;
  firedAt: string | null;
  dismissedAt: string | null;
  createdAt: string;
}

export type Section =
  | { kind: "notes" }
  | { kind: "reminders" }
  | { kind: "archive" }
  | { kind: "trash" }
  | { kind: "label"; labelId: string }
  | { kind: "smart"; smartLabelId: string };

export interface NoteSnapshot {
  id: string;
  noteId: string;
  kind: NoteKind;
  title: string;
  body: string;
  color: ColorKey;
  pinned: boolean;
  checklist: ChecklistItem[];
  vault: VaultState;
  takenAt: string;
}

/** RRULE recurrence shapes supported by NF-V0.5-A. The Rust side
 *  whitelists these strings literally — see `ALLOWED_RRULES` in
 *  src-tauri/src/commands.rs. */
export type RecurrenceRule =
  | "FREQ=DAILY"
  | "FREQ=WEEKLY"
  | "FREQ=MONTHLY"
  | "FREQ=YEARLY";

export interface SearchFilters {
  /** Note kinds to include. Empty = no kind constraint. */
  kinds: NoteKind[];
  /** Colors to include. Empty = no color constraint. */
  colors: ColorKey[];
  /** Label IDs to include. Multiple = OR within the group. Empty = no constraint. */
  labelIds: string[];
  /** Only pinned notes if true. */
  pinnedOnly: boolean;
  /** Only notes with at least one image attachment if true (v0.19.4). */
  hasImage: boolean;
  /** Only notes with an active (non-fired, non-dismissed) reminder if true (v0.19.4). */
  hasReminder: boolean;
  /** Only notes in the Private Vault if true (v0.19.4). Chip is only
   *  rendered when the vault is initialized + unlocked. */
  inVault: boolean;
}

/** v0.22.2 — Smart Label = saved filter combo shown in the sidebar. */
export interface SmartLabel {
  id: string;
  name: string;
  /** Serialised SearchFilters. Stored as a JSON string so the schema
   *  doesn't break when the filter shape evolves. */
  queryJson: string;
  position: number;
  createdAt: string;
  updatedAt: string;
}

/** v0.23.0 — opt-in offline speech-to-text via whisper.cpp. */
export interface SpeechModelStatus {
  downloaded: boolean;
  modelId: string;
  modelFilename: string;
  modelSizeBytes: number;
  modelUrl: string;
  onDiskPath: string;
}

/** v0.23.0 — persisted transcript for a single audio attachment. */
export interface TranscriptRecord {
  attachmentId: string;
  noteId: string;
  text: string;
  model: string;
  createdAt: string;
  updatedAt: string;
}

/** v0.23.0 — payload of the `transcribe://model-progress` Tauri event
 *  emitted while the speech model is downloading. */
export interface ModelDownloadProgress {
  downloaded: number;
  total: number;
}

export const EMPTY_FILTERS: SearchFilters = {
  kinds: [],
  colors: [],
  labelIds: [],
  pinnedOnly: false,
  hasImage: false,
  hasReminder: false,
  inVault: false,
};
