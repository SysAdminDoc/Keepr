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
}

export interface ChecklistItemInput {
  id?: string;
  text: string;
  checked: boolean;
  position: number;
}

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
}

export interface NoteInput {
  kind: NoteKind;
  title: string;
  body: string;
  color: ColorKey;
  pinned: boolean;
  checklist: ChecklistItemInput[];
  labels: string[];
}

export interface Label {
  id: string;
  name: string;
}

export type Section =
  | { kind: "notes" }
  | { kind: "archive" }
  | { kind: "trash" }
  | { kind: "label"; labelId: string };

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
