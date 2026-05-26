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
  | { kind: "label"; labelId: string };

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
}

export const EMPTY_FILTERS: SearchFilters = {
  kinds: [],
  colors: [],
  labelIds: [],
  pinnedOnly: false,
};
