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
